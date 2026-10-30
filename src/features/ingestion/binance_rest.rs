//! Binance Historical REST Klines fetcher with rate limit weight monitoring.

use std::time::Duration;
use chrono::{TimeZone, Utc};
use reqwest::header::HeaderMap;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, warn};

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};

pub struct BinanceRestFetcher {
    client: Client,
    base_url: String,
}

impl Default for BinanceRestFetcher {
    fn default() -> Self {
        Self::new("https://api.binance.com".to_string())
    }
}

impl BinanceRestFetcher {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, base_url }
    }

    /// Fetches historical klines from Binance Spot API.
    /// Returns AggregatorCandles with `is_settled: true` and `IngestionMode::HistoricalRest`.
    pub async fn fetch_klines(
        &self,
        symbol: &str,
        canonical_symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/v3/klines", self.base_url);
        let mut query = vec![
            ("symbol".to_string(), symbol.to_string()),
            ("interval".to_string(), interval.to_string()),
        ];

        if let Some(st) = start_time {
            query.push(("startTime".to_string(), st.to_string()));
        }
        if let Some(et) = end_time {
            query.push(("endTime".to_string(), et.to_string()));
        }
        if let Some(lim) = limit {
            query.push(("limit".to_string(), lim.min(1000).to_string()));
        }

        let res = self.client.get(&url).query(&query).send().await?;

        // Inspect rate limit weight header
        if let Some(used_weight) = extract_rate_limit_weight(res.headers()) {
            if used_weight > 1000 {
                warn!("⚠️ Binance REST rate limit weight high: {}/1200", used_weight);
            } else {
                debug!("Binance REST used weight: {}/1200", used_weight);
            }
        }

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Binance API error ({}): {}", status, body).into());
        }

        let raw_rows: Vec<Vec<Value>> = res.json().await?;
        let timeframe = TimeFrame::from_str_opt(interval).unwrap_or(TimeFrame::M1);

        let mut candles = Vec::with_capacity(raw_rows.len());
        for row in raw_rows {
            if row.len() < 11 {
                continue;
            }

            let open_time_ms = row[0].as_i64().unwrap_or(0);
            let open: f64 = row[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let high: f64 = row[2].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let low: f64 = row[3].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let close: f64 = row[4].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let volume_base: f64 = row[5].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let volume_quote: f64 = row[7].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let trade_count = row[8].as_u64().map(|n| n as u32);
            let taker_buy_base: Option<f64> = row[9].as_str().and_then(|s| s.parse().ok());
            let taker_buy_quote: Option<f64> = row[10].as_str().and_then(|s| s.parse().ok());

            let ts_utc = Utc
                .timestamp_millis_opt(open_time_ms)
                .single()
                .unwrap_or_else(Utc::now);

            candles.push(AggregatorCandle {
                timestamp_utc: ts_utc,
                canonical_symbol: canonical_symbol.to_string(),
                timeframe,
                open,
                high,
                low,
                close,
                volume_base,
                volume_quote,
                taker_buy_volume_base: taker_buy_base,
                taker_buy_volume_quote: taker_buy_quote,
                source: "binance".to_string(),
                raw_symbol: symbol.to_string(),
                ingestion_mode: IngestionMode::RestBatch,
                is_settled: true,
                sequence_id: None,
                vwap: None,
                trade_count,
                price_type: "last".to_string(),
                raw_metadata: None,
            });
        }

        Ok(candles)
    }
}

/// Helper function to parse Binance header `x-mbx-used-weight-1m`.
pub fn extract_rate_limit_weight(headers: &HeaderMap) -> Option<u32> {
    headers
        .get("x-mbx-used-weight-1m")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
}
