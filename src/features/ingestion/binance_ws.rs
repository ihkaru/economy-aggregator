//! Binance WebSocket public kline stream parser, client, and payload mapper.

use std::time::Duration;
use chrono::{TimeZone, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{error, info, warn};

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BinanceKlinePayload {
    #[serde(rename = "t")]
    pub start_time: i64,      // Kline start time in ms
    #[serde(rename = "s")]
    pub symbol: String,        // Raw symbol "BTCUSDT"
    #[serde(rename = "i")]
    pub interval: String,      // Interval "1m"
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "v")]
    pub volume_base: String,
    #[serde(rename = "q")]
    pub volume_quote: String,
    #[serde(rename = "V")]
    pub taker_buy_base: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote: String,
    #[serde(rename = "n")]
    pub trade_count: u32,
    #[serde(rename = "x")]
    pub is_closed: bool,       // true if candle is closed (settled)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BinanceKlineEvent {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: BinanceKlinePayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BinanceCombinedStreamEvent {
    pub stream: String,
    pub data: BinanceKlineEvent,
}

impl BinanceKlineEvent {
    /// Parses raw JSON string into typed Binance event.
    pub fn parse_json(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }

    /// Converts Binance payload into standardized AggregatorCandle.
    pub fn to_aggregator_candle(
        &self,
        canonical_symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<AggregatorCandle, std::num::ParseFloatError> {
        let k = &self.kline;
        let ts_utc = Utc
            .timestamp_millis_opt(k.start_time)
            .single()
            .unwrap_or_else(Utc::now);

        Ok(AggregatorCandle {
            timestamp_utc: ts_utc,
            canonical_symbol: canonical_symbol.to_string(),
            timeframe,
            open: k.open.parse()?,
            high: k.high.parse()?,
            low: k.low.parse()?,
            close: k.close.parse()?,
            volume_base: k.volume_base.parse()?,
            volume_quote: k.volume_quote.parse()?,
            taker_buy_volume_base: k.taker_buy_base.parse().ok(),
            taker_buy_volume_quote: k.taker_buy_quote.parse().ok(),
            source: "binance".to_string(),
            raw_symbol: k.symbol.clone(),
            ingestion_mode: IngestionMode::LiveWs,
            is_settled: k.is_closed,
            sequence_id: None,
            vwap: None,
            trade_count: Some(k.trade_count),
            price_type: "last".to_string(),
        })
    }
}

/// Standalone parser that accepts either single or combined Binance streams.
pub fn parse_binance_message(raw: &str) -> Result<BinanceKlineEvent, serde_json::Error> {
    if let Ok(combined) = serde_json::from_str::<BinanceCombinedStreamEvent>(raw) {
        return Ok(combined.data);
    }
    serde_json::from_str::<BinanceKlineEvent>(raw)
}

/// Binance WebSocket Stream Client configuration
pub struct BinanceWsConfig {
    pub base_url: String,
    pub streams: Vec<String>,
    pub max_reconnect_attempts: u32,
    pub initial_backoff_secs: u64,
}

impl Default for BinanceWsConfig {
    fn default() -> Self {
        Self {
            base_url: "wss://stream.binance.com:9443".to_string(),
            streams: vec!["btcusdt@kline_1m".to_string()],
            max_reconnect_attempts: 10,
            initial_backoff_secs: 1,
        }
    }
}

/// Persistent Binance WebSocket client with auto-reconnection and heartbeat handling.
pub struct BinanceWsClient {
    config: BinanceWsConfig,
}

impl BinanceWsClient {
    pub fn new(config: BinanceWsConfig) -> Self {
        Self { config }
    }

    /// Builds the stream URL for single or combined streams.
    pub fn build_url(&self) -> String {
        if self.config.streams.len() == 1 {
            format!("{}/ws/{}", self.config.base_url, self.config.streams[0])
        } else {
            let combined = self.config.streams.join("/");
            format!("{}/stream?streams={}", self.config.base_url, combined)
        }
    }

    /// Runs the streaming loop until cancelled, feeding parsed candles into tx.
    pub async fn run(
        &self,
        tx: mpsc::Sender<AggregatorCandle>,
        canonical_map: std::collections::HashMap<String, String>,
    ) {
        let mut attempts = 0;
        let mut backoff = self.config.initial_backoff_secs;

        loop {
            let url_str = self.build_url();
            info!("🔌 Connecting to Binance WebSocket: {}", url_str);

            match connect_async(&url_str).await {
                Ok((ws_stream, response)) => {
                    info!("✅ Connected to Binance WS (HTTP {})", response.status());
                    attempts = 0;
                    backoff = self.config.initial_backoff_secs;

                    let (mut write, mut read) = ws_stream.split();

                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(Message::Text(text)) => {
                                match parse_binance_message(&text) {
                                    Ok(event) => {
                                        let raw_sym = &event.symbol;
                                        let canonical = canonical_map
                                            .get(raw_sym)
                                            .cloned()
                                            .unwrap_or_else(|| raw_sym.clone());

                                        match event.to_aggregator_candle(&canonical, TimeFrame::M1) {
                                            Ok(candle) => {
                                                if let Err(e) = tx.send(candle).await {
                                                    warn!("Pipeline receiver dropped, stopping WS loop: {}", e);
                                                    return;
                                                }
                                            }
                                            Err(e) => {
                                                warn!("Failed to parse candle numbers: {}", e);
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        // Ignore ping/pong/system messages
                                    }
                                }
                            }
                            Ok(Message::Ping(payload)) => {
                                if let Err(e) = write.send(Message::Pong(payload)).await {
                                    warn!("Failed to respond to WS Ping: {}", e);
                                    break;
                                }
                            }
                            Ok(Message::Close(frame)) => {
                                warn!("WebSocket server sent close frame: {:?}", frame);
                                break;
                            }
                            Err(e) => {
                                error!("WebSocket read error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    attempts += 1;
                    error!(
                        "WebSocket connection failed (attempt {}/{}): {}",
                        attempts, self.config.max_reconnect_attempts, e
                    );
                }
            }

            if attempts >= self.config.max_reconnect_attempts {
                error!("Exceeded maximum reconnect attempts. Stopping WebSocket client.");
                break;
            }

            info!("Sleeping {}s before reconnecting...", backoff);
            sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(60);
        }
    }
}
