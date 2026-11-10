//! REST handlers for managing ingestion subscriptions and orchestrator state.

use std::sync::Arc;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::core::subscription::SubscriptionItem;
use crate::features::ingestion::search::{SearchResultItem, UnifiedSearchEngine};
use crate::features::serving::rest::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQueryParams {
    pub q: String,
    pub source: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn list_subscriptions_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let items = state.sub_manager.load().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed reading subscriptions: {}", e))
    })?;
    Ok(Json(ApiResponse { success: true, data: items }))
}

pub async fn add_subscription_handler(
    State(state): State<Arc<AppState>>,
    Json(item): Json<SubscriptionItem>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let updated = state.sub_manager.subscribe(item.clone()).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed persisting subscription: {}", e))
    })?;

    // Asynchronously trigger catch-up for newly subscribed item
    let orch = state.orchestrator.clone();
    tokio::spawn(async move {
        orch.trigger_catch_up().await;
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "action": if updated { "updated" } else { "created" },
        "subscription": item
    })))
}

#[derive(Debug, Deserialize)]
pub struct DeleteSubscriptionQuery {
    pub source: Option<String>,
}

pub async fn delete_subscription_handler(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query): Query<DeleteSubscriptionQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let removed = state.sub_manager.unsubscribe(query.source.as_deref(), &symbol).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed removing subscription: {}", e))
    })?;

    Ok(Json(serde_json::json!({
        "success": removed,
        "symbol": symbol,
        "source": query.source,
        "message": if removed { "Subscription deleted" } else { "Subscription not found" }
    })))
}

pub async fn ingestion_status_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let status = state.orchestrator.status();
    Json(ApiResponse { success: true, data: status })
}

pub async fn trigger_catch_up_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let orch = state.orchestrator.clone();
    let total = orch.trigger_catch_up().await;
    Json(serde_json::json!({
        "success": true,
        "total_candles_ingested": total,
        "message": "Gap catch-up executed successfully"
    }))
}

pub async fn search_symbols_handler(
    Query(params): Query<SearchQueryParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if params.q.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Query parameter 'q' cannot be empty".to_string()));
    }

    let limit = params.limit.unwrap_or(5);
    let engine = UnifiedSearchEngine::default();
    let results: Vec<SearchResultItem> = match params.source.as_deref().unwrap_or("all") {
        "yahoo" | "equities" => engine.search_yahoo(&params.q, limit).await,
        "binance" | "crypto" => engine.search_binance(&params.q, limit).await,
        "polymarket" => engine.search_polymarket(&params.q, limit).await,
        "kalshi" => engine.search_kalshi(&params.q, limit).await,
        "tradingview" | "tv" => engine.search_tradingview(&params.q, limit).await,
        _ => engine.search_all(&params.q, limit).await,
    };

    Ok(Json(ApiResponse { success: true, data: results }))
}
