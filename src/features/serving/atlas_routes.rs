//! REST API Route for the 6-Tank Liquidity & Capital Flow Atlas (Phase 363).
//! Exposes `GET /v1/analytics/liquidity-atlas` with zero-lookahead historical `as_of` filtering.

use std::sync::Arc;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;

use crate::features::serving::rest::AppState;
use crate::runtime::build_liquidity_atlas;

#[derive(Debug, Deserialize, Default)]
pub struct LiquidityAtlasQuery {
    pub as_of: Option<String>,
    pub range: Option<String>,
    pub window_weeks: Option<usize>,
    pub include_spillover: Option<bool>,
}

fn parse_as_of(raw: Option<&str>) -> Result<NaiveDate, (StatusCode, String)> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(Utc::now().date_naive()),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| (StatusCode::BAD_REQUEST, format!("Invalid as_of '{}', expected YYYY-MM-DD", s))),
    }
}

pub async fn query_liquidity_atlas_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LiquidityAtlasQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let as_of = parse_as_of(q.as_of.as_deref())?;
    let range = q.range.unwrap_or_else(|| "1y".to_string());
    let window_weeks = q.window_weeks.unwrap_or(4).clamp(1, 52);
    let include_spillover = q.include_spillover.unwrap_or(false);
    let lake_dir = state.config.lake_dir.clone();

    let atlas = tokio::task::spawn_blocking(move || {
        build_liquidity_atlas(&lake_dir, as_of, &range, window_weeks, include_spillover)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "atlas": atlas
    })))
}
