//! Pure time-series builder helpers for the 6-Tank Liquidity & Capital Flow Atlas (Phase 363).
//! Enforces strict zero-lookahead (`date <= as_of`) and explicit as-is vs derived provenance.

use std::collections::BTreeMap;
use chrono::NaiveDate;

use crate::core::atlas_types::{AtlasPoint, AtlasSeries};
use crate::core::observation_types::{DataProcessingLayer, MetricObservation};
use crate::core::series_catalog::SeriesCatalog;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    align_to_friday_date, FedNetLiquidityPoint, ImpliedNetFlowPoint,
};

fn fmt_d(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn map_to_points(map: BTreeMap<NaiveDate, f64>) -> Vec<AtlasPoint> {
    map.into_iter().map(|(d, value)| AtlasPoint { time: fmt_d(d), value }).collect()
}

fn stock_win_delta(obs: &[MetricObservation], win_start: NaiveDate, divisor: f64) -> Option<f64> {
    obs.last()
        .filter(|o| o.observation_ts_utc.date_naive() > win_start)
        .and_then(|end| {
            obs.iter()
                .rev()
                .find(|o| o.observation_ts_utc.date_naive() <= win_start)
                .map(|st| (end.value - st.value) / divisor)
        })
}

pub fn lag_warning(s: &AtlasSeries, max_lag: i64) -> Option<String> {
    match (s.stale_lag_days, s.latest_date.as_deref()) {
        (Some(lag), Some(d)) if lag > max_lag => {
            Some(format!("UNPRINTED LAG ({}): latest {} ({}d behind as_of)", s.label, d, lag))
        }
        (None, _) => Some(format!("NO DATA IN RANGE ({})", s.label)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn weekly_flow_series(
    sid: &str, label: &str, scale: &str, color: &str, pts: &[ImpliedNetFlowPoint],
    use_delta_aum: bool, start_date: NaiveDate, win_start: NaiveDate, as_of: NaiveDate, source: &str,
) -> AtlasSeries {
    let (mut wk_map, mut latest_d, mut win_sum, mut win_cnt) = (BTreeMap::new(), None::<NaiveDate>, 0.0, 0usize);
    for p in pts {
        let d = p.timestamp_utc.date_naive();
        let v = if use_delta_aum { p.delta_v_usd } else { p.implied_net_flow_usd };
        if d > as_of || !v.is_finite() { continue; }
        latest_d = Some(latest_d.map_or(d, |cur| cur.max(d)));
        if d > win_start { win_sum += v; win_cnt += 1; }
        let fri = align_to_friday_date(d).min(as_of);
        if fri >= start_date { *wk_map.entry(fri).or_insert(0.0) += v / 1e9; }
    }
    let points = map_to_points(wk_map);
    AtlasSeries {
        series_id: sid.to_string(),
        label: label.to_string(),
        chart_kind: if use_delta_aum { "line" } else { "histogram" }.to_string(),
        price_scale: scale.to_string(),
        color: color.to_string(),
        unit: "$B".to_string(),
        data_layer: DataProcessingLayer::DerivedAdjusted,
        raw_value_preserved: true,
        source: source.to_string(),
        latest_date: latest_d.map(fmt_d),
        stale_lag_days: latest_d.map(|d| (as_of - d).num_days()),
        latest_value: points.last().map(|p| p.value),
        window_delta: (win_cnt > 0).then_some(win_sum / 1e9),
        note: Some("Computed from raw SSGA unadjusted shares with split/glitch guard".to_string()),
        points,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn weekly_obs_sum_series(
    sid: &str, label: &str, scale: &str, color: &str, divisor: f64, obs: &[MetricObservation],
    layer: DataProcessingLayer, source: &str, start_date: NaiveDate, win_start: NaiveDate, as_of: NaiveDate,
) -> AtlasSeries {
    let (mut wk_map, mut latest_d, mut win_sum, mut win_cnt) = (BTreeMap::new(), None::<NaiveDate>, 0.0, 0usize);
    for o in obs {
        let d = o.observation_ts_utc.date_naive();
        if d > as_of || !o.value.is_finite() { continue; }
        latest_d = Some(latest_d.map_or(d, |cur| cur.max(d)));
        if d > win_start { win_sum += o.value; win_cnt += 1; }
        let fri = align_to_friday_date(d).min(as_of);
        if fri >= start_date { *wk_map.entry(fri).or_insert(0.0) += o.value / divisor; }
    }
    let points = map_to_points(wk_map);
    AtlasSeries {
        series_id: sid.to_string(),
        label: label.to_string(),
        chart_kind: "histogram".to_string(),
        price_scale: scale.to_string(),
        color: color.to_string(),
        unit: "$B".to_string(),
        data_layer: layer,
        raw_value_preserved: true,
        source: source.to_string(),
        latest_date: latest_d.map(fmt_d),
        stale_lag_days: latest_d.map(|d| (as_of - d).num_days()),
        latest_value: points.last().map(|p| p.value),
        window_delta: (win_cnt > 0).then_some(win_sum / divisor),
        note: None,
        points,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn weekly_stock_delta_series(
    sid: &str, label: &str, scale: &str, color: &str, divisor: f64, obs: &[MetricObservation],
    catalog: &SeriesCatalog, start_date: NaiveDate, win_start: NaiveDate, as_of: NaiveDate,
) -> AtlasSeries {
    let (mut eow, mut latest_d) = (BTreeMap::new(), None::<NaiveDate>);
    for o in obs {
        let d = o.observation_ts_utc.date_naive();
        if d <= as_of && o.value.is_finite() {
            latest_d = Some(latest_d.map_or(d, |cur| cur.max(d)));
            eow.insert(align_to_friday_date(d).min(as_of), o.value);
        }
    }
    let (mut points, mut prev) = (Vec::new(), None::<f64>);
    for (wk, v) in eow {
        if let Some(p) = prev {
            if wk >= start_date {
                points.push(AtlasPoint { time: fmt_d(wk), value: (v - p) / divisor });
            }
        }
        prev = Some(v);
    }
    let meta = catalog.get("stablecoin:ALL:mcap_usd");
    AtlasSeries {
        series_id: sid.to_string(),
        label: label.to_string(),
        chart_kind: "histogram".to_string(),
        price_scale: scale.to_string(),
        color: color.to_string(),
        unit: "$B".to_string(),
        data_layer: meta.map(|m| m.data_layer).unwrap_or(DataProcessingLayer::RawAsIs),
        raw_value_preserved: true,
        source: meta.map(|m| m.primary_source.clone()).unwrap_or_else(|| "defillama".to_string()),
        latest_date: latest_d.map(fmt_d),
        stale_lag_days: latest_d.map(|d| (as_of - d).num_days()),
        latest_value: points.last().map(|p| p.value),
        window_delta: stock_win_delta(obs, win_start, divisor),
        note: None,
        points,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn obs_level_series(
    sid: &str, label: &str, scale: &str, color: &str, divisor: f64, unit: &str,
    obs: &[MetricObservation], catalog: &SeriesCatalog, start_date: NaiveDate, win_start: NaiveDate, as_of: NaiveDate,
) -> AtlasSeries {
    let meta = catalog.get(sid);
    let (mut by_d, mut latest_d) = (BTreeMap::new(), None::<NaiveDate>);
    for o in obs {
        let d = o.observation_ts_utc.date_naive();
        if d <= as_of && o.value.is_finite() {
            latest_d = Some(latest_d.map_or(d, |cur| cur.max(d)));
            if d >= start_date { by_d.insert(d, o.value / divisor); }
        }
    }
    AtlasSeries {
        series_id: sid.to_string(),
        label: label.to_string(),
        chart_kind: "line".to_string(),
        price_scale: scale.to_string(),
        color: color.to_string(),
        unit: unit.to_string(),
        data_layer: meta.map(|m| m.data_layer).unwrap_or(DataProcessingLayer::RawAsIs),
        raw_value_preserved: meta.map(|m| m.raw_value_preserved).unwrap_or(true),
        source: meta.map(|m| m.primary_source.clone()).unwrap_or_else(|| "unknown".to_string()),
        latest_date: latest_d.map(fmt_d),
        stale_lag_days: latest_d.map(|d| (as_of - d).num_days()),
        latest_value: obs.last().map(|o| o.value / divisor),
        window_delta: stock_win_delta(obs, win_start, divisor),
        note: meta.and_then(|m| m.adjustment_note.clone()),
        points: map_to_points(by_d),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn candle_series(
    sid: &str, label: &str, scale: &str, color: &str, cs: &[AggregatorCandle],
    layer: DataProcessingLayer, start_date: NaiveDate, win_start: NaiveDate, as_of: NaiveDate,
) -> AtlasSeries {
    let (mut by_d, mut latest_d) = (BTreeMap::new(), None::<NaiveDate>);
    for c in cs {
        let d = c.timestamp_utc.date_naive();
        if d <= as_of && c.close.is_finite() {
            latest_d = Some(latest_d.map_or(d, |cur| cur.max(d)));
            if d >= start_date { by_d.insert(d, c.close); }
        }
    }
    let win_pct = cs
        .last()
        .filter(|c| c.timestamp_utc.date_naive() > win_start)
        .and_then(|end| {
            cs.iter()
                .rev()
                .find(|c| c.timestamp_utc.date_naive() <= win_start && c.close > 0.0)
                .map(|st| ((end.close / st.close) - 1.0) * 100.0)
        });
    AtlasSeries {
        series_id: sid.to_string(),
        label: label.to_string(),
        chart_kind: "line".to_string(),
        price_scale: scale.to_string(),
        color: color.to_string(),
        unit: "$".to_string(),
        data_layer: layer,
        raw_value_preserved: true,
        source: cs.last().map(|c| c.source.clone()).unwrap_or_else(|| "yahoo".to_string()),
        latest_date: latest_d.map(fmt_d),
        stale_lag_days: latest_d.map(|d| (as_of - d).num_days()),
        latest_value: cs.last().map(|c| c.close),
        window_delta: win_pct,
        note: Some("window_delta shown in % price return".to_string()),
        points: map_to_points(by_d),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn daily_avg_funding_series(
    sid: &str, label: &str, scale: &str, color: &str, obs: &[MetricObservation],
    start_date: NaiveDate, win_start: NaiveDate, as_of: NaiveDate,
) -> AtlasSeries {
    let (mut by_d, mut latest_d, mut w_sum, mut w_cnt) = (BTreeMap::new(), None::<NaiveDate>, 0.0, 0usize);
    for o in obs {
        let d = o.observation_ts_utc.date_naive();
        if d > as_of || !o.value.is_finite() { continue; }
        let bps = o.value * 10_000.0;
        latest_d = Some(latest_d.map_or(d, |cur| cur.max(d)));
        if d > win_start { w_sum += bps; w_cnt += 1; }
        if d >= start_date {
            let e: &mut (f64, usize) = by_d.entry(d).or_insert((0.0, 0));
            e.0 += bps;
            e.1 += 1;
        }
    }
    let points: Vec<AtlasPoint> = by_d
        .into_iter()
        .map(|(d, (s, c))| AtlasPoint { time: fmt_d(d), value: s / c as f64 })
        .collect();
    AtlasSeries {
        series_id: sid.to_string(),
        label: label.to_string(),
        chart_kind: "histogram".to_string(),
        price_scale: scale.to_string(),
        color: color.to_string(),
        unit: "bps".to_string(),
        data_layer: DataProcessingLayer::RawAsIs,
        raw_value_preserved: true,
        source: obs.last().map(|o| o.provenance.source.clone()).unwrap_or_else(|| "binance_futures".to_string()),
        latest_date: latest_d.map(fmt_d),
        stale_lag_days: latest_d.map(|d| (as_of - d).num_days()),
        latest_value: points.last().map(|p| p.value),
        window_delta: (w_cnt > 0).then_some(w_sum / w_cnt as f64),
        note: Some("window_delta reports window average 8h funding rate in bps".to_string()),
        points,
    }
}

pub fn fed_ex_rrp_series(
    pts: &[FedNetLiquidityPoint], start_date: NaiveDate, win_start: NaiveDate, as_of: NaiveDate,
) -> AtlasSeries {
    let (mut by_d, mut latest_d) = (BTreeMap::new(), None::<NaiveDate>);
    for p in pts {
        let d = p.timestamp_utc.date_naive();
        if d <= as_of {
            latest_d = Some(latest_d.map_or(d, |cur| cur.max(d)));
            by_d.insert(d, (p.fed_assets_usd - p.tga_balance_usd) / 1e12);
        }
    }
    let win_delta_b = by_d
        .iter()
        .next_back()
        .filter(|(d, _)| **d > win_start)
        .and_then(|(_, &end_t)| by_d.range(..=win_start).next_back().map(|(_, &st_t)| (end_t - st_t) * 1000.0));
    let points: Vec<AtlasPoint> = by_d
        .range(start_date..)
        .map(|(d, &value)| AtlasPoint { time: fmt_d(*d), value })
        .collect();
    AtlasSeries {
        series_id: "derived:US:FED_WALCL_MINUS_TGA_USD".to_string(),
        label: "Fed WALCL - TGA ($T)".to_string(),
        chart_kind: "area".to_string(),
        price_scale: "right".to_string(),
        color: "#2962ff".to_string(),
        unit: "$T".to_string(),
        data_layer: DataProcessingLayer::DerivedAdjusted,
        raw_value_preserved: true,
        source: "fred_api+treasury_fiscaldata".to_string(),
        latest_date: latest_d.map(fmt_d),
        stale_lag_days: latest_d.map(|d| (as_of - d).num_days()),
        latest_value: points.last().map(|p| p.value),
        window_delta: win_delta_b,
        note: Some("window_delta shown in $B; Wednesday WALCL aligned with same-day TGA".to_string()),
        points,
    }
}
