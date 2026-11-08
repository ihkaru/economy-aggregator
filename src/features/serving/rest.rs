//! REST API serving endpoints for market candles, instruments catalog, and health.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tower_http::services::{ServeDir, ServeFile};

use crate::core::config::LakehouseConfig;
use crate::core::types::{AggregatorCandle, AssetClass, TimeFrame};
use crate::features::storage::{
    friendly_symbol_name, resolve_instrument_specs, CandleResampler,
    InstrumentMapping, InstrumentRegistry, LakeReader, PartitionResolver,
};

pub struct AppState {
    pub config: LakehouseConfig,
    pub resolver: PartitionResolver,
    pub registry: RwLock<InstrumentRegistry>,
    pub start_time: Instant,
}

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

pub fn create_router(state: Arc<AppState>) -> Router {
    let api_router = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/instruments", get(list_instruments_handler))
        .route("/v1/market/catalog", get(market_catalog_handler))
        .route("/v1/market/metadata", get(market_metadata_handler))
        .route("/v1/market/candles", get(query_candles_handler))
        .route("/v1/storage/stats", get(storage_stats_handler))
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

async fn list_instruments_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<InstrumentMapping>> {
    let reg = state.registry.read().unwrap();
    Json(reg.list_all())
}

async fn market_catalog_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<InstrumentCatalogItem>> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    let reg = state.registry.read().unwrap();
    for m in reg.list_all() {
        let src = m
            .vendor_symbols
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "lakehouse".to_string());
        seen.insert(m.canonical_symbol.clone());
        let (currency, precision, min_tick, supported_tf, _) =
            resolve_instrument_specs(&m.canonical_symbol, m.asset_class.as_str());
        items.push(InstrumentCatalogItem {
            symbol: m.canonical_symbol.clone(),
            canonical_symbol: m.canonical_symbol.clone(),
            name: m.name.clone(),
            asset_class: m.asset_class.as_str().to_string(),
            currency,
            price_precision: precision,
            min_tick,
            supported_timeframes: supported_tf,
            default_timeframe: "1d".to_string(),
            source: src,
            active: true,
        });
    }

    // Auto-discover partitions from lakehouse directory
    let market_dir = state.config.lake_dir.join("market");
    if let Ok(ac_entries) = fs::read_dir(market_dir) {
        for ac_entry in ac_entries.flatten() {
            let ac_path = ac_entry.path();
            if ac_path.is_dir() {
                let ac_name = ac_path.file_name().unwrap_or_default().to_string_lossy();
                if let Some(ac_stripped) = ac_name.strip_prefix("asset_class=") {
                    if let Ok(sym_entries) = fs::read_dir(&ac_path) {
                        for sym_entry in sym_entries.flatten() {
                            let sym_name = sym_entry.file_name().to_string_lossy().to_string();
                            if let Some(sym_stripped) = sym_name.strip_prefix("symbol=") {
                                let canonical = sym_stripped.replace('_', "/");
                                if seen.insert(canonical.clone()) {
                                    let friendly_name = friendly_symbol_name(sym_stripped);
                                    let (currency, precision, min_tick, supported_tf, _) =
                                        resolve_instrument_specs(&canonical, ac_stripped);
                                    items.push(InstrumentCatalogItem {
                                        symbol: canonical.clone(),
                                        canonical_symbol: canonical,
                                        name: friendly_name.to_string(),
                                        asset_class: ac_stripped.to_string(),
                                        currency,
                                        price_precision: precision,
                                        min_tick,
                                        supported_timeframes: supported_tf,
                                        default_timeframe: "1d".to_string(),
                                        source: "parquet_lake".to_string(),
                                        active: true,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    items.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    Json(items)
}

async fn market_metadata_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MetadataQuery>,
) -> Result<Json<InstrumentMetadata>, (StatusCode, String)> {
    let clean_symbol = query.symbol.to_uppercase().replace('/', "_");

    let asset_class = if let Some(ac) = query.asset_class {
        ac.to_lowercase()
    } else {
        let mut detected = "equities".to_string();
        let known = ["equities", "crypto", "forex", "commodities", "macro"];
        let market_dir = state.config.lake_dir.join("market");
        for k in known {
            let p = market_dir.join(format!("asset_class={}", k)).join(format!("symbol={}", clean_symbol));
            if p.exists() {
                detected = k.to_string();
                break;
            }
        }
        detected
    };

    let (currency, precision, min_tick, supported_tf, has_intraday) =
        resolve_instrument_specs(&clean_symbol, &asset_class);

    let friendly_name = friendly_symbol_name(&clean_symbol);

    let candles = LakeReader::read_candles(
        &state.config.lake_dir,
        Some(&asset_class),
        &clean_symbol,
        "1d",
        None,
        None,
    ).unwrap_or_default();

    let total_bars = candles.len();
    let first_date = candles.first().map(|c| c.timestamp_utc.format("%Y-%m-%d").to_string());
    let last_date = candles.last().map(|c| c.timestamp_utc.format("%Y-%m-%d").to_string());

    Ok(Json(InstrumentMetadata {
        symbol: query.symbol.clone(),
        canonical_symbol: query.symbol.replace('_', "/"),
        name: friendly_name.to_string(),
        asset_class,
        currency,
        price_precision: precision,
        min_tick,
        supported_timeframes: supported_tf,
        default_timeframe: "1d".to_string(),
        has_intraday,
        has_daily: true,
        has_weekly_and_monthly: true,
        total_bars,
        first_date,
        last_date,
    }))
}

async fn storage_stats_handler(
    State(state): State<Arc<AppState>>,
) -> Json<StorageStatsResponse> {
    let mut total_chunks = 0;
    let mut total_bytes = 0;

    let market_dir = state.config.lake_dir.join("market");
    if market_dir.exists() {
        count_parquet_recursive(&market_dir, &mut total_chunks, &mut total_bytes);
    }

    Json(StorageStatsResponse {
        lake_dir: state.config.lake_dir.display().to_string(),
        total_chunks_found: total_chunks,
        total_bytes,
    })
}

fn count_parquet_recursive(dir: &Path, count: &mut usize, bytes: &mut u64) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                count_parquet_recursive(&p, count, bytes);
            } else if p.extension().map_or(false, |ext| ext == "parquet") {
                *count += 1;
                if let Ok(meta) = entry.metadata() {
                    *bytes += meta.len();
                }
            }
        }
    }
}

async fn query_candles_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CandleQuery>,
) -> Result<Json<Vec<AggregatorCandle>>, (StatusCode, String)> {
    let asset_class = match query.asset_class.to_lowercase().as_str() {
        "crypto" => AssetClass::Crypto,
        "equities" => AssetClass::Equities,
        "forex" => AssetClass::Forex,
        "commodities" => AssetClass::Commodities,
        "macro" => AssetClass::Macro,
        _ => return Err((StatusCode::BAD_REQUEST, format!("Unknown asset class: {}", query.asset_class))),
    };

    let timeframe = match TimeFrame::from_str_opt(&query.timeframe) {
        Some(tf) => tf,
        None => return Err((StatusCode::BAD_REQUEST, format!("Unknown timeframe: {}", query.timeframe))),
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

    let mut candles = LakeReader::read_candles(
        &state.config.lake_dir,
        Some(asset_class.as_str()),
        &query.symbol,
        timeframe.as_str(),
        start_date,
        end_date,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Dynamic Resampling: If candles are empty for weekly or monthly, resample from 1d daily lake
    if candles.is_empty() && (timeframe == TimeFrame::W1 || timeframe == TimeFrame::MN1) {
        if let Ok(daily_candles) = LakeReader::read_candles(
            &state.config.lake_dir,
            Some(asset_class.as_str()),
            &query.symbol,
            "1d",
            start_date,
            end_date,
        ) {
            candles = CandleResampler::resample(&daily_candles, timeframe);
        }
    }

    // Apply limit: keep the most recent candles for charting
    if let Some(lim) = query.limit {
        if candles.len() > lim {
            let start_idx = candles.len() - lim;
            candles = candles.split_off(start_idx);
        }
    }

    Ok(Json(candles))
}
