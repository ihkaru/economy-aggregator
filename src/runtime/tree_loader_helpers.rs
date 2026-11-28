//! Helper Aggregators & Lineage Builders for `tree_loader.rs` (Phase 359).

use chrono::NaiveDate;

use crate::core::diagnostic_types::SourceLineageEntry;
use crate::core::observation_types::{DataProcessingLayer, MetricObservation};
use crate::core::series_catalog::SeriesCatalog;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::ImpliedNetFlowPoint;

pub(crate) fn candle_window_return_pct(cs: &[AggregatorCandle], win_start: NaiveDate) -> Option<f64> {
    let end = cs.last()?;
    if end.timestamp_utc.date_naive() <= win_start {
        return None;
    }
    let start = cs.iter().rev().find(|c| c.timestamp_utc.date_naive() <= win_start)?.close;
    (start > 0.0).then(|| ((end.close / start) - 1.0) * 100.0)
}

pub(crate) fn sum_flow_exact(pts: &[ImpliedNetFlowPoint], win_start: NaiveDate) -> Option<f64> {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for p in pts {
        if p.timestamp_utc.date_naive() > win_start && p.implied_net_flow_usd.is_finite() {
            sum += p.implied_net_flow_usd;
            cnt += 1;
        }
    }
    (cnt > 0).then_some(sum)
}

pub(crate) fn sum_flow_delta_v(pts: &[ImpliedNetFlowPoint], win_start: NaiveDate) -> Option<f64> {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for p in pts {
        if p.timestamp_utc.date_naive() > win_start && p.delta_v_usd.is_finite() {
            sum += p.delta_v_usd;
            cnt += 1;
        }
    }
    (cnt > 0).then_some(sum)
}

pub(crate) fn sum_obs_window(obs: &[MetricObservation], win_start: NaiveDate) -> Option<f64> {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for o in obs {
        if o.observation_ts_utc.date_naive() > win_start && o.value.is_finite() {
            sum += o.value;
            cnt += 1;
        }
    }
    (cnt > 0).then_some(sum)
}

pub(crate) fn stock_window_delta(obs: &[MetricObservation], win_start: NaiveDate) -> (Option<f64>, Option<f64>) {
    let Some(end) = obs.last() else { return (None, None) };
    if end.observation_ts_utc.date_naive() <= win_start {
        return (None, None);
    }
    let Some(start) = obs.iter().rev().find(|o| o.observation_ts_utc.date_naive() <= win_start) else {
        return (None, None);
    };
    let diff = end.value - start.value;
    let pct = (start.value.abs() > 1e-12).then(|| (diff / start.value) * 100.0);
    (Some(diff), pct)
}

/// Computes average & latest funding rate in basis points (decimal rate * 10,000) over `(win_start, as_of]`.
pub(crate) fn funding_window_bps(obs: &[MetricObservation], win_start: NaiveDate) -> (Option<f64>, Option<f64>) {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    let mut latest = None;
    for o in obs {
        if o.observation_ts_utc.date_naive() > win_start && o.value.is_finite() {
            let bps = o.value * 10_000.0;
            sum += bps;
            cnt += 1;
            latest = Some(bps);
        }
    }
    let avg = (cnt > 0).then(|| sum / cnt as f64);
    (avg, latest)
}

pub(crate) fn push_obs_lineage(
    lin: &mut Vec<SourceLineageEntry>,
    catalog: &SeriesCatalog,
    as_of: NaiveDate,
    sid: &str,
    rows: &[MetricObservation],
    max_lag: i64,
    nodes: &[&str],
) {
    let meta = catalog.get(sid);
    let last = rows.last();
    let lag = last.map(|o| (as_of - o.observation_ts_utc.date_naive()).num_days());
    lin.push(SourceLineageEntry {
        series_id: sid.to_string(),
        source: last
            .map(|o| o.provenance.source.clone())
            .or_else(|| meta.map(|m| m.primary_source.clone()))
            .unwrap_or_else(|| "missing".to_string()),
        source_series_id: last.map(|o| o.provenance.source_series_id.clone()).unwrap_or_default(),
        source_url: last.map(|o| o.provenance.source_url.clone()).unwrap_or_default(),
        source_checksum: last.and_then(|o| o.provenance.source_checksum.clone()),
        batch_id: last.map(|o| o.provenance.batch_id.clone()).unwrap_or_default(),
        data_layer: meta.map(|m| m.data_layer).unwrap_or(DataProcessingLayer::RawAsIs),
        raw_value_preserved: meta.map(|m| m.raw_value_preserved).unwrap_or(true),
        adjustment_note: meta.and_then(|m| m.adjustment_note.clone()),
        known_limitation: meta.and_then(|m| m.limitation_note.clone()),
        latest_observation_utc: last.map(|o| o.observation_ts_utc.to_rfc3339()),
        stale_lag_days: lag,
        is_stale: lag.map_or(true, |d| d > max_lag),
        used_by_nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
    });
}

pub(crate) fn push_px_lineage(
    lin: &mut Vec<SourceLineageEntry>,
    as_of: NaiveDate,
    sid: &str,
    cs: &[AggregatorCandle],
    layer: DataProcessingLayer,
    adj: &str,
    nodes: &[&str],
) {
    let last = cs.last();
    let lag = last.map(|c| (as_of - c.timestamp_utc.date_naive()).num_days());
    lin.push(SourceLineageEntry {
        series_id: sid.to_string(),
        source: last.map(|c| c.source.clone()).unwrap_or_else(|| "missing".to_string()),
        source_series_id: sid.to_string(),
        source_url: "lake://market/candles".to_string(),
        source_checksum: None,
        batch_id: "candles_parquet".to_string(),
        data_layer: layer,
        raw_value_preserved: true,
        adjustment_note: Some(adj.to_string()),
        known_limitation: None,
        latest_observation_utc: last.map(|c| c.timestamp_utc.to_rfc3339()),
        stale_lag_days: lag,
        is_stale: lag.map_or(true, |d| d > 7),
        used_by_nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
    });
}
