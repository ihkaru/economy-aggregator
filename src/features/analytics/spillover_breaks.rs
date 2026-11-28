//! Prefix-Sum Pre-Inverted Wild-Rademacher Andrews sup-F Unknown-Breakpoint Structural Break Engine.
//! Heteroskedasticity- and fat-tail-robust over interior candidate breakpoints [20%..80%].

use serde::{Deserialize, Serialize};

use crate::features::analytics::spillover_diagnostics::{invert_spd, xorshift64};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationBreakStat {
    pub column: String,
    pub sup_f: f64,
    pub break_date: Option<String>,
    pub wild_p_value: f64,
    pub break_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralBreakDiagnostics {
    pub method: String,
    pub evaluated: bool,
    pub bootstrap_draws: usize,
    pub grid_points: usize,
    pub equations_tested: usize,
    pub equations_with_break: usize,
    pub system_max_sup_f: f64,
    pub system_wild_p_value: f64,
    pub structural_break_detected: bool,
    pub interpretation_note: String,
    pub per_equation: Vec<EquationBreakStat>,
}

/// Prefix-sum pre-inverted Andrews sup-F unknown-breakpoint test ([20%..80%]) with
/// Wild-Rademacher bootstrap (heteroskedasticity & fat-tail robust).
pub fn run_wild_andrews_sup_f(
    columns: &[String],
    row_dates: Option<&[String]>,
    lags: usize,
    x: &[Vec<f64>],
    y: &[Vec<f64>],
    y_hat: &[Vec<f64>],
    resid: &[Vec<f64>],
    n_boot: usize,
) -> StructuralBreakDiagnostics {
    let t_eff = x.len();
    let k = x[0].len();
    let n = columns.len();
    let lo = (((t_eff as f64) * 0.20).ceil() as usize).max(k + 12);
    let hi = (((t_eff as f64) * 0.80).floor() as usize).min(t_eff.saturating_sub(k + 12));
    if lo >= hi || t_eff < 2 * k + 28 {
        return unevaluated_break("Insufficient observations for interior unknown-breakpoint sup-F scan.");
    }

    let step = ((hi - lo) / 24).max(1);
    let grid: Vec<usize> = (lo..=hi).step_by(step).collect();

    let mut xtx_tot = vec![vec![0.0; k]; k];
    for r in 0..t_eff {
        for i in 0..k {
            let xi = x[r][i];
            for j in 0..k {
                xtx_tot[i][j] += xi * x[r][j];
            }
        }
    }
    let Some(p_pool) = invert_spd(&xtx_tot) else {
        return unevaluated_break("Singular pooled XtX matrix.");
    };

    let mut xtx_pre = vec![vec![0.0; k]; k];
    let mut g_idx = 0usize;
    let mut inv_pairs: Vec<(usize, Vec<Vec<f64>>, Vec<Vec<f64>>)> = Vec::with_capacity(grid.len());
    for r in 0..t_eff {
        if g_idx < grid.len() && r == grid[g_idx] {
            let mut xtx2 = vec![vec![0.0; k]; k];
            for i in 0..k {
                for j in 0..k {
                    xtx2[i][j] = xtx_tot[i][j] - xtx_pre[i][j];
                }
            }
            if let (Some(p1), Some(p2)) = (invert_spd(&xtx_pre), invert_spd(&xtx2)) {
                inv_pairs.push((r, p1, p2));
            }
            g_idx += 1;
        }
        for i in 0..k {
            let xi = x[r][i];
            for j in 0..k {
                xtx_pre[i][j] += xi * x[r][j];
            }
        }
    }
    if inv_pairs.is_empty() {
        return unevaluated_break("No well-conditioned interior breakpoints.");
    }

    let (obs_f, obs_tau) = eval_sup_f_fast(x, y, &p_pool, &inv_pairs);
    let max_obs = obs_f.iter().copied().fold(0.0_f64, f64::max);

    let draws = n_boot.clamp(100, 1500);
    let mut seed: u64 = 0xD1B5_4A32_D192_ED03 ^ ((t_eff as u64) << 12) ^ (n as u64);
    let mut eq_hits = vec![0usize; n];
    let mut sys_hits = 0usize;
    let mut y_star = vec![vec![0.0; n]; t_eff];

    for _ in 0..draws {
        for r in 0..t_eff {
            let sign = if (xorshift64(&mut seed) & 1) == 0 { 1.0 } else { -1.0 };
            for c in 0..n {
                y_star[r][c] = y_hat[r][c] + sign * resid[r][c];
            }
        }
        let (bf, _) = eval_sup_f_fast(x, &y_star, &p_pool, &inv_pairs);
        let bmax = bf.iter().copied().fold(0.0_f64, f64::max);
        if bmax >= max_obs {
            sys_hits += 1;
        }
        for c in 0..n {
            if bf[c] >= obs_f[c] {
                eq_hits[c] += 1;
            }
        }
    }

    let sys_p = ((sys_hits + 1) as f64) / ((draws + 1) as f64);
    let mut eq_breaks = 0usize;
    let mut per_eq = Vec::with_capacity(n);
    for c in 0..n {
        let wp = ((eq_hits[c] + 1) as f64) / ((draws + 1) as f64);
        let brk = wp < 0.05;
        if brk {
            eq_breaks += 1;
        }
        let bdate = row_dates.and_then(|ds| ds.get((obs_tau[c] + lags).min(ds.len().saturating_sub(1))).cloned());
        per_eq.push(EquationBreakStat {
            column: columns[c].clone(),
            sup_f: obs_f[c],
            break_date: bdate,
            wild_p_value: wp,
            break_detected: brk,
        });
    }
    let detected = sys_p < 0.05;
    let note = if detected {
        format!(
            "Wild-bootstrap Andrews sup-F rejects stability (system p={:.4}, {}/{} eqs p<0.05); pooled VAR mixes distinct regimes.",
            sys_p, eq_breaks, n
        )
    } else {
        format!(
            "Fail-to-reject stability null (system wild p={:.4}, {}/{} eqs p<0.05); not proof of parameter stability given finite-sample power.",
            sys_p, eq_breaks, n
        )
    };

    StructuralBreakDiagnostics {
        method: "andrews_sup_f_wild_rademacher".to_string(),
        evaluated: true,
        bootstrap_draws: draws,
        grid_points: inv_pairs.len(),
        equations_tested: n,
        equations_with_break: eq_breaks,
        system_max_sup_f: max_obs,
        system_wild_p_value: sys_p,
        structural_break_detected: detected,
        interpretation_note: note,
        per_equation: per_eq,
    }
}

fn eval_sup_f_fast(
    x: &[Vec<f64>],
    y: &[Vec<f64>],
    p_pool: &[Vec<f64>],
    inv_pairs: &[(usize, Vec<Vec<f64>>, Vec<Vec<f64>>)],
) -> (Vec<f64>, Vec<usize>) {
    let t_eff = x.len();
    let k = x[0].len();
    let n = y[0].len();
    let denom_dof = (t_eff.saturating_sub(2 * k)).max(1) as f64;

    let mut xty_tot = vec![vec![0.0; n]; k];
    let mut yy_tot = vec![0.0; n];
    let mut snaps: Vec<(Vec<Vec<f64>>, Vec<f64>)> = Vec::with_capacity(inv_pairs.len());
    let mut p_idx = 0usize;

    for r in 0..t_eff {
        if p_idx < inv_pairs.len() && r == inv_pairs[p_idx].0 {
            snaps.push((xty_tot.clone(), yy_tot.clone()));
            p_idx += 1;
        }
        for c in 0..n {
            let yc = y[r][c];
            yy_tot[c] += yc * yc;
            for i in 0..k {
                xty_tot[i][c] += x[r][i] * yc;
            }
        }
    }

    let rss_pool = rss_from_inv(p_pool, &xty_tot, &yy_tot);
    let mut best_f = vec![0.0_f64; n];
    let mut best_tau = vec![0usize; n];
    for (idx, (tau, p1, p2)) in inv_pairs.iter().enumerate() {
        let (xty1, yy1) = &snaps[idx];
        let mut xty2 = vec![vec![0.0; n]; k];
        let mut yy2 = vec![0.0; n];
        for c in 0..n {
            yy2[c] = yy_tot[c] - yy1[c];
            for i in 0..k {
                xty2[i][c] = xty_tot[i][c] - xty1[i][c];
            }
        }
        let r1 = rss_from_inv(p1, xty1, yy1);
        let r2 = rss_from_inv(p2, &xty2, &yy2);
        for c in 0..n {
            let rss_u = (r1[c] + r2[c]).max(1e-12);
            let f_val = ((rss_pool[c] - rss_u).max(0.0) / (k as f64)) / (rss_u / denom_dof);
            if f_val > best_f[c] {
                best_f[c] = f_val;
                best_tau[c] = *tau;
            }
        }
    }
    (best_f, best_tau)
}

fn rss_from_inv(p_inv: &[Vec<f64>], xty: &[Vec<f64>], yy: &[f64]) -> Vec<f64> {
    let k = p_inv.len();
    let n = yy.len();
    let mut rss = vec![0.0; n];
    for c in 0..n {
        let mut quad = 0.0;
        for i in 0..k {
            let mut bi = 0.0;
            for j in 0..k {
                bi += p_inv[i][j] * xty[j][c];
            }
            quad += xty[i][c] * bi;
        }
        rss[c] = (yy[c] - quad).max(1e-12);
    }
    rss
}

fn unevaluated_break(msg: &str) -> StructuralBreakDiagnostics {
    StructuralBreakDiagnostics {
        method: "andrews_sup_f_wild_rademacher".to_string(),
        evaluated: false,
        bootstrap_draws: 0,
        grid_points: 0,
        equations_tested: 0,
        equations_with_break: 0,
        system_max_sup_f: 0.0,
        system_wild_p_value: 1.0,
        structural_break_detected: false,
        interpretation_note: msg.to_string(),
        per_equation: Vec::new(),
    }
}
