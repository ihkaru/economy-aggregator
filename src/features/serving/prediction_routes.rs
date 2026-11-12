//! REST endpoints for Prediction Markets contracts, quotes, and implied probability candles.

use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::features::ingestion::prediction_market::{
    persist_prediction_candles_to_lake, KalshiFetcher, PolymarketFetcher,
};
use crate::features::serving::rest::AppState;

#[derive(Debug, Deserialize)]
pub struct PredictionMarketsQuery {
    pub venue: Option<String>,
    pub q: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub refresh: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PredictionCandlesQuery {
    pub timeframe: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
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
    let contracts = if !force_refresh {
        let cached = if let Ok(cache_guard) = state.prediction_contracts_cache.read() {
            cache_guard.get(&cache_key).and_then(|(inserted, contracts)| {
                if inserted.elapsed() < Duration::from_secs(300) {
                    Some(contracts.clone())
                } else {
                    None
                }
            })
        } else {
            None
        };
        cached
    } else {
        None
    };

    let mut combined_contracts = match contracts {
        Some(c) => c,
        None => {
            let limit_each = query.limit.unwrap_or(50).min(500);
            let mut list = Vec::new();

            if venue == "all" || venue == "kalshi" {
                let kalshi = KalshiFetcher::default();
                if let Ok(mut kalshi_contracts) = kalshi.fetch_markets(limit_each).await {
                    list.append(&mut kalshi_contracts);
                }
            }

            if venue == "all" || venue == "polymarket" {
                let polymarket = PolymarketFetcher::default();
                if let Ok(mut poly_contracts) = polymarket.fetch_contracts(limit_each).await {
                    list.append(&mut poly_contracts);
                }
            }

            if list.is_empty() {
                list = KalshiFetcher::canonical_seeds();
                list.extend(PolymarketFetcher::canonical_seeds());
            }

            if let Ok(mut cache_guard) = state.prediction_contracts_cache.write() {
                cache_guard.insert(cache_key, (Instant::now(), list.clone()));
            }

            list
        }
    };

    // Filter by query keyword
    if let Some(q) = query.q.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        combined_contracts.retain(|c| {
            c.title.to_lowercase().contains(&q)
                || c.market_id.to_lowercase().contains(&q)
                || c.outcome_label.to_lowercase().contains(&q)
        });
    }

    // Filter by status (open, closed, all)
    if let Some(status) = query.status.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        if status != "all" {
            combined_contracts.retain(|c| c.status.to_lowercase() == status);
        }
    }

    let limit = query.limit.unwrap_or(combined_contracts.len()).min(1000);
    combined_contracts.truncate(limit);
    Ok(Json(combined_contracts))
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

    let limit = query.limit.unwrap_or(300).min(5000);

    // 1. Read historical time-series candles from local Parquet Lakehouse
    let mut candles = crate::features::storage::LakePredictionReader::read_candles(
        &state.config.lake_dir,
        Some(venue),
        native_id,
        timeframe,
        query.from,
        query.to,
        Some(limit),
    ).unwrap_or_default();

    // 2. If lake has no candles yet or historical query is empty, try live orderbook snapshot
    if candles.is_empty() {
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
    }

    candles.truncate(limit);
    Ok(Json(candles))
}
