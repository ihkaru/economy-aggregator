//! REST Routes for Persistent Supply-Chain Graph & Cross-Stock Lead-Lag Analysis.
//! - `GET    /v1/supply-chain`
//! - `POST   /v1/supply-chain/edges` (persists edge AND auto-subscribes both tickers)
//! - `DELETE /v1/supply-chain/edges`
//! - `GET    /v1/analytics/supply-chain`

use std::sync::Arc;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;

use crate::core::subscription::SubscriptionItem;
use crate::core::supply_chain::SupplyChainManager;
use crate::features::serving::rest::AppState;
use crate::runtime::{build_supply_chain_analysis, build_supply_chain_leaderboard};

#[derive(Debug, Deserialize)]
pub struct UpsertSupplyChainEdgeReq {
    pub target: String,
    pub supplier: String,
    pub relationship: Option<String>,
    pub tier: Option<u8>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteSupplyChainEdgeQuery {
    pub target: String,
    pub supplier: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct SupplyChainAnalysisQuery {
    pub target: Option<String>,
    pub horizon: Option<usize>,
    pub range: Option<String>,
    pub as_of: Option<String>,
}

pub async fn get_supply_chain_graph_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mgr = SupplyChainManager::new(&state.config.lake_dir);
    let graph = mgr
        .load_or_seed()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "success": true, "graph": graph })))
}

pub async fn upsert_supply_chain_edge_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertSupplyChainEdgeReq>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let target = req.target.trim().to_ascii_uppercase();
    let supplier = req.supplier.trim().to_ascii_uppercase();
    if target.is_empty() || supplier.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "target and supplier are required".to_string()));
    }
    let mgr = SupplyChainManager::new(&state.config.lake_dir);
    let (graph, action) = mgr
        .upsert_edge(
            &target,
            &supplier,
            req.relationship.as_deref().unwrap_or("supplier"),
            req.tier.unwrap_or(1),
            req.notes.as_deref().unwrap_or(""),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Auto-subscribe both target & supplier so they stay ingested in the lakehouse
    for sym in [&target, &supplier] {
        let _ = state.sub_manager.subscribe(SubscriptionItem {
            source: "yahoo".to_string(),
            symbol: sym.clone(),
            asset_class: "equities".to_string(),
            timeframes: vec!["1d".to_string()],
            enabled: true,
            poll_interval_secs: Some(300),
            created_at_utc: Utc::now().to_rfc3339(),
        });
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "action": action,
        "graph": graph,
        "auto_subscribed": [target, supplier]
    })))
}

pub async fn delete_supply_chain_edge_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DeleteSupplyChainEdgeQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mgr = SupplyChainManager::new(&state.config.lake_dir);
    let (graph, removed) = mgr
        .remove_edge(&q.target, &q.supplier)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({
        "success": true,
        "removed": removed,
        "graph": graph
    })))
}

pub async fn query_supply_chain_analysis_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SupplyChainAnalysisQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let target = q.target.unwrap_or_else(|| "ADI".to_string());
    let horizon = q.horizon.unwrap_or(21).clamp(5, 21);
    let range_days = match q.range.as_deref().map(str::trim) {
        Some("3m") => 90,
        Some("6m") => 180,
        Some("3y") => 365 * 3,
        Some("all") => 365 * 10,
        _ => 365,
    };
    let as_of = match q.as_of.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Utc::now().date_naive(),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid as_of date".to_string()))?,
    };
    let lake_dir = state.config.lake_dir.clone();
    let res = tokio::task::spawn_blocking(move || {
        build_supply_chain_analysis(&lake_dir, &target, as_of, horizon, range_days)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match res {
        Some(analysis) => Ok(Json(serde_json::json!({ "success": true, "analysis": analysis }))),
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Insufficient historical lakehouse data for target or its suppliers".to_string(),
        )),
    }
}

pub async fn query_supply_chain_leaderboard_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SupplyChainAnalysisQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let horizon = q.horizon.unwrap_or(21).clamp(5, 21);
    let as_of = match q.as_of.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Utc::now().date_naive(),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid as_of date".to_string()))?,
    };
    let lake_dir = state.config.lake_dir.clone();
    let lb = tokio::task::spawn_blocking(move || {
        build_supply_chain_leaderboard(&lake_dir, as_of, horizon)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "success": true, "leaderboard": lb })))
}
