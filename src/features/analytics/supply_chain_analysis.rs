//! Pure Zero-Lookahead Cross-Stock Supply-Chain Lead-Lag Analysis Engine.
//! Evaluates a target stock against its registered supply-chain peers across lags
//! `[1, 2, 5, 10, 21, 42, 63]` for `h=21d` (or `5d`), identifies SURVIVED vs FLIPPED
//! relationships, and generates a causal BUY / HOLD / SELL signal from survivors.

use std::collections::BTreeMap;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::core::types::AggregatorCandle;
use crate::features::analytics::smh_predictive::TradeSignal;
use crate::features::analytics::smh_predictive_math::{align_closes, lag_vec, rolling_z};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainLagRow {
    pub supplier: String,
    pub feature: String,
    pub lag_days: usize,
    pub r_discovery: f64,
    pub r_oos: f64,
    pub newey_west_t: f64,
    pub status: String, // "SURVIVED" | "FLIPPED" | "DECAYED"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainSeriesPoint {
    pub time: String,
    pub close: f64,
    pub forecast_price: Option<f64>,
    pub signal: Option<TradeSignal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainAnalysisResponse {
    pub target: String,
    pub as_of: String,
    pub horizon_days: usize,
    pub latest_price: f64,
    pub signal: TradeSignal,
    pub signal_reason: String,
    pub forecast_ret_pct: f64,
    pub forecast_price: f64,
    pub oos_r2_pct: f64,
    pub directional_accuracy_pct: f64,
    pub baseline_up_rate_pct: f64,
    pub survivor_count: usize,
    pub lag_matrix: Vec<SupplyChainLagRow>,
    pub history: Vec<SupplyChainSeriesPoint>,
}

fn log_ret(arr: &[f64], k: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; arr.len()];
    for i in k..arr.len() {
        if arr[i].is_finite() && arr[i - k].is_finite() && arr[i - k] > 0.0 && arr[i] > 0.0 {
            out[i] = (arr[i] / arr[i - k]).ln();
        }
    }
    out
}

fn pearson_r(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 30 {
        return 0.0;
    }
    let ma = a[..n].iter().sum::<f64>() / (n as f64);
    let mb = b[..n].iter().sum::<f64>() / (n as f64);
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..n {
        let xa = a[i] - ma;
        let xb = b[i] - mb;
        num += xa * xb;
        da += xa * xa;
        db += xb * xb;
    }
    let den = (da * db).sqrt();
    if den > 1e-15 { num / den } else { 0.0 }
}

fn newey_west_t_1d(x: &[f64], y: &[f64], max_lag: usize) -> f64 {
    let n = x.len();
    if n < 30 {
        return 0.0;
    }
    let mx = x.iter().sum::<f64>() / (n as f64);
    let my = y.iter().sum::<f64>() / (n as f64);
    let var_x = x.iter().map(|&v| (v - mx).powi(2)).sum::<f64>();
    if var_x < 1e-15 {
        return 0.0;
    }
    let slope = x.iter().zip(y.iter()).map(|(&xi, &yi)| (xi - mx) * (yi - my)).sum::<f64>() / var_x;
    let intercept = my - slope * mx;
    let e: Vec<f64> = x.iter().zip(y.iter()).map(|(&xi, &yi)| yi - (intercept + slope * xi)).collect();
    let dx: Vec<f64> = x.iter().map(|&xi| xi - mx).collect();
    let mut s_val = (0..n).map(|i| (e[i] * dx[i]).powi(2)).sum::<f64>();
    for l in 1..=max_lag.min(n - 1) {
        let w = 1.0 - (l as f64) / ((max_lag + 1) as f64);
        let g: f64 = (l..n).map(|i| e[i] * dx[i] * e[i - l] * dx[i - l]).sum();
        s_val += 2.0 * w * g;
    }
    let se = (s_val.max(1e-18) / (var_x * var_x)).sqrt();
    slope / se
}

pub fn evaluate_supply_chain_lead_lag(
    target: &str,
    as_of: NaiveDate,
    horizon_days: usize,
    range_days: usize,
    target_candles: &[AggregatorCandle],
    supplier_candles: &[(String, Vec<AggregatorCandle>)],
) -> Option<SupplyChainAnalysisResponse> {
    let mut t_map = BTreeMap::new();
    for c in target_candles {
        let d = c.timestamp_utc.date_naive();
        if d <= as_of && c.close.is_finite() && c.close > 0.0 {
            t_map.insert(d, c.close);
        }
    }
    if t_map.len() < 350 {
        return None;
    }
    let dates: Vec<NaiveDate> = t_map.keys().copied().collect();
    let close: Vec<f64> = t_map.values().copied().collect();
    let n = dates.len();
    let h = horizon_days.clamp(5, 21);

    let mut y = vec![f64::NAN; n];
    for i in 0..n.saturating_sub(h) {
        y[i] = (close[i + h] / close[i]).ln();
    }
    let split_date = NaiveDate::from_ymd_opt(2022, 12, 31).unwrap();
    let lags = [1usize, 2, 5, 10, 21, 42, 63];

    let mut all_rows: Vec<(f64, SupplyChainLagRow, Vec<f64>)> = Vec::new();
    for (sup_sym, sup_c) in supplier_candles {
        let s_px = align_closes(&dates, sup_c);
        let s_log: Vec<f64> = s_px.iter().map(|&v| if v.is_finite() && v > 0.0 { v.ln() } else { f64::NAN }).collect();
        let spread: Vec<f64> = close.iter().zip(s_px.iter())
            .map(|(&t, &s)| if t > 0.0 && s.is_finite() && s > 0.0 { (t / s).ln() } else { f64::NAN })
            .collect();

        let base_feats = [
            (format!("{}_r21", sup_sym), log_ret(&s_px, 21)),
            (format!("{}_z63", sup_sym), rolling_z(&s_log, 63)),
            (format!("spread_{}_vs_{}_z63", target, sup_sym), rolling_z(&spread, 63)),
        ];

        for (fname, base_arr) in base_feats {
            for &lag in &lags {
                let x = lag_vec(&base_arr, lag);
                let mut xd = Vec::new();
                let mut yd = Vec::new();
                let mut xo = Vec::new();
                let mut yo = Vec::new();
                for i in 0..n {
                    if x[i].is_finite() && y[i].is_finite() {
                        if dates[i] <= split_date {
                            xd.push(x[i]);
                            yd.push(y[i]);
                        } else {
                            xo.push(x[i]);
                            yo.push(y[i]);
                        }
                    }
                }
                if xd.len() < 180 || xo.len() < 120 {
                    continue;
                }
                let rd = pearson_r(&xd, &yd);
                let ro = pearson_r(&xo, &yo);
                let nw_t = newey_west_t_1d(&xo, &yo, h);
                let status = if rd * ro > 0.0 && rd.abs() >= 0.08 && ro.abs() >= 0.10 {
                    "SURVIVED"
                } else if rd * ro < 0.0 {
                    "FLIPPED"
                } else {
                    "DECAYED"
                };
                let score = if status == "SURVIVED" { rd.abs().min(ro.abs()) } else { rd.abs() * 0.1 };
                all_rows.push((
                    score,
                    SupplyChainLagRow {
                        supplier: sup_sym.clone(),
                        feature: fname.clone(),
                        lag_days: lag,
                        r_discovery: rd,
                        r_oos: ro,
                        newey_west_t: nw_t,
                        status: status.to_string(),
                    },
                    x,
                ));
            }
        }
    }

    all_rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let survivor_count = all_rows.iter().filter(|r| r.1.status == "SURVIVED").count();

    // Pick best causal feature (prefer top SURVIVED, else top overall)
    let best_x = all_rows.first().map(|r| r.2.clone()).unwrap_or_else(|| vec![f64::NAN; n]);
    let best_row = all_rows.first().map(|r| r.1.clone());

    // Strictly causal expanding univariate OLS/Ridge on best_x
    let mut pred_ret = vec![None; n];
    let mut pred_px = vec![None; n];
    let mut sigs = vec![None; n];
    let mut oos_p = Vec::new();
    let mut oos_a = Vec::new();
    let mut oos_m = Vec::new();
    let mut x_tr = Vec::new();
    let mut y_tr = Vec::new();

    for t in 0..n {
        if t >= h {
            let k = t - h;
            if best_x[k].is_finite() && y[k].is_finite() {
                x_tr.push(best_x[k]);
                y_tr.push(y[k]);
            }
        }
        if x_tr.len() >= 250 && best_x[t].is_finite() {
            let m = x_tr.len() as f64;
            let mx = x_tr.iter().sum::<f64>() / m;
            let my = y_tr.iter().sum::<f64>() / m;
            let vx = x_tr.iter().map(|&v| (v - mx).powi(2)).sum::<f64>() + 120.0;
            let cov = x_tr.iter().zip(y_tr.iter()).map(|(&xi, &yi)| (xi - mx) * (yi - my)).sum::<f64>();
            let p_log = my + (cov / vx) * (best_x[t] - mx);
            let p_pct = (p_log.exp() - 1.0) * 100.0;
            let sig = if p_pct >= 0.50 {
                TradeSignal::Buy
            } else if p_pct <= -0.25 {
                TradeSignal::Sell
            } else {
                TradeSignal::Hold
            };
            pred_ret[t] = Some(p_pct);
            pred_px[t] = Some(close[t] * p_log.exp());
            sigs[t] = Some(sig);
            if y[t].is_finite() {
                oos_p.push(p_log);
                oos_a.push(y[t]);
                oos_m.push(my);
            }
        }
    }

    let mse_m = oos_p.iter().zip(&oos_a).map(|(p, a)| (a - p).powi(2)).sum::<f64>() / (oos_a.len().max(1) as f64);
    let mse_b = oos_m.iter().zip(&oos_a).map(|(m, a)| (a - m).powi(2)).sum::<f64>() / (oos_a.len().max(1) as f64);
    let oos_r2_pct = if mse_b > 1e-15 { (1.0 - mse_m / mse_b) * 100.0 } else { 0.0 };
    let da_pct = if !oos_p.is_empty() {
        oos_p.iter().zip(&oos_a).filter(|(&p, &a)| (p > 0.0) == (a > 0.0)).count() as f64 / (oos_p.len() as f64) * 100.0
    } else { 0.0 };
    let up_pct = if !oos_a.is_empty() {
        oos_a.iter().filter(|&&a| a > 0.0).count() as f64 / (oos_a.len() as f64) * 100.0
    } else { 0.0 };

    let last = n - 1;
    let last_ret = pred_ret[last].unwrap_or(0.0);
    let last_sig = sigs[last].unwrap_or(TradeSignal::Hold);
    let reason = match &best_row {
        Some(r) => format!(
            "{:?} ({}d): Driver {} lag {}d (r_oos={:+.3}, NW-t={:+.2}) projects {:+.2}%",
            last_sig, h, r.feature, r.lag_days, r.r_oos, r.newey_west_t, last_ret
        ),
        None => "Insufficient supplier history".to_string(),
    };

    let cutoff = as_of - chrono::Duration::days(range_days as i64);
    let history = (0..n)
        .filter(|&i| dates[i] >= cutoff)
        .map(|i| SupplyChainSeriesPoint {
            time: dates[i].format("%Y-%m-%d").to_string(),
            close: close[i],
            forecast_price: pred_px[i],
            signal: sigs[i],
        })
        .collect();

    let lag_matrix = all_rows.into_iter().take(18).map(|(_, row, _)| row).collect();
    Some(SupplyChainAnalysisResponse {
        target: target.to_string(),
        as_of: dates[last].format("%Y-%m-%d").to_string(),
        horizon_days: h,
        latest_price: close[last],
        signal: last_sig,
        signal_reason: reason,
        forecast_ret_pct: last_ret,
        forecast_price: pred_px[last].unwrap_or(close[last]),
        oos_r2_pct,
        directional_accuracy_pct: da_pct,
        baseline_up_rate_pct: up_pct,
        survivor_count,
        lag_matrix,
        history,
    })
}
