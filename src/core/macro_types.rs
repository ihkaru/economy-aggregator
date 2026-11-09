//! Vendor-agnostic Macroeconomic Calendar & Point-in-Time Release Domain Models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Detailed provenance and official releasing agency attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceAttribution {
    pub provider: String,                  // e.g. "forexfactory", "biquote", "bls", "tradingeconomics"
    pub original_id: String,               // Provider's internal or foreign ID
    pub source_url: Option<String>,        // Public scrape or API URL
    pub agency_name: Option<String>,       // e.g. "U.S. Bureau of Labor Statistics"
    pub agency_url: Option<String>,        // e.g. "https://www.bls.gov"
    pub observed_at_utc: DateTime<Utc>,    // Timestamp when our engine first recorded this event
}

/// Standardized macroeconomic indicator metric unit taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricUnitType {
    Percentage,   // CPI, Inflation, Unemployment Rate, GDP Growth (%)
    Currency,     // Trade Balance, Treasury Yields (USD, EUR, IDR)
    Count,        // NFP, Jobless Claims, Building Approvals
    IndexPoints,  // PMI, Consumer Confidence Index (0-100)
    BasisPoints,  // Central Bank Rate Hikes (bps)
    Qualitative,  // Speeches, Meeting Minutes, Holidays
}

impl MetricUnitType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Percentage => "percentage",
            Self::Currency => "currency",
            Self::Count => "count",
            Self::IndexPoints => "index_points",
            Self::BasisPoints => "basis_points",
            Self::Qualitative => "qualitative",
        }
    }

    pub fn from_str_lenient(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "percent" | "percentage" | "%" => Self::Percentage,
            "currency" | "usd" | "eur" | "idr" | "gbp" => Self::Currency,
            "count" | "jobs" | "people" => Self::Count,
            "index" | "index_points" | "points" | "pts" => Self::IndexPoints,
            "bps" | "basis_points" => Self::BasisPoints,
            _ => Self::Qualitative,
        }
    }
}

/// Rich vendor-agnostic economic event with exact string preserves and normalized values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichEconomicEvent {
    pub event_id: String,                  // Canonical ID: "macro:US:non_farm_payrolls:2026-11-06"
    pub scheduled_utc: DateTime<Utc>,      // Official release time (UTC)
    pub country: String,                   // ISO-3166-1 alpha-2 ("US", "ID", "EU")
    pub currency: String,                  // ISO-4217 ("USD", "IDR", "EUR")
    pub category: String,                  // "employment", "inflation", "growth", "central_banks"
    pub indicator_code: String,            // Normalized slug: "non_farm_payrolls"
    pub title: String,                     // "Non-Farm Employment Change"
    pub impact_tier: u8,                   // 0: Non-Economic/Holiday, 1: Low, 2: Medium, 3: High
    pub is_all_day: bool,                  // True if no fixed release minute
    pub is_tentative: bool,                // True if time is tentative/unconfirmed

    // Raw String representations preserving scraper source text exactly
    pub raw_actual: Option<String>,        // e.g. "202K", "3.2%", "Hold"
    pub raw_forecast: Option<String>,      // e.g. "160K", "3.4%"
    pub raw_previous: Option<String>,      // e.g. "124K"

    // Normalized values scaled to base units for analytical computations
    pub normalized_actual: Option<f64>,    // e.g. 202000.0, 0.032
    pub normalized_forecast: Option<f64>,  // e.g. 160000.0, 0.034
    pub normalized_previous: Option<f64>,  // e.g. 124000.0
    pub normalized_revised_previous: Option<f64>,

    // Unit & scale taxonomy
    pub unit_type: MetricUnitType,
    pub unit_multiplier: f64,              // 1.0, 1000.0 (K), 1000000.0 (M), 0.01 (%)
    pub unit_symbol: String,               // "K", "M", "%", "pts"

    // Market Polarity / Sentiment: Some(true) = Bullish (Green), Some(false) = Bearish (Red)
    pub better_than_expected: Option<bool>,

    // Audit trail & source attribution
    pub attribution: SourceAttribution,
    pub revision_count: u32,               // 0 if unrevised initial print
    pub revision_history_json: Option<String>,
}

impl RichEconomicEvent {
    /// Deterministic natural composite key for idempotent lake deduplication.
    pub fn natural_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.country,
            self.indicator_code,
            self.scheduled_utc.to_rfc3339(),
            self.attribution.provider
        )
    }
}
