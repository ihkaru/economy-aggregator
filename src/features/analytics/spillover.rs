//! Pure-Rust OLS VAR(p) & Generalized FEVD / Diebold-Yilmaz Spillover Engine (Phase 342).
//! Computes Total Connectedness Index (TCI) and directional FROM/TO/NET liquidity transmission.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalSpillover {
    pub column: String,
    pub from_others_pct: f64,
    pub to_others_pct: f64,
    pub net_pct: f64,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpilloverReport {
    pub columns: Vec<String>,
    pub observations_used: usize,
    pub lags: usize,
    pub horizon: usize,
    pub total_connectedness_index_pct: f64,
    pub null_noise_tci_pct: f64,
    pub null_tci_p95_pct: f64,
    pub bias_adjusted_tci_pct: f64,
    pub permutation_p_value: f64,
    pub statistically_significant: bool,
    pub connectedness_matrix_pct: Vec<Vec<f64>>,
    pub directional: Vec<DirectionalSpillover>,
}

/// Estimates standardized OLS VAR(p) and computes Pesaran-Shin / Diebold-Yilmaz Generalized FEVD.
pub fn compute_diebold_yilmaz_spillover(
    columns: &[String],
    raw_rows: &[Vec<f64>],
    lags: usize,
    horizon: usize,
) -> Result<SpilloverReport, String> {
    let n = columns.len();
    let p = lags.clamp(1, 4);
    let h_steps = horizon.clamp(2, 24);
    if n < 2 {
        return Err("Fail-fast: Spillover model requires at least 2 variables".to_string());
    }
    let t_total = raw_rows.len();
    if t_total <= p + n * p + 6 {
        return Err(format!(
            "Fail-fast: Insufficient aligned observations (have={}, need>{})",
            t_total,
            p + n * p + 6
        ));
    }
    for row in raw_rows {
        if row.len() != n || row.iter().any(|v| !v.is_finite()) {
            return Err("Fail-fast: Non-finite value or mismatched column count in spillover matrix".to_string());
        }
    }

    // Standardize each series (z-score) so USD scale differences do not distort variance shares.
    let z_rows = standardize_columns(raw_rows, n)?;
    let (tci, norm, directional) = estimate_var_gfevd(&z_rows, columns, p, h_steps)?;

    // Deterministic 200-iteration column-permutation placebo test to quantify finite-sample bias.
    let n_perm = 200usize;
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15 ^ ((t_total as u64) << 16) ^ (n as u64);
    let mut null_tcis = Vec::with_capacity(n_perm);
    let mut hits = 0usize;
    let mut perm_buf = z_rows.clone();
    for _ in 0..n_perm {
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
        if let Ok((nt, _, _)) = estimate_var_gfevd(&perm_buf, columns, p, h_steps) {
            if nt >= tci {
                hits += 1;
            }
            null_tcis.push(nt);
        }
    }
    null_tcis.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let null_mean = if null_tcis.is_empty() {
        0.0
    } else {
        null_tcis.iter().sum::<f64>() / (null_tcis.len() as f64)
    };
    let p95_idx = ((null_tcis.len() as f64) * 0.95).floor() as usize;
    let null_p95 = null_tcis
        .get(p95_idx.min(null_tcis.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0.0);
    let p_val = ((hits + 1) as f64) / ((null_tcis.len() + 1) as f64);

    Ok(SpilloverReport {
        columns: columns.to_vec(),
        observations_used: t_total - p,
        lags: p,
        horizon: h_steps,
        total_connectedness_index_pct: tci,
        null_noise_tci_pct: null_mean,
        null_tci_p95_pct: null_p95,
        bias_adjusted_tci_pct: tci - null_mean,
        permutation_p_value: p_val,
        statistically_significant: p_val < 0.05,
        connectedness_matrix_pct: norm,
        directional,
    })
}

fn estimate_var_gfevd(
    z_rows: &[Vec<f64>],
    columns: &[String],
    p: usize,
    h_steps: usize,
) -> Result<(f64, Vec<Vec<f64>>, Vec<DirectionalSpillover>), String> {
    let t_total = z_rows.len();
    let n = columns.len();
    // Build OLS design matrix X (T_eff x K) with intercept + p lags, and target Y (T_eff x N).
    let t_eff = t_total - p;
    let k = 1 + n * p;
    let mut x = vec![vec![0.0; k]; t_eff];
    let mut y = vec![vec![0.0; n]; t_eff];
    for t in p..t_total {
        let r_idx = t - p;
        x[r_idx][0] = 1.0;
        for lag in 1..=p {
            let src = &z_rows[t - lag];
            let base = 1 + (lag - 1) * n;
            x[r_idx][base..base + n].copy_from_slice(src);
        }
        y[r_idx].copy_from_slice(&z_rows[t]);
    }

    // Solve (X^T X + ridge*I) B = X^T Y
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

    // Residual covariance Sigma (N x N)
    let dof = (t_eff.saturating_sub(k)).max(1) as f64;
    let mut sigma = vec![vec![0.0; n]; n];
    for r in 0..t_eff {
        let mut resid = vec![0.0; n];
        for c in 0..n {
            let mut pred = 0.0;
            for i in 0..k {
                pred += x[r][i] * b[i][c];
            }
            resid[c] = y[r][c] - pred;
        }
        for i in 0..n {
            for j in 0..n {
                sigma[i][j] += resid[i] * resid[j];
            }
        }
    }
    for i in 0..n {
        for j in 0..n {
            sigma[i][j] /= dof;
        }
        if sigma[i][i] <= 1e-12 {
            return Err(format!("Fail-fast: Zero residual variance for variable '{}'", columns[i]));
        }
    }

    // Extract lag matrices A_1 .. A_p (each N x N, where row i, col j is coeff of y_{j,t-l} in eq i)
    let mut a_mats = vec![vec![vec![0.0; n]; n]; p];
    for lag in 0..p {
        let row_offset = 1 + lag * n;
        for i in 0..n {
            for j in 0..n {
                a_mats[lag][i][j] = b[row_offset + j][i];
            }
        }
    }

    // Compute VMA impulse matrices Phi_0 .. Phi_{H-1}
    let mut phi = vec![vec![vec![0.0; n]; n]; h_steps];
    for i in 0..n {
        phi[0][i][i] = 1.0;
    }
    for h in 1..h_steps {
        let max_l = p.min(h);
        for l in 1..=max_l {
            let prod = mat_mul(&a_mats[l - 1], &phi[h - l]);
            for i in 0..n {
                for j in 0..n {
                    phi[h][i][j] += prod[i][j];
                }
            }
        }
    }

    // Generalized FEVD: theta_ij = (1/sigma_jj) * sum_h ((Phi_h * Sigma)_ij)^2 / sum_h (Phi_h * Sigma * Phi_h^T)_ii
    let mut raw_theta = vec![vec![0.0; n]; n];
    for i in 0..n {
        let mut denom = 0.0;
        let mut num = vec![0.0; n];
        for h in 0..h_steps {
            let ps = mat_mul(&phi[h], &sigma);
            let mut diag_i = 0.0;
            for m in 0..n {
                diag_i += ps[i][m] * phi[h][i][m];
            }
            denom += diag_i;
            for j in 0..n {
                num[j] += ps[i][j] * ps[i][j];
            }
        }
        for j in 0..n {
            raw_theta[i][j] = if denom > 1e-15 {
                num[j] / (sigma[j][j] * denom)
            } else {
                0.0
            };
        }
    }

    // Row-normalize to 100%
    let mut norm = vec![vec![0.0; n]; n];
    for i in 0..n {
        let row_sum: f64 = raw_theta[i].iter().sum();
        for j in 0..n {
            norm[i][j] = if row_sum > 1e-15 {
                (raw_theta[i][j] / row_sum) * 100.0
            } else {
                0.0
            };
        }
    }

    let mut directional = Vec::with_capacity(n);
    let mut off_diag_sum = 0.0;
    for i in 0..n {
        let from_others: f64 = (0..n).filter(|&j| j != i).map(|j| norm[i][j]).sum();
        let to_others: f64 = (0..n).filter(|&j| j != i).map(|j| norm[j][i]).sum();
        off_diag_sum += from_others;
        let net = to_others - from_others;
        directional.push(DirectionalSpillover {
            column: columns[i].clone(),
            from_others_pct: from_others,
            to_others_pct: to_others,
            net_pct: net,
            role: if net >= 0.0 {
                "net_transmitter".to_string()
            } else {
                "net_receiver".to_string()
            },
        });
    }

    Ok((off_diag_sum / (n as f64), norm, directional))
}

fn standardize_columns(rows: &[Vec<f64>], n: usize) -> Result<Vec<Vec<f64>>, String> {
    let t = rows.len() as f64;
    let mut means = vec![0.0; n];
    for r in rows {
        for j in 0..n {
            means[j] += r[j];
        }
    }
    for m in &mut means {
        *m /= t;
    }
    let mut stds = vec![0.0; n];
    for r in rows {
        for j in 0..n {
            let d = r[j] - means[j];
            stds[j] += d * d;
        }
    }
    for (j, s) in stds.iter_mut().enumerate() {
        *s = (*s / (t - 1.0).max(1.0)).sqrt();
        if *s <= 1e-12 {
            return Err(format!("Fail-fast: Constant/zero-variance series at column index {}", j));
        }
    }
    Ok(rows
        .iter()
        .map(|r| (0..n).map(|j| (r[j] - means[j]) / stds[j]).collect())
        .collect())
}

fn mat_mul(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
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

fn solve_linear_system_multi(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, String> {
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
