//! Vendor-agnostic Prediction Market Domain Models & Implied Probability Types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Specification of the resolution oracle determining market payout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolutionOracleSpec {
    pub oracle_type: String,               // "official_agency", "uma_optimistic", "cftc_rules", "exchange_committee"
    pub source_name: String,               // e.g. "U.S. Bureau of Labor Statistics"
    pub source_url: String,                // e.g. "https://www.bls.gov/cpi"
    pub resolution_criteria: String,       // Verbatim text defining how contract settles
}

/// Settleable contract / prediction market definition across venues.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionMarketContract {
    pub market_id: String,                 // Canonical: "kalshi:KXFEDDECISION-26NOV-C25"
    pub venue: String,                     // "kalshi", "polymarket", "opinion", "gemini"
    pub venue_market_id: String,           // Native venue ID
    pub parent_event_id: String,           // Parent event grouping e.g. "kalshi:KXFEDDECISION-26NOV"
    pub title: String,                     // "Will the Fed cut rates by 25bps in November 2026?"
    pub outcome_label: String,             // Short label: "25 bps cut"
    pub status: String,                    // "unopened", "open", "closed", "settled"
    pub is_active: bool,                   // Accepting orders right now
    pub market_type: String,               // "binary", "categorical", "scalar"
    pub book_model: String,                // "shared_complement" (Kalshi) vs "native_per_outcome" (Polymarket)
    pub outcomes: Vec<String>,             // ["YES", "NO"] or categorical outcomes
    pub winning_outcome: Option<String>,   // Winning outcome string upon settlement
    pub face_value: f64,                   // Payout face value (default 1.00 USD)
    pub tick_size: f64,                    // Minimum price tick increment (e.g. 0.001)
    pub close_time_utc: DateTime<Utc>,     // Scheduled trading termination
    pub settlement_time_utc: Option<DateTime<Utc>>,
    pub linked_macro_indicator: Option<String>, // Direct link: "macro:US:fed_funds_rate:2026-11"
    pub oracle: ResolutionOracleSpec,
    pub venue_url: Option<String>,
}

impl PredictionMarketContract {
    pub fn natural_key(&self) -> String {
        format!("{}:{}", self.venue, self.venue_market_id)
    }
}

/// Where an implied probability candlestick bar was derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilityPriceSource {
    Trade,       // Built from executed matches/fills
    BidAskMid,   // Midpoint of resting orderbook when 0 trades occurred
    SampledMid,  // Periodic time-sampled quotes (Polymarket Gamma)
    SampledLast, // Last traded price repeated during period (Opinion)
}

impl ProbabilityPriceSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trade => "trade",
            Self::BidAskMid => "bid_ask_mid",
            Self::SampledMid => "sampled_mid",
            Self::SampledLast => "sampled_last",
        }
    }
}

/// Implied probability time-series bar in range [0.0000, 1.0000].
/// Absence of quotes/trades is strictly None, never 0.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionMarketCandle {
    pub time: i64,                         // Unix epoch seconds
    pub timestamp_utc: DateTime<Utc>,
    pub market_id: String,                 // "kalshi:KXFEDDECISION-26NOV-C25"
    pub outcome: String,                   // "YES" or "NO"
    pub timeframe: String,                 // "1m", "5m", "1h", "1d"
    
    // Implied probabilities strictly nullable
    pub prob_open: Option<f64>,
    pub prob_high: Option<f64>,
    pub prob_low: Option<f64>,
    pub prob_close: Option<f64>,
    
    // Resting book state at bar close
    pub book_bid_close: Option<f64>,
    pub book_ask_close: Option<f64>,
    
    // Separated volume metrics avoiding category errors
    pub volume_contracts: Option<f64>,     // Total contracts/shares traded
    pub volume_usd: Option<f64>,           // Notional volume in USD
    pub open_interest_usd: Option<f64>,    // Open interest in USD
    pub trade_count: Option<u32>,
    
    pub price_source: ProbabilityPriceSource,
    pub venue: String,                     // "kalshi", "polymarket"
}

impl PredictionMarketCandle {
    pub fn natural_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.market_id,
            self.outcome,
            self.timeframe,
            self.time
        )
    }
}

/// Cross-venue synthetic bucket member mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BucketMemberSpec {
    pub venue: String,                     // "kalshi" or "polymarket"
    pub market_id: String,
    pub flip_side: bool,                   // True if YES here means NO in unified bucket
    pub weight: f64,
}

/// Synthetic cross-venue bucket combining multiple prediction markets for 1 proposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedPredictionBucket {
    pub bucket_id: String,                 // "bucket:us_cpi_oct26_above_2_5"
    pub title: String,
    pub linked_macro_event_id: String,     // "macro:US:cpi_yoy:2026-10"
    pub members: Vec<BucketMemberSpec>,
}
