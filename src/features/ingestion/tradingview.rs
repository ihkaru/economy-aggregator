//! TradingView UDF Ingestion: Universal Datafeed parser for TradingView historical bar series.
//! Implements the standard /history parallel-array response format (`s: "ok"`, `t`, `o`, `h`, `l`, `c`, `v`).

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingViewUdfResponse {
    pub s: String,
    #[serde(default)]
    pub t: Vec<i64>,
    #[serde(default)]
    pub o: Vec<f64>,
    #[serde(default)]
    pub h: Vec<f64>,
    #[serde(default)]
    pub l: Vec<f64>,
    #[serde(default)]
    pub c: Vec<f64>,
    #[serde(default)]
    pub v: Vec<f64>,
    #[serde(default)]
    pub errmsg: Option<String>,
}

pub struct TradingViewParser;

impl TradingViewParser {
    /// Parses a standard TradingView UDF `/history` response into a list of domain `AggregatorCandle`s.
    pub fn parse_udf_json(
        json_str: &str,
        canonical_symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let trimmed = json_str.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let resp: TradingViewUdfResponse = serde_json::from_str(trimmed)?;

        if resp.s == "no_data" {
            return Ok(Vec::new());
        }

        if resp.s == "error" {
            let msg = resp.errmsg.unwrap_or_else(|| "Unknown TradingView error".to_string());
            return Err(format!("TradingView UDF returned error: {}", msg).into());
        }

        if resp.s != "ok" {
            return Err(format!("Unrecognized TradingView UDF status: {}", resp.s).into());
        }

        let len = resp.t.len();
        let mut candles = Vec::with_capacity(len);

        for i in 0..len {
            let raw_ts = resp.t[i];
            let ts_micros = if raw_ts > 1_000_000_000_000_000 {
                raw_ts
            } else if raw_ts > 1_000_000_000_000 {
                raw_ts * 1_000
            } else {
                raw_ts * 1_000_000
            };

            let timestamp_utc = match DateTime::from_timestamp_micros(ts_micros) {
                Some(dt) => dt,
                None => continue,
            };

            let open = resp.o.get(i).copied().unwrap_or(0.0);
            let high = resp.h.get(i).copied().unwrap_or(0.0);
            let low = resp.l.get(i).copied().unwrap_or(0.0);
            let close = resp.c.get(i).copied().unwrap_or(0.0);
            let volume = resp.v.get(i).copied().unwrap_or(0.0);

            candles.push(AggregatorCandle {
                timestamp_utc,
                canonical_symbol: canonical_symbol.to_string(),
                timeframe,
                open,
                high,
                low,
                close,
                volume_base: volume,
                volume_quote: 0.0,
                taker_buy_volume_base: None,
                taker_buy_volume_quote: None,
                source: "tradingview".to_string(),
                raw_symbol: canonical_symbol.to_string(),
                ingestion_mode: IngestionMode::RestBatch,
                is_settled: true,
                sequence_id: None,
                vwap: None,
                trade_count: None,
                price_type: "last".to_string(),
                raw_metadata: None,
            });
        }

        Ok(candles)
    }
}
