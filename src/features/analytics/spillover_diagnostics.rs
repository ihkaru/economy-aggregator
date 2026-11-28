//! Spillover Statistical Diagnostics: Autocorrelation-Preserving Surrogate Nulls
//! (Circular Time-Shift / Block-Shuffle / i.i.d. Perm), Standardized Z-Score, and Linear Algebra.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullModel {
    CircularShift,
    BlockShuffle,
    IidPerm,
}

impl NullModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CircularShift => "circular_shift",
            Self::BlockShuffle => "block_shuffle",
            Self::IidPerm => "iid_perm",
        }
    }

    pub fn parse(raw: Option<&str>) -> Result<Self, String> {
        match raw.map(str::trim) {
            None | Some("") | Some("circular_shift") => Ok(Self::CircularShift),
            Some("block_shuffle") => Ok(Self::BlockShuffle),
            Some("iid_perm") => Ok(Self::IidPerm),
            Some(other) => Err(format!(
                "Invalid null_mode '{}'; expected 'circular_shift', 'block_shuffle', or 'iid_perm'",
                other
            )),
        }
    }
}

pub struct SurrogateNullStats {
    pub null_model: String,
    pub permutations_used: usize,
    pub null_mean_pct: f64,
    pub null_std_pct: f64,
    pub null_p95_pct: f64,
    pub tci_z_score: f64,
    pub p_value: f64,
}

#[inline]
pub fn xorshift64(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

/// Fits OLS B = (X^T X + ridge*I)^(-1) X^T Y and returns (B, Y_hat, residuals E).
pub fn fit_ols_full(x: &[Vec<f64>], y: &[Vec<f64>]) -> Result<(Vec<Vec<f64>>, Vec<Vec<f64>>, Vec<Vec<f64>>), String> {
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
    let mut y_hat = vec![vec![0.0; n]; t_eff];
    let mut resid = vec![vec![0.0; n]; t_eff];
    for r in 0..t_eff {
        for c in 0..n {
            let mut pred = 0.0;
            for i in 0..k {
                pred += x[r][i] * b[i][c];
            }
            y_hat[r][c] = pred;
            resid[r][c] = y[r][c] - pred;
        }
    }
    Ok((b, y_hat, resid))
}

pub fn invert_spd(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let k = a.len();
    let mut reg = a.to_vec();
    let mut eye = vec![vec![0.0; k]; k];
    for i in 0..k {
        reg[i][i] += 1e-8;
        eye[i][i] = 1.0;
    }
    solve_linear_system_multi(&reg, &eye).ok()
}

/// Deterministic surrogate null test supporting circular time-shift (default, preserves 100% ACF &
/// volatility clustering), block-shuffle (L=8), and i.i.d. column permutation.
pub fn run_surrogate_placebo<F>(
    z_rows: &[Vec<f64>],
    n: usize,
    lags: usize,
    obs_tci: f64,
    n_perm: usize,
    mode: NullModel,
    mut eval_tci: F,
) -> SurrogateNullStats
where
    F: FnMut(&[Vec<f64>]) -> Result<f64, String>,
{
    let t_total = z_rows.len();
    let clamped = n_perm.clamp(100, 2000);
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15 ^ ((t_total as u64) << 16) ^ (n as u64);
    let mut null_tcis = Vec::with_capacity(clamped);
    let mut hits = 0usize;
    let mut perm_buf = z_rows.to_vec();
    let min_gap = (t_total / 12).clamp(lags + 2, 12).min(t_total / 3).max(1);
    let block_len = 8usize.min((t_total / 3).max(2));

    for _ in 0..clamped {
        match mode {
            NullModel::CircularShift => {
                let mut shifts = vec![0usize; n];
                let span = t_total.saturating_sub(2 * min_gap).max(1);
                for j in 1..n {
                    let mut chosen = min_gap + ((xorshift64(&mut seed) as usize) % span);
                    for _ in 0..8 {
                        let ok = (0..j).all(|m| {
                            let d = (chosen + t_total - shifts[m]) % t_total;
                            d > lags && (t_total - d) > lags
                        });
                        if ok {
                            break;
                        }
                        chosen = min_gap + ((xorshift64(&mut seed) as usize) % span);
                    }
                    shifts[j] = chosen;
                }
                for i in 0..t_total {
                    for j in 0..n {
                        perm_buf[i][j] = z_rows[(i + shifts[j]) % t_total][j];
                    }
                }
            }
            NullModel::BlockShuffle => {
                let num_blocks = t_total.div_ceil(block_len);
                let mut order: Vec<usize> = (0..num_blocks).collect();
                for j in 0..n {
                    for b in (1..num_blocks).rev() {
                        let sw = (xorshift64(&mut seed) as usize) % (b + 1);
                        order.swap(b, sw);
                    }
                    let mut dst = 0usize;
                    for &blk in &order {
                        let start = blk * block_len;
                        let end = (start + block_len).min(t_total);
                        for src in start..end {
                            perm_buf[dst][j] = z_rows[src][j];
                            dst += 1;
                        }
                    }
                }
            }
            NullModel::IidPerm => {
                for j in 0..n {
                    for i in (1..t_total).rev() {
                        let sw = (xorshift64(&mut seed) as usize) % (i + 1);
                        let tmp = perm_buf[i][j];
                        perm_buf[i][j] = perm_buf[sw][j];
                        perm_buf[sw][j] = tmp;
                    }
                }
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
    let mean = if m == 0 { 0.0 } else { null_tcis.iter().sum::<f64>() / (m as f64) };
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

    SurrogateNullStats {
        null_model: mode.as_str().to_string(),
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
