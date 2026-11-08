//! Dukascopy Ingestion: Historical bar parser and fetcher for Dukascopy FX/CFD feeds.
//! Supports both standard JSON object arrays and dense array-of-arrays formats.

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DukascopyObjectBar {
    #[serde(alias = "time", alias = "t")]
    pub timestamp: i64,
    #[serde(alias = "o")]
    pub open: f64,
    #[serde(alias = "h")]
    pub high: f64,
    #[serde(alias = "l")]
    pub low: f64,
    #[serde(alias = "c")]
    pub close: f64,
    #[serde(default, alias = "v", alias = "vol")]
    pub volume: f64,
}

pub struct DukascopyParser;

impl DukascopyParser {
    /// Parses Dukascopy JSON payload into domain `AggregatorCandle` list.
    /// Supports:
    /// 1. Array of objects: `[{"timestamp": 1704067200000, "open": 1.1, "high": 1.2, "low": 1.0, "close": 1.15, "volume": 100}, ...]`
    /// 2. Array of numeric arrays: `[[1704067200000, 1.1, 1.2, 1.0, 1.15, 100], ...]`
    pub fn parse_json(
        json_str: &str,
        canonical_symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let trimmed = json_str.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // Try parsing as array of objects
        if let Ok(objects) = serde_json::from_str::<Vec<DukascopyObjectBar>>(trimmed) {
            let candles = objects
                .into_iter()
                .filter_map(|bar| Self::convert_bar(bar, canonical_symbol, timeframe))
                .collect();
            return Ok(candles);
        }

        // Try parsing as array of arrays: [timestamp, open, high, low, close, volume]
        if let Ok(arrays) = serde_json::from_str::<Vec<Vec<serde_json::Value>>>(trimmed) {
            let mut candles = Vec::with_capacity(arrays.len());
            for row in arrays {
                if row.len() < 5 {
                    continue;
                }
                let ts_raw = row[0].as_i64().or_else(|| row[0].as_f64().map(|f| f as i64)).unwrap_or(0);
                let open = row[1].as_f64().unwrap_or(0.0);
                let high = row[2].as_f64().unwrap_or(0.0);
                let low = row[3].as_f64().unwrap_or(0.0);
                let close = row[4].as_f64().unwrap_or(0.0);
                let volume = if row.len() > 5 {
                    row[5].as_f64().unwrap_or(0.0)
                } else {
                    0.0
                };

                let bar = DukascopyObjectBar {
                    timestamp: ts_raw,
                    open,
                    high,
                    low,
                    close,
                    volume,
                };
                if let Some(c) = Self::convert_bar(bar, canonical_symbol, timeframe) {
                    candles.push(c);
                }
            }
            return Ok(candles);
        }

        Err("Failed to parse Dukascopy JSON payload: unrecognized schema".into())
    }

    fn convert_bar(
        bar: DukascopyObjectBar,
        canonical_symbol: &str,
        timeframe: TimeFrame,
    ) -> Option<AggregatorCandle> {
        let ts_micros = if bar.timestamp > 1_000_000_000_000_000 {
            bar.timestamp
        } else if bar.timestamp > 1_000_000_000_000 {
            bar.timestamp * 1_000
        } else if bar.timestamp > 1_000_000_000 {
            bar.timestamp * 1_000_000
        } else {
            return None;
        };

        let timestamp_utc = DateTime::from_timestamp_micros(ts_micros)?;

        Some(AggregatorCandle {
            timestamp_utc,
            canonical_symbol: canonical_symbol.to_string(),
            timeframe,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume_base: bar.volume,
            volume_quote: 0.0,
            taker_buy_volume_base: None,
            taker_buy_volume_quote: None,
            source: "dukascopy".to_string(),
            raw_symbol: canonical_symbol.to_string(),
            ingestion_mode: IngestionMode::RestBatch,
            is_settled: true,
            sequence_id: None,
            vwap: None,
            trade_count: None,
            price_type: "mid".to_string(),
            raw_metadata: None,
        })
    }
}
