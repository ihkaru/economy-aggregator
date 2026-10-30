use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::types::{AggregatorCandle, AssetClass, IngestionMode, InstrumentType, TimeFrame};
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentMapping, InstrumentRegistry, LakeWriter, PartitionResolver,
};

fn create_sample_candle(symbol: &str) -> AggregatorCandle {
    AggregatorCandle {
        timestamp_utc: Utc.with_ymd_and_hms(2026, 10, 30, 15, 0, 0).unwrap(),
        canonical_symbol: symbol.to_string(),
        timeframe: TimeFrame::M1,
        open: 50000.0,
        high: 50100.0,
        low: 49950.0,
        close: 50080.0,
        volume_base: 5.5,
        volume_quote: 275440.0,
        taker_buy_volume_base: Some(3.2),
        taker_buy_volume_quote: Some(160256.0),
        source: "binance".to_string(),
        raw_symbol: symbol.replace('/', ""),
        ingestion_mode: IngestionMode::LiveWs,
        is_settled: true,
        sequence_id: Some(1),
        vwap: Some(50050.0),
        trade_count: Some(25),
        price_type: "last".to_string(),
        raw_metadata: None,
    }
}

#[tokio::test]
async fn test_rest_api_health_endpoint() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.path().to_path_buf();
    config.base_dir = tmp.path().to_path_buf();

    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let state = Arc::new(AppState {
        config,
        resolver,
        registry: RwLock::new(InstrumentRegistry::new()),
        start_time: Instant::now(),
    });

    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "economy-aggregator");
}

#[tokio::test]
async fn test_rest_api_instruments_and_candles_query() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.path().to_path_buf();
    config.base_dir = tmp.path().to_path_buf();

    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let mut registry = InstrumentRegistry::new();

    let mut vendors = HashMap::new();
    vendors.insert("binance".to_string(), "ETHUSDT".to_string());
    registry.register(InstrumentMapping {
        canonical_symbol: "ETH/USDT".to_string(),
        name: "Ethereum / Tether".to_string(),
        asset_class: AssetClass::Crypto,
        instrument_type: InstrumentType::Spot,
        base_currency: "ETH".to_string(),
        quote_currency: "USDT".to_string(),
        market_timezone: "UTC".to_string(),
        is_24_7: true,
        vendor_symbols: vendors,
        contract_address: None,
        network: None,
    });

    // Write a real candle into the Parquet lake
    let writer = LakeWriter::new(resolver.clone());
    let candle = create_sample_candle("ETH/USDT");
    writer.write_candles(&[candle], AssetClass::Crypto).unwrap();

    let state = Arc::new(AppState {
        config,
        resolver,
        registry: RwLock::new(registry),
        start_time: Instant::now(),
    });

    let app = create_router(state);

    // 1. Test /v1/instruments
    let res_inst = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/instruments")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_inst.status(), StatusCode::OK);
    let body_inst = res_inst.into_body().collect().await.unwrap().to_bytes();
    let list: Vec<InstrumentMapping> = serde_json::from_slice(&body_inst).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].canonical_symbol, "ETH/USDT");

    // 2. Test /v1/market/candles
    let res_candles = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/candles?asset_class=crypto&symbol=ETH/USDT&timeframe=1m&year=2026&month=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_candles.status(), StatusCode::OK);
    let body_candles = res_candles.into_body().collect().await.unwrap().to_bytes();
    let candles: Vec<AggregatorCandle> = serde_json::from_slice(&body_candles).unwrap();
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].canonical_symbol, "ETH/USDT");
    assert_eq!(candles[0].close, 50080.0);

    // 3. Test /v1/storage/stats
    let res_stats = app
        .oneshot(
            Request::builder()
                .uri("/v1/storage/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_stats.status(), StatusCode::OK);
    let body_stats = res_stats.into_body().collect().await.unwrap().to_bytes();
    let json_stats: Value = serde_json::from_slice(&body_stats).unwrap();
    assert_eq!(json_stats["total_chunks_found"], 1);
    assert!(json_stats["total_bytes"].as_u64().unwrap() > 0);
}
