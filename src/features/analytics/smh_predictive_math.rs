//! Pure mathematical helpers & unit tests for the SMH 5-Day Predictive Engine.

use std::collections::BTreeMap;
use chrono::NaiveDate;

use crate::core::types::AggregatorCandle;
use crate::features::analytics::smh_predictive::TradeSignal;

pub(crate) fn align_closes(dates: &[NaiveDate], candles: &[AggregatorCandle]) -> Vec<f64> {
    let mut map = BTreeMap::new();
    for c in candles {
        let d = c.timestamp_utc.date_naive();
        if c.close.is_finite() && c.close > 0.0 {
            map.insert(d, c.close);
        }
    }
    let pts: Vec<(NaiveDate, f64)> = map.into_iter().collect();
    let mut out = vec![f64::NAN; dates.len()];
    let mut j = 0usize;
    let mut has = false;
    for (i, &cur_d) in dates.iter().enumerate() {
        while j < pts.len() && pts[j].0 <= cur_d {
            has = true;
            j += 1;
        }
        if has && j > 0 {
            out[i] = pts[j - 1].1;
        }
    }
    out
}

pub(crate) fn rolling_z(arr: &[f64], w: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; arr.len()];
    if arr.len() < w {
        return out;
    }
    for i in (w - 1)..arr.len() {
        let seg = &arr[i + 1 - w..=i];
        if seg.iter().all(|v| v.is_finite()) {
            let mean = seg.iter().sum::<f64>() / (w as f64);
            let var = seg.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (w as f64);
            let sd = var.sqrt();
            if sd > 1e-12 {
                out[i] = (arr[i] - mean) / sd;
            }
        }
    }
    out
}

pub(crate) fn lag_vec(arr: &[f64], k: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; arr.len()];
    if k < arr.len() {
        out[k..].copy_from_slice(&arr[..arr.len() - k]);
    }
    out
}

fn solve_3x3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> [f64; 3] {
    for col in 0..3 {
        let mut piv = col;
        for r in (col + 1)..3 {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let diag = a[col][col];
        if diag.abs() < 1e-12 {
            continue;
        }
        for r in (col + 1)..3 {
            let f = a[r][col] / diag;
            for c in col..3 {
                a[r][c] -= f * a[col][c];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = [0.0; 3];
    for r in (0..3).rev() {
        let mut s = b[r];
        for c in (r + 1)..3 {
            s -= a[r][c] * x[c];
        }
        x[r] = if a[r][r].abs() > 1e-12 { s / a[r][r] } else { 0.0 };
    }
    x
}

pub(crate) fn fit_predict_ridge3(x_tr: &[[f64; 3]], y_tr: &[f64], x_te: [f64; 3], alpha: f64) -> f64 {
    let n = x_tr.len() as f64;
    if n < 30.0 {
        return 0.0;
    }
    let mut mu = [0.0; 3];
    for row in x_tr {
        for j in 0..3 {
            mu[j] += row[j];
        }
    }
    for j in 0..3 {
        mu[j] /= n;
    }
    let mut sd = [0.0; 3];
    for row in x_tr {
        for j in 0..3 {
            sd[j] += (row[j] - mu[j]).powi(2);
        }
    }
    for j in 0..3 {
        sd[j] = (sd[j] / n).sqrt();
        if sd[j] < 1e-12 {
            sd[j] = 1.0;
        }
    }
    let y_mu = y_tr.iter().sum::<f64>() / n;
    let mut xtx = [[0.0; 3]; 3];
    let mut xty = [0.0; 3];
    for (row, &yv) in x_tr.iter().zip(y_tr.iter()) {
        let z = [(row[0] - mu[0]) / sd[0], (row[1] - mu[1]) / sd[1], (row[2] - mu[2]) / sd[2]];
        let dy = yv - y_mu;
        for i in 0..3 {
            xty[i] += z[i] * dy;
            for j in 0..3 {
                xtx[i][j] += z[i] * z[j];
            }
        }
    }
    for i in 0..3 {
        xtx[i][i] += alpha;
    }
    let w = solve_3x3(xtx, xty);
    let z_te = [(x_te[0] - mu[0]) / sd[0], (x_te[1] - mu[1]) / sd[1], (x_te[2] - mu[2]) / sd[2]];
    y_mu + w[0] * z_te[0] + w[1] * z_te[1] + w[2] * z_te[2]
}

pub(crate) fn classify_signal(pred_ret_pct: f64, sigma_5d: f64, move_z63: f64) -> (TradeSignal, f64, String) {
    let pred_log = (1.0 + pred_ret_pct / 100.0).ln();
    let z_sig = if sigma_5d > 1e-9 { pred_log / sigma_5d } else { 0.0 };
    if move_z63.is_finite() && move_z63 >= 2.2 {
        let conf = (55.0 + (move_z63 - 2.2) * 15.0).clamp(55.0, 92.0);
        return (
            TradeSignal::Sell,
            conf,
            format!("SELL / Defensive: Bond vol stress elevated (^MOVE Z63 = {:.2} >= 2.20)", move_z63),
        );
    }
    if pred_ret_pct >= 0.35 && z_sig >= 0.10 {
        let conf = (52.0 + z_sig * 75.0).clamp(52.0, 90.0);
        return (
            TradeSignal::Buy,
            conf,
            format!("BUY: +5d Lag-Ridge forecast {:+.2}% (risk-adj z={:+.2}) with contained ^MOVE stress", pred_ret_pct, z_sig),
        );
    }
    if pred_ret_pct <= -0.15 || z_sig <= -0.05 {
        let conf = (52.0 + z_sig.abs() * 75.0).clamp(52.0, 90.0);
        return (
            TradeSignal::Sell,
            conf,
            format!("SELL / Reduce: Negative 5d Lag-Ridge forecast {:+.2}% (risk-adj z={:+.2})", pred_ret_pct, z_sig),
        );
    }
    let conf = (50.0 + (0.35 - pred_ret_pct.abs()).max(0.0) * 40.0).clamp(50.0, 75.0);
    (
        TradeSignal::Hold,
        conf,
        format!("HOLD: Neutral 5d Lag-Ridge forecast {:+.2}% (within -0.15%..+0.35% hysteresis band)", pred_ret_pct),
    )
}

pub(crate) fn max_drawdown_pct(rets: &[f64]) -> f64 {
    let mut eq = 1.0_f64;
    let mut peak = 1.0_f64;
    let mut mdd = 0.0_f64;
    for &r in rets {
        eq *= 1.0 + r;
        if eq > peak {
            peak = eq;
        }
        let dd = (eq / peak - 1.0) * 100.0;
        if dd < mdd {
            mdd = dd;
        }
    }
    mdd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{IngestionMode, TimeFrame};
    use crate::features::analytics::smh_predictive::{evaluate_smh_predictive, SmhPredictiveInputs};
    use chrono::{Duration, TimeZone};

    fn make_series(start: NaiveDate, n: usize, f: impl Fn(usize) -> f64) -> Vec<AggregatorCandle> {
        (0..n).map(|i| {
            let d = start + Duration::days(i as i64);
            let ts = chrono::Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap());
            let px = f(i);
            AggregatorCandle {
                time: ts.timestamp(),
                timestamp_utc: ts,
                canonical_symbol: "TEST".to_string(),
                timeframe: TimeFrame::D1,
                open: px,
                high: px * 1.01,
                low: px * 0.99,
                close: px,
                volume_base: 1_000.0,
                volume_quote: 1_000.0 * px,
                source: "test".to_string(),
                raw_symbol: "TEST".to_string(),
                ingestion_mode: IngestionMode::RestBatch,
                is_settled: true,
                sequence_id: None,
                taker_buy_volume_base: None,
                taker_buy_volume_quote: None,
                vwap: None,
                trade_count: None,
                price_type: "last".to_string(),
                raw_metadata: None,
            }
        }).collect()
    }

    #[test]
    fn test_smh_predictive_produces_signal_and_forward_cone() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let n = 550;
        let smh = make_series(start, n, |i| 200.0 + (i as f64) * 0.3 + ((i as f64) * 0.2).sin() * 4.0);
        let vxn = make_series(start, n, |i| 20.0 + ((i as f64) * 0.15).cos() * 2.0);
        let vix = make_series(start, n, |i| 15.0 + ((i as f64) * 0.15).sin() * 1.5);
        let ztf = make_series(start, n, |i| 102.0 + ((i as f64) * 0.1).sin() * 0.8);
        let fvx = make_series(start, n, |i| 4.1 - ((i as f64) * 0.1).sin() * 0.2);
        let asml = make_series(start, n, |i| 800.0 + (i as f64) * 0.5);
        let mv = make_series(start, n, |i| 95.0 + ((i as f64) * 0.1).sin() * 5.0);

        let as_of = start + Duration::days((n - 1) as i64);
        let res = evaluate_smh_predictive(&SmhPredictiveInputs {
            as_of,
            range_days: 180,
            smh: &smh,
            vxn: &vxn,
            vix: &vix,
            ztf: &ztf,
            fvx: &fvx,
            asml: &asml,
            move_idx: &mv,
        }).expect("should evaluate");

        assert_eq!(res.forward_cone.len(), 6);
        assert!(res.upper_95_5d_price > res.forecast_5d_price);
        assert!(res.lower_95_5d_price < res.forecast_5d_price);
        assert!(res.oos_metrics.oos_samples > 50);
    }
}
