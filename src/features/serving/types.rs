//! Serving API Data Transfer Objects (DTOs) and request/response models.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
    pub uptime_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct CandleQuery {
    pub asset_class: String,
    pub symbol: String,
    pub timeframe: String,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct StorageStatsResponse {
    pub lake_dir: String,
    pub total_chunks_found: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstrumentCatalogItem {
    pub symbol: String,
    pub canonical_symbol: String,
    pub name: String,
    pub asset_class: String,
    pub currency: String,
    pub price_precision: usize,
    pub min_tick: f64,
    pub supported_timeframes: Vec<String>,
    pub default_timeframe: String,
    pub source: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct MetadataQuery {
    pub symbol: String,
    pub asset_class: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstrumentMetadata {
    pub symbol: String,
    pub canonical_symbol: String,
    pub name: String,
    pub asset_class: String,
    pub currency: String,
    pub price_precision: usize,
    pub min_tick: f64,
    pub supported_timeframes: Vec<String>,
    pub default_timeframe: String,
    pub has_intraday: bool,
    pub has_daily: bool,
    pub has_weekly_and_monthly: bool,
    pub total_bars: usize,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
}
