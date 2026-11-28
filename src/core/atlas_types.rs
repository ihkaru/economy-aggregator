//! Core Data Contracts for the 6-Tank Synchronized Liquidity & Capital Flow Atlas (Phase 362).

use serde::{Deserialize, Serialize};

use crate::core::diagnostic_types::DiagnosticTreeReport;
use crate::core::observation_types::DataProcessingLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasPoint {
    pub time: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasSeries {
    pub series_id: String,
    pub label: String,
    /// Visual primitive: "histogram", "line", or "area".
    pub chart_kind: String,
    /// Scale side: "right" or "left".
    pub price_scale: String,
    pub color: String,
    pub unit: String,
    pub data_layer: DataProcessingLayer,
    pub raw_value_preserved: bool,
    pub source: String,
    pub latest_date: Option<String>,
    pub stale_lag_days: Option<i64>,
    pub latest_value: Option<f64>,
    pub window_delta: Option<f64>,
    pub note: Option<String>,
    pub points: Vec<AtlasPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasTank {
    pub tank_id: String,
    pub title: String,
    pub subtitle: String,
    pub accounting_scope_note: String,
    pub staleness_warning: Option<String>,
    pub primary_series: AtlasSeries,
    pub overlay_series: Vec<AtlasSeries>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityAtlasResponse {
    pub as_of_date: String,
    pub range: String,
    pub range_start_date: String,
    pub window_weeks: usize,
    pub earliest_available_date: String,
    pub latest_available_date: String,
    pub tanks: Vec<AtlasTank>,
    pub diagnostic_tree: DiagnosticTreeReport,
}
