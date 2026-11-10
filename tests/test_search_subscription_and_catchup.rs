//! Verification gauntlet for Discovery Search, Subscription Manager, High-Watermark Scanner, and Gap Catch-Up.

use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use tempfile::tempdir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::subscription::{SubscriptionItem, SubscriptionManager};
use economy_aggregator::core::types::{AggregatorCandle, AssetClass, IngestionMode, TimeFrame};
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::ingestion::watermark::WatermarkScanner;
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{InstrumentRegistry, LakeWriter, PartitionResolver};

#[tokio::test]
async fn test_subscription_manager_lifecycle() {
    let tmp = tempdir().unwrap();
    let config_path = tmp.path().join("subscriptions.json");
    let manager = SubscriptionManager::new(config_path.clone());

    // 1. Initial load generates defaults
    let items = manager.load().unwrap();
    assert!(!items.is_empty(), "Defaults should be generated");
    assert!(items.iter().any(|i| i.canonical == "BBCA"));

    // 2. Add new subscription (BBRI)
    let bbri = SubscriptionItem {
        canonical: "BBRI".to_string(),
        raw_symbol: "BBRI.JK".to_string(),
        asset_class: "equities".to_string(),
        source: "yahoo".to_string(),
        timeframes: vec!["1d".to_string()],
        enabled: true,
        poll_interval_secs: Some(300),
        created_at_utc: Utc::now().to_rfc3339(),
    };
    let replaced = manager.subscribe(bbri.clone()).unwrap();
    assert!(!replaced, "First time adding should not replace");

    // 3. Upsert existing subscription (idempotent update)
    let mut bbri_updated = bbri.clone();
    bbri_updated.poll_interval_secs = Some(60);
    let replaced_again = manager.subscribe(bbri_updated).unwrap();
    assert!(replaced_again, "Second time adding should replace");

    // 4. Verify listed items
    let active = manager.list_active().unwrap();
    assert!(active.iter().any(|i| i.canonical == "BBRI"));

    // 5. Test multi-source co-existence: subscribe to BTC/USDT from both binance and yahoo
    let btc_binance = SubscriptionItem {
        canonical: "BTC/USDT".to_string(),
        raw_symbol: "BTCUSDT".to_string(),
        asset_class: "crypto".to_string(),
        source: "binance".to_string(),
        timeframes: vec!["1d".to_string()],
        enabled: true,
        poll_interval_secs: None,
        created_at_utc: Utc::now().to_rfc3339(),
    };
    let btc_yahoo = SubscriptionItem {
        canonical: "BTC/USDT".to_string(),
        raw_symbol: "BTC-USD".to_string(),
        asset_class: "crypto".to_string(),
        source: "yahoo".to_string(),
        timeframes: vec!["1d".to_string()],
        enabled: true,
        poll_interval_secs: Some(300),
        created_at_utc: Utc::now().to_rfc3339(),
    };
    manager.subscribe(btc_binance).unwrap();
    manager.subscribe(btc_yahoo).unwrap();
    let subs = manager.load().unwrap();
    let btc_subs: Vec<_> = subs.iter().filter(|s| s.canonical == "BTC/USDT").collect();
    assert_eq!(btc_subs.len(), 2, "Both Binance and Yahoo feeds for BTC/USDT should co-exist");

    // 6. Targeted Unsubscribe by source: remove only binance feed
    let removed_binance = manager.unsubscribe(Some("binance"), "BTC/USDT").unwrap();
    assert!(removed_binance);
    let subs_after_one = manager.load().unwrap();
    assert!(subs_after_one.iter().any(|s| s.canonical == "BTC/USDT" && s.source == "yahoo"));
    assert!(!subs_after_one.iter().any(|s| s.canonical == "BTC/USDT" && s.source == "binance"));

    // 7. Unsubscribe without source (removes all BBRI subscriptions)
    let removed = manager.unsubscribe(None, "BBRI").unwrap();
    assert!(removed);
    let active_after = manager.list_active().unwrap();
    assert!(!active_after.iter().any(|i| i.canonical == "BBRI"));
}

#[tokio::test]
async fn test_watermark_scanner_with_lakehouse_candles() {
    let tmp = tempdir().unwrap();
    let lake_dir = tmp.path().join("lake");
    std::fs::create_dir_all(&lake_dir).unwrap();

    let resolver = PartitionResolver::new(lake_dir.clone());
    let writer = LakeWriter::new(resolver);

    // 1. Before writing candles: gap should be flagged
    let pre_status = WatermarkScanner::scan_watermark(&lake_dir, "crypto", "BTC_USDT", "1d");
    assert!(pre_status.has_gap);
    assert_eq!(pre_status.total_rows, 0);

    // 2. Write a real candle dated 2026-11-08
    let candle = AggregatorCandle {
        time: 1794182400,
        timestamp_utc: DateTime::parse_from_rfc3339("2026-11-08T00:00:00Z").unwrap().with_timezone(&Utc),
        canonical_symbol: "BTC/USDT".to_string(),
        timeframe: TimeFrame::D1,
        open: 80000.0,
        high: 81000.0,
        low: 79500.0,
        close: 80500.0,
        volume_base: 1500.0,
        volume_quote: 120750000.0,
        source: "binance".to_string(),
        raw_symbol: "BTCUSDT".to_string(),
        ingestion_mode: IngestionMode::RestBatch,
        is_settled: true,
        sequence_id: None,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        vwap: None,
        trade_count: None,
        price_type: "last".to_string(),
        raw_metadata: None,
    };
    writer.write_candles(&[candle], AssetClass::Crypto).unwrap();

    // 3. Scan watermark again: should detect latest date and rows
    let post_status = WatermarkScanner::scan_watermark(&lake_dir, "crypto", "BTC_USDT", "1d");
    assert_eq!(post_status.total_rows, 1);
    assert_eq!(post_status.latest_date, Some(chrono::NaiveDate::from_ymd_opt(2026, 11, 8).unwrap()));
}

#[tokio::test]
async fn test_rest_api_subscription_and_orchestrator_routes() {
    let tmp = tempdir().unwrap();
    let mut config = LakehouseConfig::default();
    config.base_dir = tmp.path().to_path_buf();
    config.lake_dir = tmp.path().join("lake");
    std::fs::create_dir_all(&config.lake_dir).unwrap();

    let resolver = PartitionResolver::new(config.lake_dir.clone());
    let registry = InstrumentRegistry::new();
    let sub_manager = Arc::new(SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));

    let state = Arc::new(AppState::new(config, resolver, registry, sub_manager, orchestrator));
    let app = create_router(state);

    // 1. GET /v1/subscriptions
    let res = app.clone().oneshot(
        Request::builder().uri("/v1/subscriptions").body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. GET /v1/ingestion/status
    let res_status = app.clone().oneshot(
        Request::builder().uri("/v1/ingestion/status").body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(res_status.status(), StatusCode::OK);

    // 3. POST /v1/subscriptions
    let new_sub = serde_json::json!({
        "canonical": "TEST_ASSET",
        "raw_symbol": "TEST.JK",
        "asset_class": "equities",
        "source": "yahoo",
        "timeframes": ["1d"],
        "enabled": true,
        "created_at_utc": Utc::now().to_rfc3339()
    });
    let res_add = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/v1/subscriptions")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&new_sub).unwrap()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res_add.status(), StatusCode::OK);

    // 4. DELETE /v1/subscriptions/TEST_ASSET
    let res_del = app.clone().oneshot(
        Request::builder()
            .method("DELETE")
            .uri("/v1/subscriptions/TEST_ASSET")
            .body(Body::empty())
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res_del.status(), StatusCode::OK);
}
