//! REST API Route for SMH (VanEck Semiconductor ETF) 5-Day Predictive & Risk Analytics.
//! Exposes `GET /v1/analytics/smh-predictive`.

use std::sync::Arc;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;

use crate::features::serving::rest::AppState;
use crate::runtime::build_smh_predictive;

#[derive(Debug, Deserialize, Default)]
pub struct SmhPredictiveQuery {
    pub as_of: Option<String>,
    pub range: Option<String>,
}

fn parse_as_of(raw: Option<&str>) -> Result<NaiveDate, (StatusCode, String)> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(Utc::now().date_naive()),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid as_of '{}', expected YYYY-MM-DD", s),
            )
        }),
    }
}

fn parse_range_days(raw: Option<&str>) -> usize {
    match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("3m") | Some("12w") => 90,
        Some("6m") | Some("26w") => 180,
        Some("3y") => 365 * 3,
        Some("5y") => 365 * 5,
        Some("all") | Some("max") => 365 * 10,
        _ => 365,
    }
}

pub async fn query_smh_predictive_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SmhPredictiveQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let as_of = parse_as_of(q.as_of.as_deref())?;
    let range_days = parse_range_days(q.range.as_deref());
    let lake_dir = state.config.lake_dir.clone();

    let result = tokio::task::spawn_blocking(move || build_smh_predictive(&lake_dir, as_of, range_days))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match result {
        Some(predictive) => Ok(Json(serde_json::json!({
            "success": true,
            "predictive": predictive
        }))),
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Insufficient SMH lakehouse history (minimum 320 daily bars required)".to_string(),
        )),
    }
}
