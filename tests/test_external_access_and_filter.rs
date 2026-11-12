//! Integration Test Suite for Milestone 12: External System Access, Search & High-Precision Filter Parity.

use std::collections::HashMap;
use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::macro_types::{MetricUnitType, RichEconomicEvent, SourceAttribution};
use economy_aggregator::core::prediction_types::PredictionMarketCandle;
use economy_aggregator::core::subscription::SubscriptionManager;
use economy_aggregator::core::types::{AggregatorCandle, AssetClass, IngestionMode, InstrumentType, TimeFrame};
use economy_aggregator::features::ingestion::economic_calendar::EconomicCalendarFetcher;
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::ingestion::prediction_market::persist_prediction_candles_to_lake;
use economy_aggregator::features::serving::types::InstrumentCatalogItem;
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentMapping, InstrumentRegistry, LakeWriter, PartitionResolver,
};

fn create_candle(symbol: &str, time: i64, close: f64) -> AggregatorCandle {
    let dt = DateTime::from_timestamp(time, 0).unwrap().with_timezone(&Utc);
    AggregatorCandle {
        time,
        timestamp_utc: dt,
        canonical_symbol: symbol.to_string(),
        timeframe: TimeFrame::D1,
        open: close * 0.99,
        high: close * 1.01,
        low: close * 0.98,
        close,
        volume_base: 100.0,
        volume_quote: close * 100.0,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        source: "test".to_string(),
        raw_symbol: symbol.to_string(),
        ingestion_mode: IngestionMode::RestBatch,
        is_settled: true,
        sequence_id: None,
        vwap: Some(close),
        trade_count: Some(10),
        price_type: "last".to_string(),
        raw_metadata: None,
    }
}

#[tokio::test]
async fn test_filtered_market_catalog_and_caching() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.path().to_path_buf();
    config.base_dir = tmp.path().to_path_buf();

    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let mut registry = InstrumentRegistry::new();

    let mut vendors = HashMap::new();
    vendors.insert("binance".to_string(), "BTCUSDT".to_string());
    registry.register(InstrumentMapping {
        canonical_symbol: "BTC/USDT".to_string(),
        name: "Bitcoin / Tether".to_string(),
        asset_class: AssetClass::Crypto,
        instrument_type: InstrumentType::Spot,
        base_currency: "BTC".to_string(),
        quote_currency: "USDT".to_string(),
        market_timezone: "UTC".to_string(),
        is_24_7: true,
        vendor_symbols: vendors,
        contract_address: None,
        network: None,
        isin: None,
        figi: None,
        perm_id: None,
        aliases: Vec::new(),
    });

    let mut vendors_eth = HashMap::new();
    vendors_eth.insert("binance".to_string(), "ETHUSDT".to_string());
    registry.register(InstrumentMapping {
        canonical_symbol: "ETH/USDT".to_string(),
        name: "Ethereum / Tether".to_string(),
        asset_class: AssetClass::Crypto,
        instrument_type: InstrumentType::Spot,
        base_currency: "ETH".to_string(),
        quote_currency: "USDT".to_string(),
        market_timezone: "UTC".to_string(),
        is_24_7: true,
        vendor_symbols: vendors_eth,
        contract_address: None,
        network: None,
        isin: None,
        figi: None,
        perm_id: None,
        aliases: Vec::new(),
    });

    let sub_manager = Arc::new(SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));
    let state = Arc::new(AppState::new(config, resolver, registry, sub_manager, orchestrator));
    let app = create_router(state);

    // 1. Search keyword "BTC"
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/catalog?q=BTC")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let items: Vec<InstrumentCatalogItem> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].symbol, "BTC/USDT");

    // 2. Filter by asset_class
    let res_ac = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/catalog?asset_class=crypto")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_ac.status(), StatusCode::OK);
    let bytes_ac = res_ac.into_body().collect().await.unwrap().to_bytes();
    let items_ac: Vec<InstrumentCatalogItem> = serde_json::from_slice(&bytes_ac).unwrap();
    assert_eq!(items_ac.len(), 2);
}

#[tokio::test]
async fn test_market_candles_from_to_range_filtering() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.path().to_path_buf();
    config.base_dir = tmp.path().to_path_buf();

    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let writer = LakeWriter::new(resolver.clone());

    // Write 3 consecutive daily candles: Nov 8 (1794096000), Nov 9 (1794182400), Nov 10 (1794268800)
    let c1 = create_candle("BTC_USDT", 1794096000, 80000.0);
    let c2 = create_candle("BTC_USDT", 1794182400, 81000.0);
    let c3 = create_candle("BTC_USDT", 1794268800, 82000.0);
    writer.write_candles(&[c1, c2, c3], AssetClass::Crypto).unwrap();

    let sub_manager = Arc::new(SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));
    let state = Arc::new(AppState::new(config, resolver, InstrumentRegistry::new(), sub_manager, orchestrator));
    let app = create_router(state);

    // Query with from=1794100000 and to=1794200000 -> should only match c2 (Nov 9)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/candles?asset_class=crypto&symbol=BTC_USDT&timeframe=1d&from=1794100000&to=1794200000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let candles: Vec<AggregatorCandle> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].time, 1794182400);
}

#[tokio::test]
async fn test_macro_calendar_lake_archival_reading() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.path().to_path_buf();
    config.base_dir = tmp.path().to_path_buf();

    let resolver = PartitionResolver::new(tmp.path().to_path_buf());

    // Persist a sample macro event in the lake
    let dt = Utc.with_ymd_and_hms(2026, 11, 10, 13, 30, 0).unwrap();
    let event = RichEconomicEvent {
        event_id: "macro:US:cpi_m_m:20261110".to_string(),
        scheduled_utc: dt,
        country: "US".to_string(),
        currency: "USD".to_string(),
        category: "inflation".to_string(),
        indicator_code: "cpi_m_m".to_string(),
        title: "Consumer Price Index m/m".to_string(),
        impact_tier: 3,
        is_all_day: false,
        is_tentative: false,
        raw_actual: Some("0.4%".to_string()),
        raw_forecast: Some("0.3%".to_string()),
        raw_previous: Some("0.2%".to_string()),
        normalized_actual: Some(0.004),
        normalized_forecast: Some(0.003),
        normalized_previous: Some(0.002),
        normalized_revised_previous: None,
        unit_type: MetricUnitType::Percentage,
        unit_multiplier: 0.01,
        unit_symbol: "%".to_string(),
        better_than_expected: Some(false),
        attribution: SourceAttribution {
            provider: "biquote".to_string(),
            original_id: "cpi:123".to_string(),
            source_url: Some("https://bls.gov".to_string()),
            agency_name: Some("BLS".to_string()),
            agency_url: Some("https://bls.gov".to_string()),
            observed_at_utc: dt,
        },
        revision_count: 0,
        revision_history_json: None,
    };
    EconomicCalendarFetcher::persist_to_lake(&[event], &resolver).unwrap();

    let sub_manager = Arc::new(SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));
    let state = Arc::new(AppState::new(config, resolver, InstrumentRegistry::new(), sub_manager, orchestrator));
    let app = create_router(state);

    // Query macro calendar directly from lakehouse
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/macro/calendar?source=lake&country=US&indicator=cpi&year=2026&month=11")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let events: Vec<RichEconomicEvent> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].indicator_code, "cpi_m_m");
}

#[tokio::test]
async fn test_prediction_candles_lake_reading() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.path().to_path_buf();
    config.base_dir = tmp.path().to_path_buf();

    let resolver = PartitionResolver::new(tmp.path().to_path_buf());

    let dt = Utc.with_ymd_and_hms(2026, 11, 10, 10, 0, 0).unwrap();
    let candle = PredictionMarketCandle {
        time: dt.timestamp(),
        timestamp_utc: dt,
        market_id: "KXCPI-NOV".to_string(),
        outcome: "YES".to_string(),
        timeframe: "1h".to_string(),
        prob_open: Some(0.60),
        prob_high: Some(0.65),
        prob_low: Some(0.58),
        prob_close: Some(0.63),
        book_bid_close: Some(0.62),
        book_ask_close: Some(0.64),
        volume_contracts: Some(1000.0),
        volume_usd: Some(630.0),
        open_interest_usd: Some(12000.0),
        trade_count: Some(50),
        price_source: economy_aggregator::core::prediction_types::ProbabilityPriceSource::Trade,
        venue: "kalshi".to_string(),
    };
    persist_prediction_candles_to_lake(&[candle], &resolver).unwrap();

    let sub_manager = Arc::new(SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));
    let state = Arc::new(AppState::new(config, resolver, InstrumentRegistry::new(), sub_manager, orchestrator));
    let app = create_router(state);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/prediction-markets/markets/kalshi:KXCPI-NOV/candles?timeframe=1h")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.is_array());
    assert_eq!(json[0]["market_id"], "KXCPI-NOV");
    assert_eq!(json[0]["prob_close"], 0.63);
}
