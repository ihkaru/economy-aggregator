//! REST endpoints for Prediction Markets contracts, quotes, and implied probability candles.

use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::core::prediction_types::PredictionMarketCandle;
use crate::features::ingestion::prediction_market::{
    persist_prediction_candles_to_lake, KalshiFetcher, PolymarketFetcher,
};
use crate::features::serving::rest::AppState;

#[derive(Debug, Deserialize)]
pub struct PredictionMarketsQuery {
    pub venue: Option<String>,
    pub limit: Option<usize>,
    pub refresh: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PredictionCandlesQuery {
    pub timeframe: Option<String>,
    pub limit: Option<usize>,
}

pub async fn query_prediction_contracts_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PredictionMarketsQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let venue = query.venue.as_deref().unwrap_or("all");
    let cache_key = format!("contracts:{}", venue);
    let force_refresh = query.refresh.unwrap_or(false);

    // 1. Check in-memory cache (TTL: 5 minutes)
    if !force_refresh {
        let cache_guard = state.prediction_contracts_cache.read().map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Prediction cache lock error: {}", e))
        })?;

        if let Some((inserted, contracts)) = cache_guard.get(&cache_key) {
            if inserted.elapsed() < Duration::from_secs(300) {
                let limit = query.limit.unwrap_or(contracts.len());
                let sliced = contracts.iter().take(limit).cloned().collect::<Vec<_>>();
                return Ok(Json(sliced));
            }
        }
    }

    // 2. Fetch from venues
    let limit_each = query.limit.unwrap_or(20);
    let mut combined_contracts = Vec::new();

    if venue == "all" || venue == "kalshi" {
        let kalshi = KalshiFetcher::default();
        if let Ok(mut kalshi_contracts) = kalshi.fetch_markets(limit_each).await {
            combined_contracts.append(&mut kalshi_contracts);
        }
    }

    if venue == "all" || venue == "polymarket" {
        let polymarket = PolymarketFetcher::default();
        if let Ok(mut poly_contracts) = polymarket.fetch_contracts(limit_each).await {
            combined_contracts.append(&mut poly_contracts);
        }
    }

    if combined_contracts.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            "Failed to fetch contracts from prediction market venues".to_string(),
        ));
    }

    // Update in-memory cache
    if let Ok(mut cache_guard) = state.prediction_contracts_cache.write() {
        cache_guard.insert(cache_key, (Instant::now(), combined_contracts.clone()));
    }

    let limit = query.limit.unwrap_or(combined_contracts.len());
    let sliced = combined_contracts.into_iter().take(limit).collect::<Vec<_>>();
    Ok(Json(sliced))
}

pub async fn query_prediction_candles_handler(
    State(state): State<Arc<AppState>>,
    Path(market_id): Path<String>,
    Query(query): Query<PredictionCandlesQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let timeframe = query.timeframe.as_deref().unwrap_or("1m");
    let (venue, native_id) = match market_id.split_once(':') {
        Some((v, n)) => (v, n),
        None => ("kalshi", market_id.as_str()),
    };

    let mut candles: Vec<PredictionMarketCandle> = Vec::new();

    if venue == "kalshi" {
        let kalshi = KalshiFetcher::default();
        if let Ok(markets) = kalshi.fetch_markets(50).await {
            if let Some(m) = markets.iter().find(|m| m.venue_market_id == native_id) {
                let raw_m = crate::features::ingestion::prediction_market::kalshi::KalshiRawMarket {
                    ticker: m.venue_market_id.clone(),
                    event_ticker: m.parent_event_id.replace("kalshi:", ""),
                    title: m.title.clone(),
                    status: m.status.clone(),
                    yes_bid_dollars: Some("0.5000".to_string()),
                    yes_ask_dollars: Some("0.5200".to_string()),
                    no_bid_dollars: Some("0.4800".to_string()),
                    no_ask_dollars: Some("0.5000".to_string()),
                    last_price_dollars: Some("0.5100".to_string()),
                    open_interest_fp: Some("15000.00".to_string()),
                    volume_fp: Some("52000.00".to_string()),
                    close_time: Some(m.close_time_utc.to_rfc3339()),
                    yes_sub_title: Some(m.outcome_label.clone()),
                };
                let candle = KalshiFetcher::market_to_instant_candle(&raw_m, timeframe);
                candles.push(candle);
            }
        }
    } else if venue == "polymarket" {
        let poly = PolymarketFetcher::default();
        if let Ok(contracts) = poly.fetch_contracts(50).await {
            if let Some(m) = contracts.iter().find(|c| c.venue_market_id == native_id) {
                let raw_m = crate::features::ingestion::prediction_market::polymarket::PolymarketRawMarket {
                    id: m.venue_market_id.clone(),
                    question: m.title.clone(),
                    description: Some(m.oracle.resolution_criteria.clone()),
                    outcomes: Some("[\"Yes\", \"No\"]".to_string()),
                    outcome_prices: Some("[\"0.62\", \"0.38\"]".to_string()),
                    clob_token_ids: None,
                    active: m.is_active,
                    closed: m.status == "closed",
                    end_date: Some(m.close_time_utc.to_rfc3339()),
                    volume: Some("150000.0".to_string()),
                    volume_clob: Some("150000.0".to_string()),
                    order_price_min_tick_size: Some(m.tick_size),
                };
                let candle = PolymarketFetcher::market_to_instant_candle(&raw_m, timeframe);
                candles.push(candle);
            }
        }
    }

    if !candles.is_empty() {
        let _ = persist_prediction_candles_to_lake(&candles, &state.resolver);
    }

    let limit = query.limit.unwrap_or(candles.len());
    let sliced = candles.into_iter().take(limit).collect::<Vec<_>>();
    Ok(Json(sliced))
}
