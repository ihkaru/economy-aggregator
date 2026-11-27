//! Integration tests for server API key auth middleware, 3-tier CLI config,
//! and end-to-end Agent-First Remote CLI execution against a live Axum server.

use std::collections::HashMap;
use std::sync::Arc;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::{DateTime, Utc};
use tempfile::tempdir;
use tower::ServiceExt;

use economy_aggregator::core::cli_config::{
    extract_global_options, handle_config, mask_secret, normalize_base_url, resolve_client_config,
};
use economy_aggregator::core::cli_remote::{dispatch_remote, RemoteClient};
use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::subscription::SubscriptionManager;
use economy_aggregator::core::types::{
    AggregatorCandle, AssetClass, IngestionMode, InstrumentType, TimeFrame,
};
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::serving::auth::constant_time_eq;
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentMapping, InstrumentRegistry, LakeWriter, PartitionResolver,
};

fn build_test_state(tmp: &std::path::Path, api_keys: Vec<String>) -> Arc<AppState> {
    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.join("lake");
    config.hot_buffer_dir = tmp.join("hot_buffer");
    config.base_dir = tmp.to_path_buf();
    config.ensure_directories().unwrap();

    let resolver = PartitionResolver::new(config.lake_dir.clone());
    let writer = LakeWriter::new(resolver.clone());

    let dt = DateTime::from_timestamp(1794182400, 0).unwrap().with_timezone(&Utc);
    let candle = AggregatorCandle {
        time: dt.timestamp(),
        timestamp_utc: dt,
        canonical_symbol: "BTC_USDT".to_string(),
        timeframe: TimeFrame::D1,
        open: 80000.0,
        high: 82000.0,
        low: 79500.0,
        close: 81500.0,
        volume_base: 120.0,
        volume_quote: 9_780_000.0,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        source: "binance".to_string(),
        raw_symbol: "BTCUSDT".to_string(),
        ingestion_mode: IngestionMode::RestBatch,
        is_settled: true,
        sequence_id: None,
        vwap: Some(81000.0),
        trade_count: Some(500),
        price_type: "last".to_string(),
        raw_metadata: None,
    };
    writer.write_candles(&[candle], AssetClass::Crypto).unwrap();

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

    let sub_manager = Arc::new(SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));

    Arc::new(
        AppState::new(config, resolver, registry, sub_manager, orchestrator)
            .with_api_keys(api_keys),
    )
}

#[test]
fn test_constant_time_eq_and_secret_masking() {
    assert!(constant_time_eq(b"ea_secret_12345", b"ea_secret_12345"));
    assert!(!constant_time_eq(b"ea_secret_12345", b"ea_secret_12346"));
    assert!(!constant_time_eq(b"short", b"longer"));

    let masked = mask_secret("ea_live_9988776655");
    assert_eq!(masked, "ea_l***55");
    assert!(!masked.contains("99887766"));
    assert_eq!(mask_secret("short"), "***");

    assert_eq!(
        normalize_base_url("https://api.example.com///").unwrap(),
        "https://api.example.com"
    );
    assert!(normalize_base_url("ftp://invalid.local").is_err());
}

#[test]
fn test_cli_config_set_show_clear_and_precedence() {
    let tmp = tempdir().unwrap();
    let cfg_path = tmp.path().join("config.json");

    let args_set: Vec<String> = vec![
        "--base-url".into(),
        "https://market.dvlpid.my.id/".into(),
        "--api-key".into(),
        "ea_super_secret_token_99".into(),
    ];
    let global_set = extract_global_options(&args_set);
    let out_set = handle_config(&["set".into()], &global_set, Some(cfg_path.clone())).unwrap();
    assert_eq!(out_set["success"], true);
    assert_eq!(out_set["data"]["base_url"], "https://market.dvlpid.my.id");
    assert_eq!(out_set["data"]["api_key_masked"], "ea_s***99");
    assert!(!out_set.to_string().contains("ea_super_secret_token_99"));

    // Show reads persisted file and masks key
    let empty_global = extract_global_options(&[]);
    let out_show = handle_config(&["show".into()], &empty_global, Some(cfg_path.clone())).unwrap();
    assert_eq!(out_show["data"]["base_url"], "https://market.dvlpid.my.id");
    assert_eq!(out_show["data"]["base_url_source"], "file");
    assert_eq!(out_show["data"]["api_key_source"], "file");
    assert!(!out_show.to_string().contains("ea_super_secret_token_99"));

    // CLI flag overrides persisted file; --local forces local mode
    let override_args: Vec<String> = vec![
        "--base-url=http://127.0.0.1:9999".into(),
        "--local".into(),
    ];
    let override_global = extract_global_options(&override_args);
    let resolved = resolve_client_config(&override_global, Some(cfg_path.clone())).unwrap();
    assert_eq!(resolved.base_url.as_deref(), Some("http://127.0.0.1:9999"));
    assert_eq!(resolved.base_url_source, "flag");
    assert_eq!(resolved.effective_base_url(), None);

    // Clear removes persisted config
    let out_clear = handle_config(&["clear".into()], &empty_global, Some(cfg_path.clone())).unwrap();
    assert_eq!(out_clear["success"], true);
    assert!(out_clear["data"]["base_url"].is_null());
    assert!(out_clear["data"]["api_key_masked"].is_null());
}

#[tokio::test]
async fn test_api_key_middleware_enforcement_and_health_exemption() {
    let tmp = tempdir().unwrap();
    let state = build_test_state(
        tmp.path(),
        vec!["primary-key-001".to_string(), "rotated-key-002".to_string()],
    );
    let app = create_router(state);

    // 1. /health and /v1/health remain open without key
    for health_path in ["/health", "/v1/health"] {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(health_path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // 2. Protected route rejected without key
    let res_no_key = app
        .clone()
        .oneshot(Request::builder().uri("/v1/market/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res_no_key.status(), StatusCode::UNAUTHORIZED);

    // 3. Protected route rejected with wrong key
    let res_bad_key = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/catalog")
                .header(header::AUTHORIZATION, "Bearer wrong-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad_key.status(), StatusCode::UNAUTHORIZED);

    // 4. Protected route succeeds with Bearer token & carries OWASP security headers
    let res_bearer = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/catalog?q=BTC")
                .header(header::AUTHORIZATION, "Bearer primary-key-001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bearer.status(), StatusCode::OK);
    assert_eq!(
        res_bearer.headers().get(header::X_CONTENT_TYPE_OPTIONS).unwrap(),
        "nosniff"
    );
    assert_eq!(
        res_bearer.headers().get(header::X_FRAME_OPTIONS).unwrap(),
        "DENY"
    );
    assert_eq!(
        res_bearer.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    // 4b. Path traversal symbol rejected with 400 Bad Request
    let res_traversal = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/candles?asset_class=crypto&symbol=../../etc/passwd&timeframe=1d")
                .header(header::AUTHORIZATION, "Bearer primary-key-001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_traversal.status(), StatusCode::BAD_REQUEST);

    // 5. Protected route succeeds with rotated key via X-API-Key
    let res_x_key = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/catalog?q=BTC")
                .header("x-api-key", "rotated-key-002")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_x_key.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_remote_cli_end_to_end_against_authenticated_server() {
    let tmp = tempdir().unwrap();
    let secret = "ea_live_test_key_2026".to_string();
    let state = build_test_state(tmp.path(), vec![secret.clone()]);
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // Unauthorized client gets exit_code 1 and status 401
    let bad_client = RemoteClient::new(base_url.clone(), Some("wrong-secret".into())).unwrap();
    let err = dispatch_remote("catalog", &[], &bad_client)
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(err.exit_code, 1);
    assert_eq!(err.status_code, Some(401));

    // Authorized client executes full agent workflow
    let client = RemoteClient::new(base_url.clone(), Some(secret)).unwrap();

    let health = dispatch_remote("health", &[], &client).await.unwrap().unwrap();
    assert_eq!(health["success"], true);
    assert_eq!(health["data"]["status"], "ok");

    let catalog = dispatch_remote("catalog", &["--q".into(), "BTC".into()], &client)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(catalog["success"], true);
    assert_eq!(catalog["data"].as_array().unwrap().len(), 1);

    let candles = dispatch_remote(
        "candles",
        &[
            "--symbol".into(),
            "BTC_USDT".into(),
            "--asset".into(),
            "crypto".into(),
            "--tf".into(),
            "1d".into(),
        ],
        &client,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(candles["success"], true);
    assert_eq!(candles["data"][0]["close"], 81500.0);

    let sub = dispatch_remote(
        "subscribe",
        &[
            "--source".into(),
            "yahoo".into(),
            "--symbol".into(),
            "BBCA.JK".into(),
            "--asset".into(),
            "equities".into(),
        ],
        &client,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(sub["success"], true);

    let subs = dispatch_remote("subscriptions", &[], &client)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(subs["success"], true);

    let unsub = dispatch_remote(
        "unsubscribe",
        &["--symbol".into(), "BBCA.JK".into(), "--source".into(), "yahoo".into()],
        &client,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(unsub["success"], true);

    // Filtered synchronous catch-up via `sync` alias
    let sync_res = dispatch_remote(
        "sync",
        &[
            "--symbol".into(),
            "NONEXISTENT_TICKER".into(),
            "--source".into(),
            "yahoo".into(),
            "--force".into(),
        ],
        &client,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(sync_res["success"], true);
    assert_eq!(sync_res["data"]["status"], "completed");
    assert_eq!(sync_res["data"]["mode"], "sync");
    assert_eq!(sync_res["data"]["filter"]["force"], true);
    assert_eq!(sync_res["data"]["summaries"].as_array().unwrap().len(), 0);

    // Asynchronous fire-and-forget catch-up trigger (HTTP 202 Accepted)
    let async_res = dispatch_remote(
        "catch-up",
        &[
            "--symbol".into(),
            "NONEXISTENT_TICKER".into(),
            "--async".into(),
        ],
        &client,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(async_res["success"], true);
    assert_eq!(async_res["data"]["status"], "accepted");
    assert_eq!(async_res["data"]["mode"], "async");

    // Remote compaction via POST /v1/storage/compact
    let compact_res = dispatch_remote("compact", &["--min-files".into(), "1".into()], &client)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(compact_res["success"], true);
    assert_eq!(compact_res["data"]["action"], "compact");
    assert!(compact_res["data"]["files_merged"].as_u64().unwrap() >= 1);

    server_handle.abort();
}
