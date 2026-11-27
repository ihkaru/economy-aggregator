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

#[derive(Debug, Default, Deserialize)]
pub struct CatchUpTriggerRequest {
    pub symbol: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default, rename = "async")]
    pub run_async: bool,
}

fn invalidate_serving_caches(state: &AppState) {
    if let Ok(mut c) = state.candle_cache.write() { c.clear(); }
    if let Ok(mut c) = state.macro_cache.write() { c.clear(); }
    if let Ok(mut c) = state.catalog_cache.write() { *c = None; }
    if let Ok(mut c) = state.partitions_cache.write() { *c = None; }
}

pub async fn trigger_catch_up_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CatchUpTriggerRequest>,
    body: Option<Json<CatchUpTriggerRequest>>,
) -> impl IntoResponse {
    let req = body.map(|Json(b)| b).unwrap_or(query);
    let symbol = req.symbol.clone();
    let source = req.source.clone();
    let force = req.force;

    if req.run_async {
        let state_bg = state.clone();
        tokio::spawn(async move {
            state_bg
                .orchestrator
                .trigger_catch_up_filtered(symbol.as_deref(), source.as_deref(), force)
                .await;
            invalidate_serving_caches(&state_bg);
        });
        return (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "success": true,
                "status": "accepted",
                "mode": "async",
                "filter": { "symbol": req.symbol, "source": req.source, "force": force },
                "message": "Gap catch-up triggered asynchronously in background"
            })),
        );
    }

    let summaries = state
        .orchestrator
        .trigger_catch_up_filtered(symbol.as_deref(), source.as_deref(), force)
        .await;
    invalidate_serving_caches(&state);
    let total: usize = summaries.iter().map(|s| s.candles_ingested).sum();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "status": "completed",
            "mode": "sync",
            "filter": { "symbol": req.symbol, "source": req.source, "force": force },
            "total_candles_ingested": total,
            "summaries": summaries,
            "message": "Gap catch-up executed successfully"
        })),
    )
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
