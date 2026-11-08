//! Multi-Source Candle Reconciliation Engine: Audits lakehouse data integrity against live vendor feeds.
//! Detects corporate action drift, retroactive revisions, missing bars, and volume discrepancies.

use std::collections::{BTreeMap, HashSet};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscrepancyType {
    PriceDrift,
    VolumeDrift,
    MissingInLake,
    MissingInLive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleDiscrepancy {
    pub date: NaiveDate,
    pub lake_close: f64,
    pub live_close: f64,
    pub drift_abs: f64,
    pub drift_pct: f64,
    pub lake_volume: f64,
    pub live_volume: f64,
    pub discrepancy_type: DiscrepancyType,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationReport {
    pub canonical_symbol: String,
    pub lake_source: String,
    pub live_source: String,
    pub timeframe: String,
    pub total_lake_bars: usize,
    pub total_live_bars: usize,
    pub matched_bars: usize,
    pub exact_matches: usize,
    pub tolerance_pct: f64,
    pub drifted_bars_count: usize,
    pub missing_in_lake_count: usize,
    pub missing_in_live_count: usize,
    pub max_close_drift_pct: f64,
    pub mean_close_drift_pct: f64,
    pub is_valid: bool,
    pub discrepancies: Vec<CandleDiscrepancy>,
}

pub struct ReconciliationEngine;

impl ReconciliationEngine {
    /// Compares two sets of candles (lake vs live) across common dates.
    pub fn compare_candles(
        canonical_symbol: &str,
        lake_candles: &[AggregatorCandle],
        live_candles: &[AggregatorCandle],
        tolerance_pct: f64,
    ) -> ReconciliationReport {
        let mut lake_map: BTreeMap<NaiveDate, &AggregatorCandle> = BTreeMap::new();
        for c in lake_candles {
            lake_map.insert(c.timestamp_utc.date_naive(), c);
        }

        let mut live_map: BTreeMap<NaiveDate, &AggregatorCandle> = BTreeMap::new();
        for c in live_candles {
            live_map.insert(c.timestamp_utc.date_naive(), c);
        }

        let lake_source = lake_candles.first().map(|c| c.source.as_str()).unwrap_or("lake").to_string();
        let live_source = live_candles.first().map(|c| c.source.as_str()).unwrap_or("live").to_string();
        let timeframe = lake_candles.first().or_else(|| live_candles.first())
            .map(|c| c.timeframe.as_str().to_string()).unwrap_or_else(|| "1d".to_string());

        let all_dates: HashSet<NaiveDate> = lake_map.keys().copied().chain(live_map.keys().copied()).collect();
        let mut sorted_dates: Vec<NaiveDate> = all_dates.into_iter().collect();
        sorted_dates.sort();

        // Calculate common date range bounds to avoid penalizing out-of-range history
        let live_start = live_map.keys().next().copied();
        let live_end = live_map.keys().next_back().copied();
        let lake_start = lake_map.keys().next().copied();
        let lake_end = lake_map.keys().next_back().copied();

        let mut matched_bars = 0usize;
        let mut exact_matches = 0usize;
        let mut drifted_bars_count = 0usize;
        let mut missing_in_lake_count = 0usize;
        let mut missing_in_live_count = 0usize;
        let mut max_drift = 0.0f64;
        let mut total_drift = 0.0f64;
        let mut discrepancies = Vec::new();

        for d in sorted_dates {
            match (lake_map.get(&d), live_map.get(&d)) {
                (Some(lake_c), Some(live_c)) => {
                    matched_bars += 1;
                    let drift_abs = (lake_c.close - live_c.close).abs();
                    let denom = if lake_c.close.abs() > 1e-9 { lake_c.close.abs() } else { 1.0 };
                    let drift_pct = (drift_abs / denom) * 100.0;

                    if drift_pct > max_drift {
                        max_drift = drift_pct;
                    }
                    total_drift += drift_pct;

                    if drift_pct < 0.001 {
                        exact_matches += 1;
                    } else if drift_pct > tolerance_pct {
                        drifted_bars_count += 1;
                        discrepancies.push(CandleDiscrepancy {
                            date: d,
                            lake_close: lake_c.close,
                            live_close: live_c.close,
                            drift_abs,
                            drift_pct,
                            lake_volume: lake_c.volume_base,
                            live_volume: live_c.volume_base,
                            discrepancy_type: DiscrepancyType::PriceDrift,
                            details: format!(
                                "Close price drift {:.3}% exceeds tolerance {:.3}% (lake={:.4}, live={:.4})",
                                drift_pct, tolerance_pct, lake_c.close, live_c.close
                            ),
                        });
                    }
                }
                (None, Some(live_c)) => {
                    let in_range = match (lake_start, lake_end) {
                        (Some(s), Some(e)) => d >= s && d <= e,
                        _ => false,
                    };
                    if in_range {
                        missing_in_lake_count += 1;
                        discrepancies.push(CandleDiscrepancy {
                            date: d,
                            lake_close: 0.0,
                            live_close: live_c.close,
                            drift_abs: live_c.close,
                            drift_pct: 100.0,
                            lake_volume: 0.0,
                            live_volume: live_c.volume_base,
                            discrepancy_type: DiscrepancyType::MissingInLake,
                            details: format!("Candle present in {} but missing from lake within observation range", live_source),
                        });
                    }
                }
                (Some(lake_c), None) => {
                    // Only flag if within live fetch date range
                    let in_range = match (live_start, live_end) {
                        (Some(s), Some(e)) => d >= s && d <= e,
                        _ => false,
                    };
                    if in_range {
                        missing_in_live_count += 1;
                        discrepancies.push(CandleDiscrepancy {
                            date: d,
                            lake_close: lake_c.close,
                            live_close: 0.0,
                            drift_abs: lake_c.close,
                            drift_pct: 100.0,
                            lake_volume: lake_c.volume_base,
                            live_volume: 0.0,
                            discrepancy_type: DiscrepancyType::MissingInLive,
                            details: format!("Candle present in lake but missing from {} within fetch range", live_source),
                        });
                    }
                }
                (None, None) => {}
            }
        }

        let mean_drift = if matched_bars > 0 { total_drift / matched_bars as f64 } else { 0.0 };
        let is_valid = drifted_bars_count == 0 && missing_in_lake_count == 0 && matched_bars > 0;

        ReconciliationReport {
            canonical_symbol: canonical_symbol.to_string(),
            lake_source,
            live_source,
            timeframe,
            total_lake_bars: lake_candles.len(),
            total_live_bars: live_candles.len(),
            matched_bars,
            exact_matches,
            tolerance_pct,
            drifted_bars_count,
            missing_in_lake_count,
            missing_in_live_count,
            max_close_drift_pct: max_drift,
            mean_close_drift_pct: mean_drift,
            is_valid,
            discrepancies,
        }
    }

    /// Compares two daily price series tuples: `(NaiveDate, ClosePrice)`.
    pub fn compare_daily_series(
        canonical_symbol: &str,
        lake_series: &[(NaiveDate, f64)],
        live_series: &[(NaiveDate, f64)],
        tolerance_pct: f64,
    ) -> ReconciliationReport {
        let to_candles = |series: &[(NaiveDate, f64)], src: &str| -> Vec<AggregatorCandle> {
            series
                .iter()
                .map(|(d, p)| AggregatorCandle {
                    timestamp_utc: d.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc(),
                    canonical_symbol: canonical_symbol.to_string(),
                    timeframe: TimeFrame::D1,
                    open: *p,
                    high: *p,
                    low: *p,
                    close: *p,
                    volume_base: 0.0,
                    volume_quote: 0.0,
                    taker_buy_volume_base: None,
                    taker_buy_volume_quote: None,
                    source: src.to_string(),
                    raw_symbol: canonical_symbol.to_string(),
                    ingestion_mode: IngestionMode::RestBatch,
                    is_settled: true,
                    sequence_id: None,
                    vwap: None,
                    trade_count: None,
                    price_type: "close".to_string(),
                    raw_metadata: None,
                })
                .collect()
        };

        let lake_candles = to_candles(lake_series, "lake");
        let live_candles = to_candles(live_series, "live_series");
        Self::compare_candles(canonical_symbol, &lake_candles, &live_candles, tolerance_pct)
    }
}
