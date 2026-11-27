//! Integration tests for Milestone 14: Universal Bitemporal Parquet Storage,
//! Mandatory Provenance, Dynamic Matrix Pivot, and Stage-1 Liquidity Flows.

use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, TimeZone, Utc};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::observation_types::{
    MetricObservation, ObservationFrequency, SourceProvenance, SourceTier,
};
use economy_aggregator::core::series_catalog::SeriesCatalog;
use economy_aggregator::core::subscription::SubscriptionManager;
use economy_aggregator::core::types::{AggregatorCandle, IngestionMode, TimeFrame};
use economy_aggregator::features::analytics::{
    compute_fed_net_liquidity, compute_implied_net_flows, net_flows_to_observations,
};
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentRegistry, LakeCompactor, ObservationFilter, ObservationReader, ObservationWriter,
    PartitionResolver,
};

fn test_config(base: &std::path::Path) -> LakehouseConfig {
    LakehouseConfig {
        base_dir: base.to_path_buf(),
        lake_dir: base.join("lake"),
        hot_buffer_dir: base.join("hot_buffer"),
        micro_batch_flush_seconds: 5,
        micro_batch_max_rows: 1000,
        row_group_size: 1024,
    }
}

fn sample_obs(
    series_id: &str,
    domain: &str,
    compartment: &str,
    obs_ts: chrono::DateTime<Utc>,
    known_at: chrono::DateTime<Utc>,
    rev: u32,
    val: f64,
    source: &str,
) -> MetricObservation {
    MetricObservation {
        observation_ts_utc: obs_ts,
        known_at_utc: known_at,
        revision_seq: rev,
        series_id: series_id.to_string(),
        domain: domain.to_string(),
        compartment: Some(compartment.to_string()),
        entity_id: "US".to_string(),
        metric_name: "test_metric".to_string(),
        frequency: ObservationFrequency::Daily,
        value: val,
        raw_value: Some(val),
        unit: "usd".to_string(),
        provenance: SourceProvenance {
            source: source.to_string(),
            source_series_id: format!("src:{}", series_id),
            source_url: "https://example.org/data".to_string(),
            source_tier: SourceTier::OfficialApi,
            source_checksum: Some("abc123".to_string()),
            batch_id: "test_batch".to_string(),
            extra_json: None,
        },
    }
}

#[test]
fn test_zero_anonymous_data_rejected_and_bitemporal_pit_resolution() {
    let dir = TempDir::new().unwrap();
    let resolver = PartitionResolver::new(dir.path().to_path_buf());
    let writer = ObservationWriter::new(resolver.clone());
    let reader = ObservationReader::new(resolver);

    let t_obs = Utc.with_ymd_and_hms(2026, 10, 1, 23, 59, 59).unwrap();
    let k1 = Utc.with_ymd_and_hms(2026, 10, 2, 12, 0, 0).unwrap();
    let k2 = Utc.with_ymd_and_hms(2026, 10, 15, 12, 0, 0).unwrap();

    // Missing source must fail validation cleanly
    let mut bad = sample_obs("macro:US:WALCL", "macro_liquidity", "macro_control", t_obs, k1, 0, 100.0, "");
    bad.provenance.source = "   ".to_string();
    assert!(writer.write_observations(&[bad]).is_err());

    let v0 = sample_obs("macro:US:WALCL", "macro_liquidity", "macro_control", t_obs, k1, 0, 7_000_000.0, "dbnomics_fed");
    let v1 = sample_obs("macro:US:WALCL", "macro_liquidity", "macro_control", t_obs, k2, 1, 7_050_000.0, "dbnomics_fed");
    // Adding a second source on the same series_id/timestamp must remain strictly isolated (never overwritten!)
    let v_other = sample_obs("macro:US:WALCL", "macro_liquidity", "macro_control", t_obs, k2, 0, 7_100_000.0, "other_src");
    writer.write_observations(&[v0, v1, v_other]).unwrap();

    // As-of Oct 10 (pinned to dbnomics_fed) must return ONLY revision 0 (zero look-ahead bias!)
    let pit_early = reader.read_observations(&ObservationFilter {
        series_ids: vec!["macro:US:WALCL".to_string()],
        source: Some("dbnomics_fed".to_string()),
        as_of_known_at: Some(k1 + Duration::days(3)),
        ..Default::default()
    });
    assert_eq!(pit_early.len(), 1);
    assert_eq!(pit_early[0].value, 7_000_000.0);

    // As-of Oct 20 (pinned to dbnomics_fed) must return revision 1
    let pit_late = reader.read_observations(&ObservationFilter {
        series_ids: vec!["macro:US:WALCL".to_string()],
        source: Some("dbnomics_fed".to_string()),
        as_of_known_at: Some(k2 + Duration::days(1)),
        ..Default::default()
    });
    assert_eq!(pit_late.len(), 1);
    assert_eq!(pit_late[0].value, 7_050_000.0);

    // Querying without ?source= preserves both isolated sources without folding/overwriting
    let unpinned = reader.read_observations(&ObservationFilter {
        series_ids: vec!["macro:US:WALCL".to_string()],
        resolve_latest_revision: true,
        ..Default::default()
    });
    assert_eq!(unpinned.len(), 2);

    // Compactor coexistence check: running global LakeCompactor must compact cleanly without panicking on observations schema
    assert!(LakeCompactor::default().compact_all(dir.path(), 1).is_ok());
}

#[test]
fn test_stage1_implied_net_flow_and_fed_net_liquidity_math() {
    let d1 = Utc.with_ymd_and_hms(2026, 11, 10, 23, 59, 59).unwrap();
    let d2 = Utc.with_ymd_and_hms(2026, 11, 11, 23, 59, 59).unwrap();

    // Suppose AUM grows from 100B to 115B while price jumps +10% -> revaluation = +10B, implied net flow = +5B
    let aum = vec![
        sample_obs("etf:SPY:aum_usd", "etf_flow", "equities", d1, d1, 0, 100.0e9, "ssga_xlsx"),
        sample_obs("etf:SPY:aum_usd", "etf_flow", "equities", d2, d2, 0, 115.0e9, "ssga_xlsx"),
    ];
    let mk_candle = |ts: chrono::DateTime<Utc>, close| AggregatorCandle {
        time: ts.timestamp(),
        timestamp_utc: ts,
        canonical_symbol: "SPY".to_string(),
        timeframe: TimeFrame::D1,
        open: close,
        high: close,
        low: close,
        close,
        volume_base: 1.0,
        volume_quote: close,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        source: "yahoo".to_string(),
        raw_symbol: "SPY".to_string(),
        ingestion_mode: IngestionMode::RestBatch,
        is_settled: true,
        sequence_id: None,
        vwap: None,
        trade_count: None,
        price_type: "last".to_string(),
        raw_metadata: None,
    };
    let flows = compute_implied_net_flows(&aum, &[mk_candle(d1, 500.0), mk_candle(d2, 550.0)], "SPY", "equities");
    assert_eq!(flows.len(), 1);
    assert!((flows[0].price_return - 0.10).abs() < 1e-9);
    assert!((flows[0].implied_net_flow_usd - 5.0e9).abs() < 1.0);
    assert_eq!(flows[0].aum_source, "ssga_xlsx");
    assert_eq!(flows[0].price_source, "yahoo");

    let derived_obs = net_flows_to_observations(&flows, "test");
    assert_eq!(derived_obs.len(), 1);
    assert!(derived_obs[0].validate().is_ok());

    // Fed Net Liquidity = WALCL (7T) - TGA (800B) - RRP (200B) = 6T
    let fed = vec![sample_obs("macro:US:FED_TOTAL_ASSETS_USD", "macro_liquidity", "macro_control", d2, d2, 0, 7.0e12, "fed_h41_html")];
    let tga = vec![sample_obs("macro:US:TGA_BALANCE_USD", "macro_liquidity", "macro_control", d1, d1, 0, 8.0e11, "treasury_fiscaldata")];
    let rrp = vec![sample_obs("macro:US:ON_RRP_USD", "macro_liquidity", "fiat_park", d2, d2, 0, 2.0e11, "nyfed_api")];
    let nl = compute_fed_net_liquidity(&fed, &tga, &rrp);
    assert_eq!(nl.len(), 1);
    assert!((nl[0].net_liquidity_usd - 6.0e12).abs() < 1.0);
}

#[tokio::test]
async fn test_rest_observations_catalog_matrix_and_panel_routes() {
    let dir = TempDir::new().unwrap();
    let cfg = test_config(dir.path());
    cfg.ensure_directories().unwrap();
    let resolver = PartitionResolver::new(cfg.lake_dir.clone());
    let writer = ObservationWriter::new(resolver.clone());

    let d1 = Utc.with_ymd_and_hms(2026, 11, 20, 23, 59, 59).unwrap();
    writer
        .write_observations(&[
            sample_obs("stablecoin:ALL:mcap_usd", "stablecoin", "crypto_park", d1, d1, 0, 200.0e9, "defillama"),
            sample_obs("macro:US:ON_RRP_USD", "macro_liquidity", "fiat_park", d1, d1, 0, 150.0e9, "nyfed_api"),
        ])
        .unwrap();

    let sub_mgr = Arc::new(SubscriptionManager::new(cfg.base_dir.join("config/subscriptions.json")));
    let orch = Arc::new(IngestionOrchestrator::new(cfg.clone(), sub_mgr.clone()));
    let app = create_router(Arc::new(AppState::new(
        cfg,
        resolver,
        InstrumentRegistry::default(),
        sub_mgr,
        orch,
    )));

    let res = app
        .clone()
        .oneshot(Request::builder().uri("/v1/observations/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cat_body: serde_json::Value = serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(cat_body["defined_series"].as_array().unwrap().len(), SeriesCatalog::new_canonical().list().len());
    assert_eq!(cat_body["ingested_coverage"].as_array().unwrap().len(), 2);

    let m_res = app
        .clone()
        .oneshot(Request::builder().uri("/v1/observations/matrix?resample=1w").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(m_res.status(), StatusCode::OK);
    let m_body: serde_json::Value = serde_json::from_slice(&m_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(m_body["columns"].as_array().unwrap().len(), 2);
    assert!(m_body["sources_by_series"]["stablecoin:ALL:mcap_usd"].as_array().unwrap().iter().any(|v| v == "defillama"));

    let csv_res = app
        .clone()
        .oneshot(Request::builder().uri("/v1/observations/matrix?resample=1w&format=csv").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(csv_res.status(), StatusCode::OK);
    let csv_txt = String::from_utf8(csv_res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(csv_txt.starts_with("date,"));

    // Cross-source mixing guard: writing two different sources under the same series_id must trigger 409 Conflict unless ?source= is pinned!
    writer
        .write_observations(&[
            sample_obs("macro:US:TEST_MIX", "macro_liquidity", "macro_control", d1, d1, 0, 10.0, "src_a"),
            sample_obs("macro:US:TEST_MIX", "macro_liquidity", "macro_control", d1, d1, 0, 25.0, "src_b"),
        ])
        .unwrap();
    let conflict_res = app
        .clone()
        .oneshot(Request::builder().uri("/v1/observations/matrix?series_ids=macro:US:TEST_MIX").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(conflict_res.status(), StatusCode::CONFLICT);
    let pinned_res = app
        .oneshot(Request::builder().uri("/v1/observations/matrix?series_ids=macro:US:TEST_MIX&source=src_a").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(pinned_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_m15_occ_quoted_csv_treasury_pagination_and_fred_no_key_leak() {
    use axum::routing::get;
    use axum::Router;
    use economy_aggregator::features::ingestion::flows::OccOiFetcher;
    use economy_aggregator::features::ingestion::liquidity::{
        FredFetcher, TreasuryFetcher, CANONICAL_FRED_SPECS,
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let app = Router::new()
        .route(
            "/daily-open-interest",
            get(|| async {
                concat!(
                    "Daily Open Interest - November 2026\r\n",
                    "Date,Equity,,,Index/Other,,,Debt,,,Futures,OCC Total\r\n",
                    ",Calls,Puts,Total,Calls,Puts,Total,Calls,Puts,Total,Total,\r\n",
                    "11/25/2026,\"376,236,882\",\"273,081,881\",\"649,318,763\",\"21,458,742\",\"23,061,918\",\"44,520,660\",\"0\",\"0\",\"0\",\"386,388\",\"694,225,811\",\r\n"
                )
            }),
        )
        .route(
            "/services/api/fiscal_service/v1/accounting/dts/operating_cash_balance",
            get(|| async {
                r#"{"data":[{"record_date":"2016-01-04","account_type":"Treasury General Account (Closing Balance)","close_today_bal":"333123"}],"meta":{"total-pages":1}}"#
            }),
        )
        .route(
            "/fred/series/observations",
            get(|| async {
                r#"{"observations":[{"date":"2026-11-18","realtime_start":"2026-11-19","value":"6741452"},{"date":"2026-11-11","realtime_start":"2026-11-12","value":"."}]}"#
            }),
        );

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let base = format!("http://{}", addr);

    let occ = OccOiFetcher::new(base.clone());
    let occ_rows = occ.fetch_month_for_date("11/25/2026", "b_occ").await.unwrap();
    assert_eq!(occ_rows.len(), 1);
    assert_eq!(occ_rows[0].value, 694_225_811.0);
    assert!(occ_rows[0]
        .provenance
        .extra_json
        .as_deref()
        .unwrap()
        .contains("649318763"));

    let tr = TreasuryFetcher::new(base.clone(), base.clone());
    let tga_rows = tr.fetch_tga_balance(10_000, "b_tga").await.unwrap();
    assert_eq!(tga_rows.len(), 1);
    assert_eq!(tga_rows[0].value, 333_123.0 * 1_000_000.0);

    let fred = FredFetcher::new(base, "super_secret_key_should_never_persist".to_string());
    let walcl_spec = CANONICAL_FRED_SPECS.iter().find(|s| s.fred_id == "WALCL").unwrap();
    let fred_rows = fred
        .fetch_series(walcl_spec, "2016-01-01", "b_fred")
        .await
        .unwrap();
    assert_eq!(fred_rows.len(), 1);
    assert_eq!(fred_rows[0].value, 6_741_452.0 * 1_000_000.0);
    assert_eq!(fred_rows[0].provenance.source, "fred_api");
    assert!(!fred_rows[0]
        .provenance
        .source_url
        .contains("super_secret_key_should_never_persist"));
}
