//! Performance & Latency SLA Test Suite
//! Validates execution latency thresholds across ingestion, Arrow columnar conversion,
//! Parquet lake I/O, dynamic resampling, and REST serving endpoints.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::types::{
    AggregatorCandle, AssetClass, IngestionMode, InstrumentType, TimeFrame,
};
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    candles_to_record_batch, record_batch_to_candles, CandleResampler,
    InstrumentMapping, InstrumentRegistry, LakeReader, LakeWriter, PartitionResolver,
};

fn generate_synthetic_candles(count: usize) -> Vec<AggregatorCandle> {
    let base_time = Utc.with_ymd_and_hms(2004, 1, 1, 0, 0, 0).unwrap();
    let mut candles = Vec::with_capacity(count);

    for i in 0..count {
        let dt = base_time + chrono::Duration::days(i as i64);
        candles.push(AggregatorCandle {
            time: dt.timestamp(),
            timestamp_utc: dt,
            canonical_symbol: "BBCA".to_string(),
            timeframe: TimeFrame::D1,
            open: 5000.0 + (i as f64 * 0.5),
            high: 5050.0 + (i as f64 * 0.5),
            low: 4950.0 + (i as f64 * 0.5),
            close: 5020.0 + (i as f64 * 0.5),
            volume_base: 100_000.0,
            volume_quote: 502_000_000.0,
            taker_buy_volume_base: None,
            taker_buy_volume_quote: None,
            source: "synthetic".to_string(),
            raw_symbol: "BBCA.JK".to_string(),
            ingestion_mode: IngestionMode::RestBatch,
            is_settled: true,
            sequence_id: None,
            vwap: None,
            trade_count: None,
            price_type: "last".to_string(),
            raw_metadata: None,
        });
    }

    candles
}

#[test]
fn test_sla_resampling_latency_5000_bars() {
    // 5,000 daily candles corresponds to ~20 years of daily market trading data
    let daily_candles = generate_synthetic_candles(5000);

    // 1. Weekly Resampling SLA (must complete in < 25ms)
    let t0 = Instant::now();
    let weekly = CandleResampler::resample_to_weekly(&daily_candles);
    let elapsed_weekly = t0.elapsed();

    println!("⏱️  Resample 5,000 bars to Weekly (1w): {:?}", elapsed_weekly);
    assert!(!weekly.is_empty());
    assert!(
        elapsed_weekly.as_millis() < 25,
        "Weekly resampling of 5,000 bars took {:?}, exceeding 25ms SLA",
        elapsed_weekly
    );

    // 2. Monthly Resampling SLA (must complete in < 25ms)
    let t1 = Instant::now();
    let monthly = CandleResampler::resample_to_monthly(&daily_candles);
    let elapsed_monthly = t1.elapsed();
    println!("⏱️  Resample 5,000 bars to Monthly (1M): {:?}", elapsed_monthly);

    assert!(!monthly.is_empty());
    assert!(
        elapsed_monthly.as_millis() < 25,
        "Monthly resampling of 5,000 bars took {:?}, exceeding 25ms SLA",
        elapsed_monthly
    );
}

#[test]
fn test_sla_arrow_record_batch_conversion_latency() {
    let candles = generate_synthetic_candles(5000);

    // Conversion: Rust domain Vec<AggregatorCandle> -> Arrow RecordBatch (< 30ms)
    let t0 = Instant::now();
    let candle_refs: Vec<&AggregatorCandle> = candles.iter().collect();
    let batch = candles_to_record_batch(&candle_refs).expect("Conversion to record batch failed");
    let elapsed_to_batch = t0.elapsed();

    println!("⏱️  Arrow Serialization (5,000 candles -> RecordBatch): {:?}", elapsed_to_batch);
    assert_eq!(batch.num_rows(), 5000);
    assert!(
        elapsed_to_batch.as_millis() < 30,
        "Conversion to RecordBatch took {:?}, exceeding 30ms SLA",
        elapsed_to_batch
    );

    // Inverted Conversion: Arrow RecordBatch -> Rust domain Vec<AggregatorCandle> (< 60ms in debug on mobile)
    let t1 = Instant::now();
    let parsed_candles = record_batch_to_candles(&batch).expect("Conversion from batch failed");
    let elapsed_from_batch = t1.elapsed();
    println!("⏱️  Arrow Deserialization (RecordBatch -> 5,000 candles): {:?}", elapsed_from_batch);

    assert_eq!(parsed_candles.len(), 5000);
    assert!(
        elapsed_from_batch.as_millis() < 60,
        "Conversion from RecordBatch took {:?}, exceeding 60ms SLA",
        elapsed_from_batch
    );
}

#[tokio::test]
async fn test_sla_lake_write_and_read_roundtrip() {
    let tmp = tempdir().unwrap();
    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let writer = LakeWriter::new(resolver.clone());

    // 1,000 intraday/daily candles in the same month (single partition)
    let base_time = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();
    let mut single_partition_candles = Vec::with_capacity(1000);
    for i in 0..1000 {
        let dt = base_time + chrono::Duration::minutes(i as i64);
        single_partition_candles.push(AggregatorCandle {
            time: dt.timestamp(),
            timestamp_utc: dt,
            canonical_symbol: "ETH/USDT".to_string(),
            timeframe: TimeFrame::M1,
            open: 2500.0,
            high: 2510.0,
            low: 2490.0,
            close: 2505.0,
            volume_base: 10.0,
            volume_quote: 25050.0,
            taker_buy_volume_base: None,
            taker_buy_volume_quote: None,
            source: "binance".to_string(),
            raw_symbol: "ETHUSDT".to_string(),
            ingestion_mode: IngestionMode::LiveWs,
            is_settled: true,
            sequence_id: None,
            vwap: None,
            trade_count: None,
            price_type: "last".to_string(),
            raw_metadata: None,
        });
    }

    // Write SLA: 1,000 candles to single partition Parquet file (< 80ms)
    let t0 = Instant::now();
    writer
        .write_candles(&single_partition_candles, AssetClass::Crypto)
        .expect("Parquet write failed");
    let write_elapsed = t0.elapsed();
    println!("⏱️  Parquet Write (1,000 candles single partition): {:?}", write_elapsed);

    assert!(
        write_elapsed.as_millis() < 80,
        "Single-partition Parquet write took {:?}, exceeding 80ms SLA",
        write_elapsed
    );

    // Read SLA: Read 1,000 candles from disk Parquet file (< 350ms in debug build with Rayon threadpool init)
    let t1 = Instant::now();
    let read_back = LakeReader::read_candles(
        tmp.path(),
        Some("crypto"),
        "ETH/USDT",
        "1m",
        None,
        None,
    ).expect("Parquet read failed");
    let read_elapsed = t1.elapsed();
    println!("⏱️  Parquet Read (1,000 candles from lake): {:?}", read_elapsed);

    assert_eq!(read_back.len(), 1000);
    assert!(
        read_elapsed.as_millis() < 350,
        "Parquet read took {:?}, exceeding 350ms SLA",
        read_elapsed
    );
}

#[tokio::test]
async fn test_sla_rest_endpoint_latencies() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.path().to_path_buf();
    config.base_dir = tmp.path().to_path_buf();

    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let mut registry = InstrumentRegistry::new();

    let mut vendors = HashMap::new();
    vendors.insert("yahoo".to_string(), "BBCA.JK".to_string());
    registry.register(InstrumentMapping {
        canonical_symbol: "BBCA".to_string(),
        name: "Bank Central Asia".to_string(),
        asset_class: AssetClass::Equities,
        instrument_type: InstrumentType::Spot,
        base_currency: "BBCA".to_string(),
        quote_currency: "IDR".to_string(),
        market_timezone: "Asia/Jakarta".to_string(),
        is_24_7: false,
        vendor_symbols: vendors,
        contract_address: None,
        network: None,
        isin: None,
        figi: None,
        perm_id: None,
        aliases: Vec::new(),
    });

    let sub_manager = Arc::new(economy_aggregator::core::subscription::SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));
    let state = Arc::new(AppState::new(config, resolver, registry, sub_manager, orchestrator));

    let app = create_router(state);

    // 1. Health Endpoint SLA (< 5ms)
    let t_health = Instant::now();
    let res_health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed_health = t_health.elapsed();
    println!("⏱️  REST GET /health: {:?}", elapsed_health);

    assert_eq!(res_health.status(), StatusCode::OK);
    assert!(
        elapsed_health.as_millis() < 5,
        "GET /health took {:?}, exceeding 5ms SLA",
        elapsed_health
    );

    // 2. Catalog Endpoint SLA (< 20ms)
    let t_catalog = Instant::now();
    let res_catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/catalog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed_catalog = t_catalog.elapsed();
    println!("⏱️  REST GET /v1/market/catalog: {:?}", elapsed_catalog);

    assert_eq!(res_catalog.status(), StatusCode::OK);
    assert!(
        elapsed_catalog.as_millis() < 20,
        "GET /v1/market/catalog took {:?}, exceeding 20ms SLA",
        elapsed_catalog
    );

    // 3. Metadata Endpoint SLA (< 30ms)
    let t_meta = Instant::now();
    let res_meta = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/metadata?symbol=BBCA")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed_meta = t_meta.elapsed();
    println!("⏱️  REST GET /v1/market/metadata: {:?}", elapsed_meta);

    assert_eq!(res_meta.status(), StatusCode::OK);
    let body = res_meta.into_body().collect().await.unwrap().to_bytes();
    assert!(!body.is_empty());
    assert!(
        elapsed_meta.as_millis() < 30,
        "GET /v1/market/metadata took {:?}, exceeding 30ms SLA",
        elapsed_meta
    );

    // 4. Query Candles Endpoint SLA: Viewport Windowing (300 bars) & Backward Pagination
    let t_candles_cold = Instant::now();
    let res_candles_cold = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/candles?symbol=ETH/USDT&asset_class=crypto&timeframe=1m&limit=300")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed_candles_cold = t_candles_cold.elapsed();
    println!("⏱️  REST GET /v1/market/candles Viewport 300 (Cold): {:?}", elapsed_candles_cold);
    assert_eq!(res_candles_cold.status(), StatusCode::OK);
    assert!(
        elapsed_candles_cold.as_millis() < 50,
        "Cold viewport windowing (300 bars) took {:?}, exceeding 50ms SLA",
        elapsed_candles_cold
    );

    // 5. Query Candles Cached Read SLA (< 5ms)
    let t_candles_cached = Instant::now();
    let res_candles_cached = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/candles?symbol=ETH/USDT&asset_class=crypto&timeframe=1m&limit=300")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed_candles_cached = t_candles_cached.elapsed();
    println!("⏱️  REST GET /v1/market/candles (Cached): {:?}", elapsed_candles_cached);
    assert_eq!(res_candles_cached.status(), StatusCode::OK);
    assert!(
        elapsed_candles_cached.as_millis() < 5,
        "Cached GET /v1/market/candles took {:?}, exceeding 5ms SLA",
        elapsed_candles_cached
    );

    // 6. Backward Historical Pagination with `to` Cutoff SLA (< 50ms)
    let t_backfill = Instant::now();
    let res_backfill = app
        .oneshot(
            Request::builder()
                .uri("/v1/market/candles?symbol=ETH/USDT&asset_class=crypto&timeframe=1m&limit=300&to=1704067200")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed_backfill = t_backfill.elapsed();
    println!("⏱️  REST GET /v1/market/candles Backward `to` (Backfill): {:?}", elapsed_backfill);
    assert_eq!(res_backfill.status(), StatusCode::OK);
    assert!(
        elapsed_backfill.as_millis() < 50,
        "Backward pagination slice took {:?}, exceeding 50ms SLA",
        elapsed_backfill
    );
}
