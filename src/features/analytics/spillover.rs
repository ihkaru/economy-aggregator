//! Pure-Rust OLS VAR(p) & Generalized FEVD / Diebold-Yilmaz Spillover Engine (Phase 342).
//! Defaults to autocorrelation-preserving Circular Time-Shift Surrogate null (1,000 draws),
//! Wild-Rademacher Andrews sup-F structural-break diagnostics, and explicit methodology warnings.

use serde::{Deserialize, Serialize};

use crate::features::analytics::spillover_breaks::{
    run_wild_andrews_sup_f, StructuralBreakDiagnostics,
};
use crate::features::analytics::spillover_diagnostics::{
    fit_ols_full, mat_mul, run_surrogate_placebo, NullModel,
};

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
    pub null_model: String,
    pub permutations_used: usize,
    pub total_connectedness_index_pct: f64,
    pub null_noise_tci_pct: f64,
    pub null_tci_std_pct: f64,
    pub null_tci_p95_pct: f64,
    pub bias_adjusted_tci_pct: f64,
    pub tci_z_score: f64,
    pub permutation_p_value: f64,
    pub statistically_significant: bool,
    pub structural_break: StructuralBreakDiagnostics,
    pub methodology_warnings: Vec<String>,
    pub connectedness_matrix_pct: Vec<Vec<f64>>,
    pub directional: Vec<DirectionalSpillover>,
}

/// Standardized OLS VAR(p) & Diebold-Yilmaz GFEVD with 1,000 circular time-shift surrogates.
pub fn compute_diebold_yilmaz_spillover(
    columns: &[String],
    raw_rows: &[Vec<f64>],
    lags: usize,
    horizon: usize,
) -> Result<SpilloverReport, String> {
    compute_diebold_yilmaz_spillover_full(
        columns,
        raw_rows,
        None,
        lags,
        horizon,
        1000,
        NullModel::CircularShift,
    )
}

/// Backward-compatible wrapper with custom surrogate draw count (circular time-shift default).
pub fn compute_diebold_yilmaz_spillover_with_options(
    columns: &[String],
    raw_rows: &[Vec<f64>],
    lags: usize,
    horizon: usize,
    permutations: usize,
) -> Result<SpilloverReport, String> {
    compute_diebold_yilmaz_spillover_full(
        columns,
        raw_rows,
        None,
        lags,
        horizon,
        permutations,
        NullModel::CircularShift,
    )
}

/// Full configurable VAR(p)/GFEVD engine with date-labeled wild-bootstrap Andrews sup-F & null selector.
pub fn compute_diebold_yilmaz_spillover_full(
    columns: &[String],
    raw_rows: &[Vec<f64>],
    row_dates: Option<&[String]>,
    lags: usize,
    horizon: usize,
    permutations: usize,
    null_mode: NullModel,
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

    let z_rows = standardize_columns(raw_rows, n)?;
    let (x, y) = build_var_design(&z_rows, n, p);
    let (tci, norm, directional, y_hat, resid) = estimate_var_gfevd(&x, &y, columns, p, h_steps)?;
    let structural_break =
        run_wild_andrews_sup_f(columns, row_dates, p, &x, &y, &y_hat, &resid, 1000);

    let null_stats = run_surrogate_placebo(&z_rows, n, p, tci, permutations, null_mode, |perm_z| {
        let (px, py) = build_var_design(perm_z, n, p);
        estimate_var_gfevd(&px, &py, columns, p, h_steps).map(|(nt, _, _, _, _)| nt)
    });

    let mut warnings = Vec::new();
    let has_net_liq = columns.iter().any(|c| c == "fed_net_liquidity");
    let has_mmf = columns.iter().any(|c| c == "fiat_park_mmf");
    if has_net_liq && has_mmf {
        warnings.push(
            "Accounting-overlap caution: ON RRP inside fed_net_liquidity is a direct counterparty to MMFs (fiat_park_mmf). Directional roles may partly reflect balance-sheet mechanics; prefer liquidity_mode=ex_rrp (WALCL-TGA)."
                .to_string(),
        );
    }
    if null_mode == NullModel::IidPerm {
        warnings.push(
            "Null-model caution: iid_perm destroys per-series autocorrelation and volatility clustering, inflating tci_z_score; prefer null_mode=circular_shift."
                .to_string(),
        );
    }
    if structural_break.structural_break_detected {
        warnings.push(format!(
            "Structural break detected: wild-bootstrap Andrews sup-F flagged system p={:.4} ({}/{} equations p<0.05, max sup-F={:.2}). Pooled VAR attenuates regime changes; inspect sub-windows.",
            structural_break.system_wild_p_value,
            structural_break.equations_with_break,
            structural_break.equations_tested,
            structural_break.system_max_sup_f
        ));
    }
    warnings.push(
        "Epistemic scope: reduced-form GFEVD measures predictive forecast-error variance shares, not structural causal identification; directional roles are specification-sensitive."
            .to_string(),
    );

    Ok(SpilloverReport {
        columns: columns.to_vec(),
        observations_used: t_total - p,
        lags: p,
        horizon: h_steps,
        null_model: null_stats.null_model,
        permutations_used: null_stats.permutations_used,
        total_connectedness_index_pct: tci,
        null_noise_tci_pct: null_stats.null_mean_pct,
        null_tci_std_pct: null_stats.null_std_pct,
        null_tci_p95_pct: null_stats.null_p95_pct,
        bias_adjusted_tci_pct: tci - null_stats.null_mean_pct,
        tci_z_score: null_stats.tci_z_score,
        permutation_p_value: null_stats.p_value,
        statistically_significant: null_stats.p_value < 0.05,
        structural_break,
        methodology_warnings: warnings,
        connectedness_matrix_pct: norm,
        directional,
    })
}

fn build_var_design(z_rows: &[Vec<f64>], n: usize, p: usize) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let t_total = z_rows.len();
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
    (x, y)
}

fn estimate_var_gfevd(
    x: &[Vec<f64>],
    y: &[Vec<f64>],
    columns: &[String],
    p: usize,
    h_steps: usize,
) -> Result<(f64, Vec<Vec<f64>>, Vec<DirectionalSpillover>, Vec<Vec<f64>>, Vec<Vec<f64>>), String> {
    let t_eff = x.len();
    let k = x[0].len();
    let n = columns.len();
    let (b, y_hat, resid) = fit_ols_full(x, y)?;

    let dof = (t_eff.saturating_sub(k)).max(1) as f64;
    let mut sigma = vec![vec![0.0; n]; n];
    for r in 0..t_eff {
        for i in 0..n {
            for j in 0..n {
                sigma[i][j] += resid[r][i] * resid[r][j];
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

    let mut a_mats = vec![vec![vec![0.0; n]; n]; p];
    for lag in 0..p {
        let row_offset = 1 + lag * n;
        for i in 0..n {
            for j in 0..n {
                a_mats[lag][i][j] = b[row_offset + j][i];
            }
        }
    }

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

    Ok((off_diag_sum / (n as f64), norm, directional, y_hat, resid))
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
