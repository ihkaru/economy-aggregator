//! Milestone 20 Integration & Unit Tests:
//! 6-Tank Liquidity & Capital Flow Atlas (`GET /v1/analytics/liquidity-atlas`),
//! Zero-Lookahead Historical Date-Shifting (`as_of`), Honest Unprinted-Lag Warnings,
//! and Range Preset Resolution.

use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::observation_types::{
    MetricObservation, ObservationFrequency, SourceProvenance, SourceTier,
};
use economy_aggregator::core::subscription::SubscriptionManager;
use economy_aggregator::core::types::{AggregatorCandle, AssetClass, IngestionMode, TimeFrame};
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::serving::rest::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentRegistry, LakeWriter, ObservationWriter, PartitionResolver,
};
use economy_aggregator::runtime::atlas_loader::resolve_range_start;

fn mk_candle(sym: &str, src: &str, d: NaiveDate, close: f64) -> AggregatorCandle {
    let ts = Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap());
    AggregatorCandle {
        time: ts.timestamp(),
        timestamp_utc: ts,
        canonical_symbol: sym.to_string(),
        timeframe: TimeFrame::D1,
        open: close * 0.995,
        high: close * 1.005,
        low: close * 0.99,
        close,
        volume_base: 1_000_000.0,
        volume_quote: close * 1_000_000.0,
        source: src.to_string(),
        raw_symbol: sym.to_string(),
        ingestion_mode: IngestionMode::RestBatch,
        is_settled: true,
        sequence_id: None,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        vwap: None,
        trade_count: None,
        price_type: "last".to_string(),
        raw_metadata: None,
    }
}

fn mk_obs(
    sid: &str,
    domain: &str,
    entity_id: &str,
    src: &str,
    unit: &str,
    d: NaiveDate,
    val: f64,
) -> MetricObservation {
    let ts = Utc.from_utc_datetime(&d.and_hms_opt(12, 0, 0).unwrap());
    MetricObservation {
        observation_ts_utc: ts,
        known_at_utc: ts,
        revision_seq: 0,
        series_id: sid.to_string(),
        domain: domain.to_string(),
        compartment: None,
        entity_id: entity_id.to_string(),
        metric_name: sid.to_string(),
        frequency: ObservationFrequency::Daily,
        value: val,
        raw_value: Some(val),
        unit: unit.to_string(),
        provenance: SourceProvenance {
            source: src.to_string(),
            source_series_id: sid.to_string(),
            source_url: format!("https://example.test/{}", sid),
            source_tier: SourceTier::OfficialApi,
            source_checksum: Some("sha256_test".to_string()),
            batch_id: "batch_m20_test".to_string(),
            extra_json: None,
        },
    }
}

#[test]
fn test_m20_resolve_range_start_presets() {
    let as_of = NaiveDate::from_ymd_opt(2026, 11, 28).unwrap();
    assert_eq!(resolve_range_start(as_of, "12w").0, "12w");
    assert_eq!(resolve_range_start(as_of, "6m").0, "6m");
    assert_eq!(resolve_range_start(as_of, "1y").0, "1y");
    assert_eq!(resolve_range_start(as_of, "3y").0, "3y");
    assert_eq!(resolve_range_start(as_of, "5y").0, "5y");
    assert_eq!(
        resolve_range_start(as_of, "all"),
        ("all".to_string(), NaiveDate::from_ymd_opt(2016, 1, 1).unwrap())
    );
}

#[tokio::test]
async fn test_m20_atlas_rest_zero_lookahead_and_staleness() {
    let tmp = TempDir::new().unwrap();
    let lake_dir = tmp.path().join("lake");
    let resolver = PartitionResolver::new(lake_dir.clone());
    let lake_writer = LakeWriter::new(resolver.clone());
    let obs_writer = ObservationWriter::new(resolver.clone());

    // Populate daily SPY/BTC/GLD candles from 2026-09-01 to 2026-11-28 (89 days)
    let start = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 11, 28).unwrap();
    let mut spy_cs = Vec::new();
    let mut btc_cs = Vec::new();
    let mut gld_cs = Vec::new();
    let mut obs_rows = Vec::new();

    let mut cur = start;
    let mut i = 0usize;
    while cur <= end {
        let px = 600.0 + (i as f64) * 0.5;
        spy_cs.push(mk_candle("SPY", "yahoo", cur, px));
        btc_cs.push(mk_candle("BTCUSDT", "binance", cur, 90_000.0 + (i as f64) * 100.0));
        gld_cs.push(mk_candle("GLD", "yahoo", cur, 260.0 + (i as f64) * 0.1));

        // SSGA SPY shares & AUM (sharp creation spike AFTER 2026-10-30 to catch any lookahead)
        let shares = if cur <= NaiveDate::from_ymd_opt(2026, 10, 30).unwrap() {
            900_000_000.0 + (i as f64) * 100_000.0
        } else {
            950_000_000.0 + (i as f64) * 500_000.0
        };
        obs_rows.push(mk_obs("etf:SPY:shares_outstanding", "etf_flow", "SPY", "ssga_xlsx", "shares", cur, shares));
        obs_rows.push(mk_obs("etf:SPY:aum_usd", "etf_flow", "SPY", "ssga_xlsx", "usd", cur, shares * px));
        obs_rows.push(mk_obs("deriv:BINANCE_BTC:funding_rate_8h", "derivative", "BTC", "binance_futures", "ratio", cur, 0.0001));

        // H.8 Bank Deposits stop at 2026-11-18 (10d lag behind 2026-11-28 -> triggers UNPRINTED LAG)
        if cur <= NaiveDate::from_ymd_opt(2026, 11, 18).unwrap() && cur.weekday() == chrono::Weekday::Wed {
            obs_rows.push(mk_obs("macro:US:FRED_BANK_DEPOSITS_USD", "banking", "US", "fred_api", "usd", cur, 18.2e12));
        }

        cur += Duration::days(1);
        i += 1;
    }

    lake_writer.write_candles(&spy_cs, AssetClass::Equities).unwrap();
    lake_writer.write_candles(&btc_cs, AssetClass::Crypto).unwrap();
    lake_writer.write_candles(&gld_cs, AssetClass::Commodities).unwrap();
    obs_writer.write_observations(&obs_rows).unwrap();

    let cfg = LakehouseConfig {
        base_dir: tmp.path().to_path_buf(),
        lake_dir: lake_dir.clone(),
        hot_buffer_dir: tmp.path().join("hot_buffer"),
        ..Default::default()
    };
    cfg.ensure_directories().unwrap();
    let sub_mgr = Arc::new(SubscriptionManager::new(tmp.path().join("subs.json")));
    let orch = Arc::new(IngestionOrchestrator::new(cfg.clone(), sub_mgr.clone()));
    let state = Arc::new(AppState::new(cfg, resolver, InstrumentRegistry::default(), sub_mgr, orch));
    let app = create_router(state);

    // 1. Historical date shift to 2026-10-30 -> MUST have ZERO lookahead
    let resp = app
        .clone()
        .oneshot(Request::builder().uri("/v1/analytics/liquidity-atlas?as_of=2026-10-30&range=6m&window_weeks=4").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["success"], true);
    let atlas = &body["atlas"];
    assert_eq!(atlas["as_of_date"], "2026-10-30");
    assert_eq!(atlas["earliest_available_date"], "2026-09-01");
    assert_eq!(atlas["latest_available_date"], "2026-11-28");
    assert_eq!(atlas["diagnostic_tree"]["as_of_date"], "2026-10-30");

    let tanks = atlas["tanks"].as_array().unwrap();
    assert_eq!(tanks.len(), 6);
    let ids: Vec<&str> = tanks.iter().map(|t| t["tank_id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        vec!["equities_spy", "crypto_btc", "gold_gld", "cash_short_term", "credit_banks", "macro_fed_pipes"]
    );

    for tank in tanks {
        for p in tank["primary_series"]["points"].as_array().unwrap() {
            let t = p["time"].as_str().unwrap();
            assert!(t <= "2026-10-30", "Lookahead in primary {}: {}", tank["tank_id"], t);
        }
        for ov in tank["overlay_series"].as_array().unwrap() {
            for p in ov["points"].as_array().unwrap() {
                let t = p["time"].as_str().unwrap();
                assert!(t <= "2026-10-30", "Lookahead in overlay {}: {}", ov["series_id"], t);
            }
        }
    }

    // 2. Latest snapshot 2026-11-28 -> Tank 5 (credit_banks) MUST expose UNPRINTED LAG for H.8 (2026-11-18)
    let resp2 = app
        .clone()
        .oneshot(Request::builder().uri("/v1/analytics/liquidity-atlas?as_of=2026-11-28&range=1y&window_weeks=4").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let body2: serde_json::Value = serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let credit_tank = body2["atlas"]["tanks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tank_id"] == "credit_banks")
        .unwrap();
    assert_eq!(credit_tank["primary_series"]["latest_date"], "2026-11-18");
    assert_eq!(credit_tank["primary_series"]["stale_lag_days"], 10);
    let warn = credit_tank["staleness_warning"].as_str().unwrap();
    assert!(warn.contains("UNPRINTED LAG"), "Expected honest UNPRINTED LAG warning, got: {}", warn);

    // 3. Invalid as_of format returns HTTP 400
    let resp3 = app
        .oneshot(Request::builder().uri("/v1/analytics/liquidity-atlas?as_of=not-a-date").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
}
