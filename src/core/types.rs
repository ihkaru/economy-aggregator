//! Core domain types for economy and financial data aggregator engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Equities,
    Crypto,
    Forex,
    Commodities,
    Macro,
}

impl AssetClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Equities => "equities",
            Self::Crypto => "crypto",
            Self::Forex => "forex",
            Self::Commodities => "commodities",
            Self::Macro => "macro",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentType {
    Spot,
    Perpetual,
    Future,
    Index,
    Indicator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeFrame {
    Tick,
    #[serde(rename = "1s")]
    S1,
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "4h")]
    H4,
    #[serde(rename = "1d")]
    D1,
    #[serde(rename = "1w")]
    W1,
}

impl TimeFrame {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tick => "tick",
            Self::S1 => "1s",
            Self::M1 => "1m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::H4 => "4h",
            Self::D1 => "1d",
            Self::W1 => "1w",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionMode {
    LiveWs,
    RestBatch,
    Reconciliation,
}

impl IngestionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LiveWs => "live_ws",
            Self::RestBatch => "rest_batch",
            Self::Reconciliation => "reconciliation",
        }
    }
}

/// Standardized high-frequency candlestick / bar record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregatorCandle {
    pub timestamp_utc: DateTime<Utc>,
    pub canonical_symbol: String,
    pub timeframe: TimeFrame,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume_base: f64,
    pub volume_quote: f64,
    pub source: String,
    pub raw_symbol: String,
    pub ingestion_mode: IngestionMode,
    pub is_settled: bool,
    pub sequence_id: Option<u64>,
    pub taker_buy_volume_base: Option<f64>,
    pub taker_buy_volume_quote: Option<f64>,
    pub vwap: Option<f64>,
    pub trade_count: Option<u32>,
    pub price_type: String, // "last", "mark", "index"
}

impl AggregatorCandle {
    /// Deterministic natural composite key for idempotent lake deduplication.
    pub fn natural_key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.canonical_symbol,
            self.timeframe.as_str(),
            self.timestamp_utc.to_rfc3339(),
            self.source,
            self.price_type
        )
    }
}

/// Macroeconomic event record following SDMX standards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomicEventRecord {
    pub event_id: String,
    pub scheduled_utc: DateTime<Utc>,
    pub country: String,  // ISO-3166-1 alpha-2
    pub currency: String, // ISO-4217
    pub category: String,
    pub indicator_code: String,
    pub title: String,
    pub impact_level: u8, // 1: Low, 2: Medium, 3: High
    pub source: String,
    pub unit: String,
    pub actual: Option<f64>,
    pub forecast: Option<f64>,
    pub previous: Option<f64>,
    pub revised_previous: Option<f64>,
    pub released_at_utc: Option<DateTime<Utc>>,
}
