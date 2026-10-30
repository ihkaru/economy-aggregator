//! Yahoo Finance REST API fetcher for global equities, indices, commodities, bonds, and forex.

use std::time::Duration;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};

#[derive(Debug, Deserialize)]
struct YahooChartResponse {
    chart: YahooChartPayload,
}

#[derive(Debug, Deserialize)]
struct YahooChartPayload {
    result: Option<Vec<YahooChartResult>>,
    error: Option<YahooChartError>,
}

#[derive(Debug, Deserialize)]
struct YahooChartError {
    code: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct YahooChartResult {
    timestamp: Option<Vec<i64>>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Option<Vec<YahooQuoteData>>,
}

#[derive(Debug, Deserialize)]
struct YahooQuoteData {
    open: Option<Vec<Option<f64>>>,
    high: Option<Vec<Option<f64>>>,
    low: Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
    volume: Option<Vec<Option<f64>>>,
}

pub struct YahooFinanceFetcher {
    client: Client,
    base_url: String,
}

impl Default for YahooFinanceFetcher {
    fn default() -> Self {
        Self::new("https://query1.finance.yahoo.com".to_string())
    }
}

impl YahooFinanceFetcher {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build HTTP client for Yahoo Finance");
        Self { client, base_url }
    }

    /// Fetches historical daily candles for any global ticker (e.g. "^GSPC", "^JKSE", "^N225", "BBCA.JK", "GC=F", "USDIDR=X").
    /// Range can be "5d", "1mo", "1y", "5y", "10y", "max" (multi-decade deep backfill).
    pub async fn fetch_daily_bars(
        &self,
        raw_symbol: &str,
        canonical_symbol: &str,
        range: &str,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let url = if range == "max" {
            let now = Utc::now().timestamp();
            format!(
                "{}/v8/finance/chart/{}?interval=1d&period1=0&period2={}",
                self.base_url,
                urlencoding_safe(raw_symbol),
                now
            )
        } else {
            format!(
                "{}/v8/finance/chart/{}?interval=1d&range={}",
                self.base_url,
                urlencoding_safe(raw_symbol),
                range
            )
        };

        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(format!("Yahoo Finance HTTP {} for {}", resp.status(), raw_symbol).into());
        }

        let payload: YahooChartResponse = resp.json().await?;
        if let Some(err) = payload.chart.error {
            return Err(format!("Yahoo Finance API Error: {} - {}", err.code, err.description).into());
        }

        let results = payload.chart.result.ok_or("No chart result found")?;
        let result = results.into_iter().next().ok_or("Empty chart results vector")?;
        let timestamps = result.timestamp.unwrap_or_default();
        let quote = result
            .indicators
            .quote
            .and_then(|q| q.into_iter().next())
            .ok_or("Missing quote indicators")?;

        let opens = quote.open.unwrap_or_default();
        let highs = quote.high.unwrap_or_default();
        let lows = quote.low.unwrap_or_default();
        let closes = quote.close.unwrap_or_default();
        let volumes = quote.volume.unwrap_or_default();

        let mut candles: Vec<AggregatorCandle> = Vec::with_capacity(timestamps.len());
        for i in 0..timestamps.len() {
            let ts = timestamps[i];
            let o = opens.get(i).copied().flatten();
            let h = highs.get(i).copied().flatten();
            let l = lows.get(i).copied().flatten();
            let c = closes.get(i).copied().flatten();
            let v = volumes.get(i).copied().flatten().unwrap_or(0.0);

            if let (Some(open), Some(high), Some(low), Some(close)) = (o, h, l, c) {
                if open <= 0.0 || high <= 0.0 || low <= 0.0 || close <= 0.0 {
                    continue;
                }
                // Universal statistical outlier gate: reject abnormal jumps (> 3x ratio) without symbol-specific hardcoding
                if let Some(prev) = candles.last() {
                    if crate::features::ingestion::SanityGate::is_price_outlier(prev.close, close, 3.0) {
                        continue;
                    }
                }
                let dt = DateTime::<Utc>::from_timestamp(ts, 0)
                    .unwrap_or_else(|| Utc::now());

                candles.push(AggregatorCandle {
                    timestamp_utc: dt,
                    canonical_symbol: canonical_symbol.to_string(),
                    timeframe: TimeFrame::D1,
                    open,
                    high,
                    low,
                    close,
                    volume_base: v,
                    volume_quote: 0.0,
                    source: "yahoo".to_string(),
                    raw_symbol: raw_symbol.to_string(),
                    ingestion_mode: IngestionMode::RestBatch,
                    is_settled: true,
                    sequence_id: None,
                    taker_buy_volume_base: None,
                    taker_buy_volume_quote: None,
                    vwap: None,
                    trade_count: None,
                    price_type: "last".to_string(),
                    raw_metadata: None,
                });
            }
        }

        Ok(candles)
    }
}

fn urlencoding_safe(s: &str) -> String {
    s.replace('^', "%5E")
}
