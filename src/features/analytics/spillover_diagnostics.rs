//! Spillover Statistical Diagnostics: High-Resolution Permutation Placebo,
//! Standardized Z-Score vs Null, Midpoint Chow Structural-Break Test, and Linear Algebra Helpers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationBreakStat {
    pub column: String,
    pub chow_f: f64,
    pub break_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralBreakDiagnostics {
    pub evaluated: bool,
    pub split_midpoint_index: usize,
    pub equations_tested: usize,
    pub equations_with_break: usize,
    pub max_chow_f: f64,
    pub critical_f_1pct: f64,
    pub structural_break_detected: bool,
    pub per_equation: Vec<EquationBreakStat>,
}

pub struct PermutationNullStats {
    pub permutations_used: usize,
    pub null_mean_pct: f64,
    pub null_std_pct: f64,
    pub null_p95_pct: f64,
    pub tci_z_score: f64,
    pub p_value: f64,
}

/// Wilson-Hilferty 1% upper-tail F(df1=k, df2=inf) critical threshold: chi2_0.01(k) / k.
pub fn wilson_hilferty_f_crit_1pct(k: usize) -> f64 {
    let kf = (k.max(1)) as f64;
    let term = 2.0 / (9.0 * kf);
    let inner = (1.0 - term + 2.326_347_874 * term.sqrt()).max(0.1);
    inner * inner * inner
}

/// Fits OLS B = (X^T X + ridge*I)^(-1) X^T Y and returns (B, per-column RSS).
pub fn fit_ols_and_rss(x: &[Vec<f64>], y: &[Vec<f64>]) -> Result<(Vec<Vec<f64>>, Vec<f64>), String> {
    let t_eff = x.len();
    let k = x[0].len();
    let n = y[0].len();
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![vec![0.0; n]; k];
    for r in 0..t_eff {
        for i in 0..k {
            let xi = x[r][i];
            for j in 0..k {
                xtx[i][j] += xi * x[r][j];
            }
            for c in 0..n {
                xty[i][c] += xi * y[r][c];
            }
        }
    }
    for i in 0..k {
        xtx[i][i] += 1e-8;
    }
    let b = solve_linear_system_multi(&xtx, &xty)?;
    let mut rss = vec![0.0; n];
    for r in 0..t_eff {
        for c in 0..n {
            let mut pred = 0.0;
            for i in 0..k {
                pred += x[r][i] * b[i][c];
            }
            let e = y[r][c] - pred;
            rss[c] += e * e;
        }
    }
    Ok((b, rss))
}

/// Equation-by-equation midpoint Chow F-test for VAR(p) parameter stability.
pub fn run_midpoint_chow_break_test(
    columns: &[String],
    x: &[Vec<f64>],
    y: &[Vec<f64>],
    rss_pool: &[f64],
) -> StructuralBreakDiagnostics {
    let t_eff = x.len();
    let k = x[0].len();
    let n = columns.len();
    let crit = wilson_hilferty_f_crit_1pct(k);
    if t_eff < 2 * k + 24 {
        return StructuralBreakDiagnostics {
            evaluated: false,
            split_midpoint_index: t_eff / 2,
            equations_tested: 0,
            equations_with_break: 0,
            max_chow_f: 0.0,
            critical_f_1pct: crit,
            structural_break_detected: false,
            per_equation: Vec::new(),
        };
    }
    let mid = t_eff / 2;
    let Ok((_, rss1)) = fit_ols_and_rss(&x[..mid], &y[..mid]) else {
        return StructuralBreakDiagnostics {
            evaluated: false,
            split_midpoint_index: mid,
            equations_tested: 0,
            equations_with_break: 0,
            max_chow_f: 0.0,
            critical_f_1pct: crit,
            structural_break_detected: false,
            per_equation: Vec::new(),
        };
    };
    let Ok((_, rss2)) = fit_ols_and_rss(&x[mid..], &y[mid..]) else {
        return StructuralBreakDiagnostics {
            evaluated: false,
            split_midpoint_index: mid,
            equations_tested: 0,
            equations_with_break: 0,
            max_chow_f: 0.0,
            critical_f_1pct: crit,
            structural_break_detected: false,
            per_equation: Vec::new(),
        };
    };

    let denom_dof = (t_eff.saturating_sub(2 * k)).max(1) as f64;
    let mut per_eq = Vec::with_capacity(n);
    let mut breaks = 0usize;
    let mut max_f = 0.0f64;
    for j in 0..n {
        let rss_u = (rss1[j] + rss2[j]).max(1e-15);
        let num = ((rss_pool[j] - rss_u).max(0.0)) / (k as f64);
        let f_stat = num / (rss_u / denom_dof);
        let is_brk = f_stat > crit;
        if is_brk {
            breaks += 1;
        }
        if f_stat > max_f {
            max_f = f_stat;
        }
        per_eq.push(EquationBreakStat {
            column: columns[j].clone(),
            chow_f: f_stat,
            break_detected: is_brk,
        });
    }
    let min_breaks_for_system = if n >= 4 { 2 } else { 1 };
    StructuralBreakDiagnostics {
        evaluated: true,
        split_midpoint_index: mid,
        equations_tested: n,
        equations_with_break: breaks,
        max_chow_f: max_f,
        critical_f_1pct: crit,
        structural_break_detected: breaks >= min_breaks_for_system,
        per_equation: per_eq,
    }
}

/// Deterministic Xorshift64 column-permutation placebo loop returning null distribution & z-score.
pub fn run_permutation_placebo<F>(
    z_rows: &[Vec<f64>],
    n: usize,
    obs_tci: f64,
    n_perm: usize,
    mut eval_tci: F,
) -> PermutationNullStats
where
    F: FnMut(&[Vec<f64>]) -> Result<f64, String>,
{
    let t_total = z_rows.len();
    let clamped = n_perm.clamp(100, 2000);
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15 ^ ((t_total as u64) << 16) ^ (n as u64);
    let mut null_tcis = Vec::with_capacity(clamped);
    let mut hits = 0usize;
    let mut perm_buf = z_rows.to_vec();

    for _ in 0..clamped {
        for j in 0..n {
            for i in (1..t_total).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let swap_idx = (seed as usize) % (i + 1);
                let tmp = perm_buf[i][j];
                perm_buf[i][j] = perm_buf[swap_idx][j];
                perm_buf[swap_idx][j] = tmp;
            }
        }
        if let Ok(nt) = eval_tci(&perm_buf) {
            if nt >= obs_tci {
                hits += 1;
            }
            null_tcis.push(nt);
        }
    }

    null_tcis.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let m = null_tcis.len();
    let mean = if m == 0 {
        0.0
    } else {
        null_tcis.iter().sum::<f64>() / (m as f64)
    };
    let std = if m <= 1 {
        0.0
    } else {
        let var = null_tcis.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / ((m - 1) as f64);
        var.sqrt()
    };
    let p95_idx = ((m as f64) * 0.95).floor() as usize;
    let p95 = null_tcis.get(p95_idx.min(m.saturating_sub(1))).copied().unwrap_or(0.0);
    let z_score = if std > 1e-9 { (obs_tci - mean) / std } else { 0.0 };
    let p_val = ((hits + 1) as f64) / ((m + 1) as f64);

    PermutationNullStats {
        permutations_used: m,
        null_mean_pct: mean,
        null_std_pct: std,
        null_p95_pct: p95,
        tci_z_score: z_score,
        p_value: p_val,
    }
}

pub fn mat_mul(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut out = vec![vec![0.0; n]; n];
    for i in 0..n {
        for m in 0..n {
            let aim = a[i][m];
            for j in 0..n {
                out[i][j] += aim * b[m][j];
            }
        }
    }
    out
}

pub fn solve_linear_system_multi(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, String> {
    let k = a.len();
    let m = b[0].len();
    let mut aug: Vec<Vec<f64>> = (0..k)
        .map(|i| {
            let mut row = Vec::with_capacity(k + m);
            row.extend_from_slice(&a[i]);
            row.extend_from_slice(&b[i]);
            row
        })
        .collect();

    for col in 0..k {
        let mut pivot = col;
        let mut max_val = aug[col][col].abs();
        for r in (col + 1)..k {
            if aug[r][col].abs() > max_val {
                max_val = aug[r][col].abs();
                pivot = r;
            }
        }
        if max_val < 1e-14 {
            return Err("Fail-fast: Singular design matrix in VAR OLS estimation".to_string());
        }
        aug.swap(col, pivot);
        let div = aug[col][col];
        for c in col..(k + m) {
            aug[col][c] /= div;
        }
        for r in 0..k {
            if r == col {
                continue;
            }
            let factor = aug[r][col];
            if factor.abs() > 1e-18 {
                for c in col..(k + m) {
                    aug[r][c] -= factor * aug[col][c];
                }
            }
        }
    }

    Ok((0..k).map(|i| aug[i][k..(k + m)].to_vec()).collect())
}
