//! REST API serving endpoints for market candles, instruments metadata, and health.

use std::fs::{self, File};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};

use crate::core::config::LakehouseConfig;
use crate::core::types::{AggregatorCandle, AssetClass, IngestionMode, TimeFrame};
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

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/v1/instruments", get(list_instruments_handler))
        .route("/v1/market/candles", get(query_candles_handler))
        .route("/v1/storage/stats", get(storage_stats_handler))
        .with_state(state)
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

fn count_parquet_recursive(dir: &std::path::Path, count: &mut usize, bytes: &mut u64) {
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
                            if let Ok(parsed) = parse_batch_to_candles(&batch) {
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

fn parse_batch_to_candles(
    batch: &arrow::record_batch::RecordBatch,
) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
    use arrow::array::*;
    use arrow::datatypes::{DataType, TimeUnit};
    use chrono::{DateTime, Utc};

    let ts_col = batch.column(0);
    let ts_micros: Vec<i64> = match ts_col.data_type() {
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            let arr = ts_col.as_any().downcast_ref::<TimestampNanosecondArray>().ok_or("Failed downcast ts nanos")?;
            (0..batch.num_rows()).map(|i| arr.value(i) / 1_000).collect()
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            let arr = ts_col.as_any().downcast_ref::<TimestampMicrosecondArray>().ok_or("Failed downcast ts micros")?;
            (0..batch.num_rows()).map(|i| arr.value(i)).collect()
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            let arr = ts_col.as_any().downcast_ref::<TimestampMillisecondArray>().ok_or("Failed downcast ts millis")?;
            (0..batch.num_rows()).map(|i| arr.value(i) * 1_000).collect()
        }
        dt => return Err(format!("Unexpected timestamp data type: {:?}", dt).into()),
    };

    let extract_str = |col_idx: usize, row_idx: usize| -> String {
        let col = batch.column(col_idx);
        if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
            return arr.value(row_idx).to_string();
        }
        if let Some(arr) = col.as_any().downcast_ref::<LargeStringArray>() {
            return arr.value(row_idx).to_string();
        }
        if let Some(dict) = col.as_any().downcast_ref::<DictionaryArray<arrow::datatypes::Int32Type>>() {
            if let Some(values) = dict.values().as_any().downcast_ref::<StringArray>() {
                if let Some(key) = dict.key(row_idx) {
                    return values.value(key).to_string();
                }
            }
        }
        String::new()
    };

    let open_array = batch.column(3).as_any().downcast_ref::<Float64Array>().ok_or("Invalid open col")?;
    let high_array = batch.column(4).as_any().downcast_ref::<Float64Array>().ok_or("Invalid high col")?;
    let low_array = batch.column(5).as_any().downcast_ref::<Float64Array>().ok_or("Invalid low col")?;
    let close_array = batch.column(6).as_any().downcast_ref::<Float64Array>().ok_or("Invalid close col")?;
    let vol_base_array = batch.column(7).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vol_base col")?;
    let vol_quote_array = batch.column(8).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vol_quote col")?;
    let taker_base_array = batch.column(9).as_any().downcast_ref::<Float64Array>().ok_or("Invalid taker_base col")?;
    let taker_quote_array = batch.column(10).as_any().downcast_ref::<Float64Array>().ok_or("Invalid taker_quote col")?;
    let is_settled_array = batch.column(14).as_any().downcast_ref::<BooleanArray>().ok_or("Invalid is_settled col")?;
    let sequence_id_array = batch.column(15).as_any().downcast_ref::<UInt64Array>().ok_or("Invalid seq col")?;
    let vwap_array = batch.column(16).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vwap col")?;
    let trade_count_array = batch.column(17).as_any().downcast_ref::<UInt32Array>().ok_or("Invalid trade_count col")?;

    let mut candles = Vec::with_capacity(batch.num_rows());
    for i in 0..batch.num_rows() {
        let ts = DateTime::from_timestamp_micros(ts_micros[i]).unwrap_or_else(Utc::now);
        let tf_str = extract_str(2, i);
        let timeframe = TimeFrame::from_str_opt(&tf_str).unwrap_or(TimeFrame::M1);

        let mode_str = extract_str(13, i);
        let ingestion_mode = match mode_str.as_str() {
            "rest_batch" => IngestionMode::RestBatch,
            "reconciliation" => IngestionMode::Reconciliation,
            _ => IngestionMode::LiveWs,
        };

        candles.push(AggregatorCandle {
            timestamp_utc: ts,
            canonical_symbol: extract_str(1, i),
            timeframe,
            open: open_array.value(i),
            high: high_array.value(i),
            low: low_array.value(i),
            close: close_array.value(i),
            volume_base: vol_base_array.value(i),
            volume_quote: vol_quote_array.value(i),
            taker_buy_volume_base: if taker_base_array.is_null(i) { None } else { Some(taker_base_array.value(i)) },
            taker_buy_volume_quote: if taker_quote_array.is_null(i) { None } else { Some(taker_quote_array.value(i)) },
            source: extract_str(11, i),
            raw_symbol: extract_str(12, i),
            ingestion_mode,
            is_settled: is_settled_array.value(i),
            sequence_id: if sequence_id_array.is_null(i) { None } else { Some(sequence_id_array.value(i)) },
            vwap: if vwap_array.is_null(i) { None } else { Some(vwap_array.value(i)) },
            trade_count: if trade_count_array.is_null(i) { None } else { Some(trade_count_array.value(i)) },
            price_type: extract_str(18, i),
        });
    }

    Ok(candles)
}
