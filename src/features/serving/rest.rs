//! REST API serving endpoints for market candles, instruments catalog, and health.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use tower_http::services::{ServeDir, ServeFile};

use crate::core::config::LakehouseConfig;
use crate::core::macro_types::RichEconomicEvent;
use crate::core::prediction_types::PredictionMarketContract;
use crate::core::types::{AggregatorCandle, AssetClass, TimeFrame};
use crate::features::serving::analytics_routes::*;
use crate::features::serving::catalog_routes::*;
use crate::features::serving::macro_routes::query_macro_calendar_handler;
use crate::features::serving::prediction_routes::{
    query_prediction_candles_handler, query_prediction_contracts_handler,
};
use crate::features::serving::types::*;
use crate::features::storage::{
    CandleResampler, InstrumentRegistry, LakeReader, PartitionResolver,
};

pub struct AppState {
    pub config: LakehouseConfig,
    pub resolver: PartitionResolver,
    pub registry: RwLock<InstrumentRegistry>,
    pub start_time: Instant,
    pub candle_cache: RwLock<HashMap<String, (Instant, Vec<AggregatorCandle>)>>,
    pub macro_cache: RwLock<HashMap<String, (Instant, Vec<RichEconomicEvent>)>>,
    pub prediction_contracts_cache: RwLock<HashMap<String, (Instant, Vec<PredictionMarketContract>)>>,
}

impl AppState {
    pub fn new(
        config: LakehouseConfig,
        resolver: PartitionResolver,
        registry: InstrumentRegistry,
    ) -> Self {
        Self {
            config,
            resolver,
            registry: RwLock::new(registry),
            start_time: Instant::now(),
            candle_cache: RwLock::new(HashMap::new()),
            macro_cache: RwLock::new(HashMap::new()),
            prediction_contracts_cache: RwLock::new(HashMap::new()),
        }
    }
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let api_router = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/health", get(health_handler))
        .route("/v1/instruments", get(list_instruments_handler))
        .route("/v1/market/catalog", get(market_catalog_handler))
        .route("/v1/market/metadata", get(market_metadata_handler))
        .route("/v1/market/candles", get(query_candles_handler))
        .route("/v1/storage/stats", get(storage_stats_handler))
        .route("/v1/storage/partitions", get(storage_partitions_handler))
        .route("/v1/macro/calendar", get(query_macro_calendar_handler))
        .route("/v1/prediction-markets/events", get(query_prediction_contracts_handler))
        .route("/v1/prediction-markets/markets/:id/candles", get(query_prediction_candles_handler))
        .route("/v1/analytics/divergence", get(query_divergence_handler))
        .route("/v1/integrity/audit", get(integrity_audit_handler))
        .with_state(state);

    let web_dist = Path::new("web/dist");
    if web_dist.exists() {
        let serve_dir = ServeDir::new(web_dist)
            .not_found_service(ServeFile::new(web_dist.join("index.html")));
        api_router.fallback_service(serve_dir)
    } else {
        api_router
    }
}

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    Json(HealthResponse {
        status: "ok",
        service: "economy-aggregator",
        version: "0.1.0",
        uptime_secs: uptime,
    })
}

pub async fn query_candles_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CandleQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let asset_class = match query.asset_class.to_lowercase().as_str() {
        "crypto" => AssetClass::Crypto,
        "equities" => AssetClass::Equities,
        "forex" => AssetClass::Forex,
        "commodities" => AssetClass::Commodities,
        "macro" => AssetClass::Macro,
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid asset_class".to_string())),
    };
    let timeframe = match TimeFrame::from_str_opt(&query.timeframe) {
        Some(tf) => tf,
        None => return Err((StatusCode::BAD_REQUEST, "Invalid timeframe".to_string())),
    };

    let (start_date, end_date) = match (query.year, query.month) {
        (Some(y), Some(m)) => {
            let s = NaiveDate::from_ymd_opt(y, m, 1);
            let next_m = if m == 12 { 1 } else { m + 1 };
            let next_y = if m == 12 { y + 1 } else { y };
            let e = NaiveDate::from_ymd_opt(next_y, next_m, 1).and_then(|d| d.pred_opt());
            (s, e)
        }
        (Some(y), None) => {
            let s = NaiveDate::from_ymd_opt(y, 1, 1);
            let e = NaiveDate::from_ymd_opt(y, 12, 31);
            (s, e)
        }
        _ => (None, None),
    };

    let clean_symbol = query.symbol.to_uppercase().replace('/', "_");
    let cache_key = format!("{}:{}:{}", asset_class.as_str(), clean_symbol, timeframe.as_str());

    // Check fast in-memory cache if no arbitrary historical date filter and no to cutoff
    if start_date.is_none() && end_date.is_none() && query.to.is_none() {
        if let Ok(cache) = state.candle_cache.read() {
            if let Some((inserted_at, cached_candles)) = cache.get(&cache_key) {
                if inserted_at.elapsed() < Duration::from_secs(60) {
                    let mut res = cached_candles.clone();
                    let lim = query.limit.unwrap_or(300);
                    if res.len() > lim {
                        let start_idx = res.len() - lim;
                        res = res.split_off(start_idx);
                    }
                    return Ok(Json(res));
                }
            }
        }
    }

    let default_limit = query.limit.or(Some(300));
    let mut candles = LakeReader::read_candles_with_limit(
        &state.config.lake_dir,
        Some(asset_class.as_str()),
        &query.symbol,
        timeframe.as_str(),
        start_date,
        end_date,
        default_limit,
        query.to,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Dynamic Resampling: If candles are empty for weekly or monthly, resample from 1d daily lake
    if candles.is_empty() && (timeframe == TimeFrame::W1 || timeframe == TimeFrame::MN1) {
        let daily_limit = default_limit.map(|l| if timeframe == TimeFrame::MN1 { l * 31 } else { l * 7 });
        if let Ok(daily_candles) = LakeReader::read_candles_with_limit(
            &state.config.lake_dir,
            Some(asset_class.as_str()),
            &query.symbol,
            "1d",
            start_date,
            end_date,
            daily_limit,
            query.to,
        ) {
            candles = CandleResampler::resample(&daily_candles, timeframe);
        }
    }

    // Cache resolved candles in memory if full initial view
    if start_date.is_none() && end_date.is_none() && query.to.is_none() && !candles.is_empty() {
        if let Ok(mut cache) = state.candle_cache.write() {
            cache.insert(cache_key, (Instant::now(), candles.clone()));
        }
    }

    // Apply limit: keep the most recent candles for charting
    if let Some(lim) = default_limit {
        if candles.len() > lim {
            let start_idx = candles.len() - lim;
            candles = candles.split_off(start_idx);
        }
    }

    Ok(Json(candles))
}
