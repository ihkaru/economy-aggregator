//! TradingView Ingestion: WebSocket bar fetcher, live streamer, and UDF parser.

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use tracing::warn;
use tradingview_streamer::discovery::DiscoveryClient;
use tradingview_streamer::ticker::Ticker;

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};
use crate::features::ingestion::search::SearchResultItem;

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
                time: timestamp_utc.timestamp(),
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

/// High-performance TradingView client utilizing native async WebSocket streaming and venue discovery.
#[derive(Default, Clone)]
pub struct TradingViewFetcher;

impl TradingViewFetcher {
    /// Fetches historical bars directly from TradingView gateway preserving full venue prefix.
    /// Example symbols: "COINBASE:BTCUSD", "BINANCE:BTCUSDT", "NASDAQ:AAPL", "IDX:BBCA".
    pub async fn fetch_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        count: u32,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let tf_str = match timeframe {
            TimeFrame::Tick => "1S",
            TimeFrame::S1 => "1S",
            TimeFrame::M1 => "1m",
            TimeFrame::M5 => "5m",
            TimeFrame::M15 => "15m",
            TimeFrame::H1 => "1h",
            TimeFrame::H4 => "4h",
            TimeFrame::D1 => "1D",
            TimeFrame::W1 => "1W",
            TimeFrame::MN1 => "1M",
        };

        let ticker = Ticker::new(symbol);
        let raw_candles = ticker.history(tf_str, count).await.map_err(|e| {
            format!("TradingView fetch failed for {}: {}", symbol, e)
        })?;

        let (venue, ticker_name) = match symbol.split_once(':') {
            Some((v, t)) => (v.to_string(), t.to_string()),
            None => ("TRADINGVIEW".to_string(), symbol.to_string()),
        };

        let mut candles = Vec::with_capacity(raw_candles.len());
        for c in raw_candles {
            let timestamp_utc = match DateTime::from_timestamp(c.timestamp, 0) {
                Some(dt) => dt,
                None => continue,
            };

            candles.push(AggregatorCandle {
                time: c.timestamp,
                timestamp_utc,
                canonical_symbol: symbol.to_string(),
                timeframe,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume_base: c.volume.unwrap_or(0.0),
                volume_quote: 0.0,
                taker_buy_volume_base: None,
                taker_buy_volume_quote: None,
                source: "tradingview".to_string(),
                raw_symbol: symbol.to_string(),
                ingestion_mode: IngestionMode::RestBatch,
                is_settled: true,
                sequence_id: None,
                vwap: None,
                trade_count: None,
                price_type: "last".to_string(),
                raw_metadata: Some(format!(
                    "{{\"venue\":\"{}\",\"ticker\":\"{}\"}}",
                    venue, ticker_name
                )),
            });
        }

        Ok(candles)
    }

    /// Discovers multi-venue listings for a query (e.g. "BTCUSD" -> Coinbase, Binance, OKX, Kraken).
    pub async fn discover_venues(&self, query: &str, limit: usize) -> Vec<SearchResultItem> {
        let discovery = DiscoveryClient::new();
        match discovery.discover_venues(query).await {
            Ok(venues) => venues
                .into_iter()
                .take(limit)
                .map(|v| {
                    let asset_class = match v.security_type.as_deref() {
                        Some("crypto") => "crypto",
                        Some("forex") => "forex",
                        Some("commodity") => "commodities",
                        _ => {
                            if v.symbol.contains("BTC") || v.symbol.contains("ETH") || v.symbol.contains("USDT") {
                                "crypto"
                            } else {
                                "equities"
                            }
                        }
                    };

                    SearchResultItem {
                        source: "tradingview".to_string(),
                        raw_symbol: v.symbol.clone(),
                        canonical_symbol: v.symbol,
                        name: format!("{} on {}", v.name, v.exchange),
                        asset_class: asset_class.to_string(),
                        exchange: Some(v.exchange),
                        currency: v.currency,
                        quote_type: v.security_type,
                    }
                })
                .collect(),
            Err(e) => {
                warn!("TradingView venue discovery error for '{}': {}", query, e);
                Vec::new()
            }
        }
    }
}
