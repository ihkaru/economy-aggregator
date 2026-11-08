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
    pub source: String,
    pub active: bool,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let api_router = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/instruments", get(list_instruments_handler))
        .route("/v1/market/catalog", get(market_catalog_handler))
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
        items.push(InstrumentCatalogItem {
            symbol: m.canonical_symbol.clone(),
            canonical_symbol: m.canonical_symbol.clone(),
            name: m.name.clone(),
            asset_class: m.asset_class.as_str().to_string(),
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
                                    let friendly_name = match sym_stripped {
                                        "GSPC" => "S&P 500 Index",
                                        "BBCA" => "Bank Central Asia",
                                        "JKSE" => "Jakarta Composite Index (IHSG)",
                                        "N225" => "Nikkei 225 Index",
                                        "TLT" => "iShares 20+ Year Treasury Bond",
                                        "IEF" => "iShares 7-10 Year Treasury Bond",
                                        "LQD" => "iShares Investment Grade Corporate Bond",
                                        _ => sym_stripped,
                                    };
                                    items.push(InstrumentCatalogItem {
                                        symbol: canonical.clone(),
                                        canonical_symbol: canonical,
                                        name: friendly_name.to_string(),
                                        asset_class: ac_stripped.to_string(),
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

    // Apply limit: keep the most recent candles for charting
    if let Some(lim) = query.limit {
        if candles.len() > lim {
            let start_idx = candles.len() - lim;
            candles = candles.split_off(start_idx);
        }
    }

    Ok(Json(candles))
}
