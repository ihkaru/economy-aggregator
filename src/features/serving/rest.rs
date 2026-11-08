//! REST API serving endpoints for market candles, instruments catalog, and health.

use std::fs::{self, File};
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};
use tower_http::services::{ServeDir, ServeFile};

use crate::core::config::LakehouseConfig;
use crate::core::types::{AggregatorCandle, AssetClass, TimeFrame};
use crate::features::storage::arrow_schema::record_batch_to_candles;
use crate::features::storage::{InstrumentMapping, InstrumentRegistry, PartitionResolver};

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
    pub year: i32,
    pub month: u32,
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
    let reg = state.registry.read().unwrap();
    let mappings = reg.list_all();
    let items: Vec<InstrumentCatalogItem> = mappings
        .into_iter()
        .map(|m| {
            let src = m
                .vendor_symbols
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "lakehouse".to_string());
            InstrumentCatalogItem {
                symbol: m.canonical_symbol.clone(),
                canonical_symbol: m.canonical_symbol.clone(),
                name: m.name.clone(),
                asset_class: m.asset_class.as_str().to_string(),
                source: src,
                active: true,
            }
        })
        .collect();
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

    let clean_symbol = query.symbol.to_uppercase().replace('/', "_").replace(':', "_");
    let year_str = format!("{:04}", query.year);
    let month_str = format!("{:02}", query.month);

    let part_dir = state.config.lake_dir
        .join("market")
        .join(format!("asset_class={}", asset_class.as_str()))
        .join(format!("symbol={}", clean_symbol))
        .join(format!("timeframe={}", timeframe.as_str()))
        .join(format!("year={}", year_str))
        .join(format!("month={}", month_str));

    if !part_dir.exists() {
        return Ok(Json(Vec::new()));
    }

    let mut candles = Vec::new();
    let entries = match fs::read_dir(&part_dir) {
        Ok(e) => e,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "parquet") {
            if let Ok(file) = File::open(&path) {
                if let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) {
                    if let Ok(mut reader) = builder.build() {
                        while let Some(Ok(batch)) = reader.next() {
                            if let Ok(parsed) = record_batch_to_candles(&batch) {
                                candles.extend(parsed);
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort chronologically
    candles.sort_by_key(|c| c.timestamp_utc);

    // Apply limit if specified
    if let Some(lim) = query.limit {
        if candles.len() > lim {
            candles.truncate(lim);
        }
    }

    Ok(Json(candles))
}
