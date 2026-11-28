//! SMH (VanEck Semiconductor ETF) 5-Day Predictive & Risk Envelope Engine.
//! Uses strictly causal (zero-lookahead) expanding Ridge regression on empirically
//! validated lag drivers (`^VXN - ^VIX` lag 2d, `ZT=F` Z63 lag 42d, `^FVX` Z63 lag 42d)
//! paired with EWMA(0.94) 95% volatility envelopes and explicit BUY / HOLD / SELL signals.

use std::collections::BTreeMap;
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::core::types::AggregatorCandle;
use crate::features::analytics::smh_predictive_math::{
    align_closes, classify_signal, fit_predict_ridge3, lag_vec, max_drawdown_pct, rolling_z,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TradeSignal {
    Buy,
    Hold,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmhSeriesPoint {
    pub time: String,
    pub close: f64,
    pub predicted_5d_price: Option<f64>,
    pub upper_95_price: Option<f64>,
    pub lower_95_price: Option<f64>,
    pub predicted_5d_ret_pct: Option<f64>,
    pub signal: Option<TradeSignal>,
    pub ann_vol_pct: Option<f64>,
    pub vxn_minus_vix: Option<f64>,
    pub ztf_z63_lag42: Option<f64>,
    pub fvx_z63_lag42: Option<f64>,
    pub move_z63: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmhForwardConePoint {
    pub time: String,
    pub projected_price: f64,
    pub upper_95: f64,
    pub lower_95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmhOosMetrics {
    pub oos_samples: usize,
    pub oos_r2_vs_mean_pct: f64,
    pub directional_accuracy_pct: f64,
    pub baseline_up_rate_pct: f64,
    pub strategy_sharpe: f64,
    pub buy_hold_sharpe: f64,
    pub strategy_max_dd_pct: f64,
    pub buy_hold_max_dd_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmhPredictiveResponse {
    pub symbol: String,
    pub as_of: String,
    pub latest_price: f64,
    pub signal: TradeSignal,
    pub signal_confidence_pct: f64,
    pub signal_reason: String,
    pub recommended_allocation_pct: f64,
    pub forecast_5d_ret_pct: f64,
    pub forecast_5d_price: f64,
    pub upper_95_5d_price: f64,
    pub lower_95_5d_price: f64,
    pub ann_vol_pct: f64,
    pub drivers: SmhDriversSnapshot,
    pub oos_metrics: SmhOosMetrics,
    pub history: Vec<SmhSeriesPoint>,
    pub forward_cone: Vec<SmhForwardConePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmhDriversSnapshot {
    pub vxn_minus_vix_lag2: f64,
    pub ztf_z63_lag42: f64,
    pub fvx_z63_lag42: f64,
    pub move_z63: f64,
    pub smh_vs_asml_z63_lag21: f64,
}

pub struct SmhPredictiveInputs<'a> {
    pub as_of: NaiveDate,
    pub range_days: usize,
    pub smh: &'a [AggregatorCandle],
    pub vxn: &'a [AggregatorCandle],
    pub vix: &'a [AggregatorCandle],
    pub ztf: &'a [AggregatorCandle],
    pub fvx: &'a [AggregatorCandle],
    pub asml: &'a [AggregatorCandle],
    pub move_idx: &'a [AggregatorCandle],
}

pub fn evaluate_smh_predictive(inputs: &SmhPredictiveInputs<'_>) -> Option<SmhPredictiveResponse> {
    let mut smh_map = BTreeMap::new();
    for c in inputs.smh {
        let d = c.timestamp_utc.date_naive();
        if d <= inputs.as_of && c.close.is_finite() && c.close > 0.0 {
            smh_map.insert(d, c.close);
        }
    }
    if smh_map.len() < 320 {
        return None;
    }
    let dates: Vec<NaiveDate> = smh_map.keys().copied().collect();
    let close: Vec<f64> = smh_map.values().copied().collect();
    let n = dates.len();

    let vxn = align_closes(&dates, inputs.vxn);
    let vix = align_closes(&dates, inputs.vix);
    let ztf = align_closes(&dates, inputs.ztf);
    let fvx = align_closes(&dates, inputs.fvx);
    let asml = align_closes(&dates, inputs.asml);
    let move_idx = align_closes(&dates, inputs.move_idx);

    let vxn_vix: Vec<f64> = vxn.iter().zip(vix.iter()).map(|(a, b)| a - b).collect();
    let vxn_vix_lag2 = lag_vec(&vxn_vix, 2);
    let ztf_z63_lag42 = lag_vec(&rolling_z(&ztf, 63), 42);
    let fvx_z63_lag42 = lag_vec(&rolling_z(&fvx, 63), 42);
    let move_z63 = rolling_z(&move_idx, 63);
    let smh_asml_spread: Vec<f64> = close.iter().zip(asml.iter())
        .map(|(&s, &a)| if s > 0.0 && a > 0.0 && a.is_finite() { (s / a).ln() } else { f64::NAN })
        .collect();
    let smh_asml_z63_lag21 = lag_vec(&rolling_z(&smh_asml_spread, 63), 21);

    // EWMA(0.94) variance on daily log returns
    let mut ewma_var = vec![f64::NAN; n];
    let mut s2 = 0.0004_f64;
    for i in 1..n {
        let r = (close[i] / close[i - 1]).ln();
        s2 = 0.94 * s2 + 0.06 * r * r;
        ewma_var[i] = s2;
    }

    // 5-day forward log return target (only valid up to n - 6)
    let mut y5 = vec![f64::NAN; n];
    for i in 0..n.saturating_sub(5) {
        y5[i] = (close[i + 5] / close[i]).ln();
    }

    let mut pred_5d_log = vec![None; n];
    let mut pred_5d_price = vec![None; n];
    let mut upper_95 = vec![None; n];
    let mut lower_95 = vec![None; n];
    let mut signals = vec![None; n];

    let mut x_pool: Vec<[f64; 3]> = Vec::new();
    let mut y_pool: Vec<f64> = Vec::new();
    let mut oos_preds = Vec::new();
    let mut oos_actuals = Vec::new();
    let mut oos_means = Vec::new();
    let mut strat_rets = Vec::new();
    let mut bh_rets = Vec::new();
    let mut next_rebal = 0usize;

    for t in 0..n {
        if t >= 5 {
            let k = t - 5;
            let row = [vxn_vix_lag2[k], ztf_z63_lag42[k], fvx_z63_lag42[k]];
            if row.iter().all(|v| v.is_finite()) && y5[k].is_finite() {
                x_pool.push(row);
                y_pool.push(y5[k]);
            }
        }
        let x_cur = [vxn_vix_lag2[t], ztf_z63_lag42[t], fvx_z63_lag42[t]];
        if x_pool.len() >= 300 && x_cur.iter().all(|v| v.is_finite()) && ewma_var[t].is_finite() {
            let p_log = fit_predict_ridge3(&x_pool, &y_pool, x_cur, 150.0);
            let p_ret_pct = (p_log.exp() - 1.0) * 100.0;
            let p_price = close[t] * p_log.exp();
            let sig_5d = (5.0 * ewma_var[t]).sqrt();
            let u95 = p_price * (1.96 * sig_5d).exp();
            let l95 = p_price * (-1.96 * sig_5d).exp();
            let mz = if move_z63[t].is_finite() { move_z63[t] } else { 0.0 };
            let (sig, _, _) = classify_signal(p_ret_pct, sig_5d, mz);

            pred_5d_log[t] = Some(p_log);
            pred_5d_price[t] = Some(p_price);
            upper_95[t] = Some(u95);
            lower_95[t] = Some(l95);
            signals[t] = Some(sig);

            if y5[t].is_finite() {
                let y_mean = y_pool.iter().sum::<f64>() / (y_pool.len() as f64);
                oos_preds.push(p_log);
                oos_actuals.push(y5[t]);
                oos_means.push(y_mean);
                if t >= next_rebal {
                    let ann_vol = (252.0 * ewma_var[t]).sqrt() * 100.0;
                    let w = match sig {
                        TradeSignal::Buy => (22.0 / ann_vol.max(5.0)).clamp(0.25, 1.0),
                        TradeSignal::Hold => (18.0 / ann_vol.max(5.0)).clamp(0.15, 0.85),
                        TradeSignal::Sell => 0.0,
                    };
                    let simple_r = y5[t].exp() - 1.0;
                    strat_rets.push(w * simple_r);
                    bh_rets.push(simple_r);
                    next_rebal = t + 5;
                }
            }
        }
    }

    let last = n - 1;
    let last_p_ret_pct = pred_5d_log[last].map(|v| (v.exp() - 1.0) * 100.0).unwrap_or(0.0);
    let last_sig5d = (5.0 * ewma_var[last]).sqrt();
    let last_mz = if move_z63[last].is_finite() { move_z63[last] } else { 0.0 };
    let (last_sig, last_conf, last_reason) = classify_signal(last_p_ret_pct, last_sig5d, last_mz);
    let last_ann_vol = (252.0 * ewma_var[last]).sqrt() * 100.0;
    let rec_alloc = match last_sig {
        TradeSignal::Buy => (22.0 / last_ann_vol.max(5.0)).clamp(0.25, 1.0) * 100.0,
        TradeSignal::Hold => (18.0 / last_ann_vol.max(5.0)).clamp(0.15, 0.85) * 100.0,
        TradeSignal::Sell => 0.0,
    };

    let mse_m = oos_preds.iter().zip(&oos_actuals).map(|(p, a)| (a - p).powi(2)).sum::<f64>()
        / (oos_actuals.len().max(1) as f64);
    let mse_b = oos_means.iter().zip(&oos_actuals).map(|(m, a)| (a - m).powi(2)).sum::<f64>()
        / (oos_actuals.len().max(1) as f64);
    let r2_pct = if mse_b > 1e-15 { (1.0 - mse_m / mse_b) * 100.0 } else { 0.0 };
    let da_pct = if !oos_preds.is_empty() {
        oos_preds.iter().zip(&oos_actuals).filter(|(&p, &a)| (p > 0.0) == (a > 0.0)).count() as f64
            / (oos_preds.len() as f64) * 100.0
    } else {
        0.0
    };
    let up_rate = if !oos_actuals.is_empty() {
        oos_actuals.iter().filter(|&&a| a > 0.0).count() as f64 / (oos_actuals.len() as f64) * 100.0
    } else {
        0.0
    };
    let sharpe = |rs: &[f64]| -> f64 {
        if rs.len() < 4 {
            return 0.0;
        }
        let m = rs.iter().sum::<f64>() / (rs.len() as f64);
        let sd = (rs.iter().map(|r| (r - m).powi(2)).sum::<f64>() / (rs.len() as f64)).sqrt();
        if sd > 1e-12 { (m / sd) * (252.0_f64 / 5.0).sqrt() } else { 0.0 }
    };

    let cutoff = inputs.as_of - Duration::days(inputs.range_days as i64);
    let history: Vec<SmhSeriesPoint> = (0..n)
        .filter(|&i| dates[i] >= cutoff)
        .map(|i| SmhSeriesPoint {
            time: dates[i].format("%Y-%m-%d").to_string(),
            close: close[i],
            predicted_5d_price: pred_5d_price[i],
            upper_95_price: upper_95[i],
            lower_95_price: lower_95[i],
            predicted_5d_ret_pct: pred_5d_log[i].map(|v| (v.exp() - 1.0) * 100.0),
            signal: signals[i],
            ann_vol_pct: if ewma_var[i].is_finite() { Some((252.0 * ewma_var[i]).sqrt() * 100.0) } else { None },
            vxn_minus_vix: if vxn_vix_lag2[i].is_finite() { Some(vxn_vix_lag2[i]) } else { None },
            ztf_z63_lag42: if ztf_z63_lag42[i].is_finite() { Some(ztf_z63_lag42[i]) } else { None },
            fvx_z63_lag42: if fvx_z63_lag42[i].is_finite() { Some(fvx_z63_lag42[i]) } else { None },
            move_z63: if move_z63[i].is_finite() { Some(move_z63[i]) } else { None },
        })
        .collect();

    // 5 trading-day forward cone starting from `dates[last]`
    let p_log_last = pred_5d_log[last].unwrap_or(0.0);
    let mut forward_cone = Vec::with_capacity(6);
    let mut d_cur = dates[last];
    for step in 0..=5usize {
        if step > 0 {
            d_cur += Duration::days(1);
            while d_cur.weekday().num_days_from_monday() >= 5 {
                d_cur += Duration::days(1);
            }
        }
        let frac = (step as f64) / 5.0;
        let proj = close[last] * (p_log_last * frac).exp();
        let sig_s = ((step as f64) * ewma_var[last]).sqrt();
        forward_cone.push(SmhForwardConePoint {
            time: d_cur.format("%Y-%m-%d").to_string(),
            projected_price: proj,
            upper_95: proj * (1.96 * sig_s).exp(),
            lower_95: proj * (-1.96 * sig_s).exp(),
        });
    }

    Some(SmhPredictiveResponse {
        symbol: "SMH".to_string(),
        as_of: dates[last].format("%Y-%m-%d").to_string(),
        latest_price: close[last],
        signal: last_sig,
        signal_confidence_pct: last_conf,
        signal_reason: last_reason,
        recommended_allocation_pct: rec_alloc,
        forecast_5d_ret_pct: last_p_ret_pct,
        forecast_5d_price: pred_5d_price[last].unwrap_or(close[last]),
        upper_95_5d_price: upper_95[last].unwrap_or(close[last]),
        lower_95_5d_price: lower_95[last].unwrap_or(close[last]),
        ann_vol_pct: last_ann_vol,
        drivers: SmhDriversSnapshot {
            vxn_minus_vix_lag2: if vxn_vix_lag2[last].is_finite() { vxn_vix_lag2[last] } else { 0.0 },
            ztf_z63_lag42: if ztf_z63_lag42[last].is_finite() { ztf_z63_lag42[last] } else { 0.0 },
            fvx_z63_lag42: if fvx_z63_lag42[last].is_finite() { fvx_z63_lag42[last] } else { 0.0 },
            move_z63: last_mz,
            smh_vs_asml_z63_lag21: if smh_asml_z63_lag21[last].is_finite() { smh_asml_z63_lag21[last] } else { 0.0 },
        },
        oos_metrics: SmhOosMetrics {
            oos_samples: oos_actuals.len(),
            oos_r2_vs_mean_pct: r2_pct,
            directional_accuracy_pct: da_pct,
            baseline_up_rate_pct: up_rate,
            strategy_sharpe: sharpe(&strat_rets),
            buy_hold_sharpe: sharpe(&bh_rets),
            strategy_max_dd_pct: max_drawdown_pct(&strat_rets),
            buy_hold_max_dd_pct: max_drawdown_pct(&bh_rets),
        },
        history,
        forward_cone,
    })
}
