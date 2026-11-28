//! Milestone 17 Integration & Unit Tests:
//! As-Is vs Adjusted Data Layer Lineage, Split-Guard Transparency, Deterministic Liquidity Diagnostic Tree,
//! Append-Only Arrow/Parquet Diagnostic Ledger, and REST Endpoints.

use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{NaiveDate, TimeZone, Utc};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::diagnostic_types::{EpistemicTier, NodeEvalStatus, SourceLineageEntry};
use economy_aggregator::core::observation_types::{
    DataProcessingLayer, MetricObservation, ObservationFrequency, SourceProvenance, SourceTier,
};
use economy_aggregator::core::series_catalog::SeriesCatalog;
use economy_aggregator::core::subscription::SubscriptionManager;
use economy_aggregator::features::analytics::{
    compute_exact_share_flows, evaluate_liquidity_tree, DiagnosticTreeInputs, DiagnosticTreeMetrics,
};
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::serving::rest::{create_router, AppState};
use economy_aggregator::features::storage::{
    DiagnosticTreeLedger, InstrumentRegistry, PartitionResolver,
};

#[test]
fn test_m17_series_catalog_explicit_as_is_vs_adjusted_layers() {
    let cat = SeriesCatalog::new_canonical();
    let spy_sh = cat.get("etf:SPY:shares_outstanding").unwrap();
    assert_eq!(spy_sh.data_layer, DataProcessingLayer::RawAsIs);
    assert!(spy_sh.raw_value_preserved);

    let dep = cat.get("macro:US:FRED_BANK_DEPOSITS_USD").unwrap();
    assert_eq!(dep.data_layer, DataProcessingLayer::UpstreamSeasonallyAdjusted);
    assert!(dep.adjustment_note.as_deref().unwrap().contains("Seasonally adjusted"));

    let wrm = cat.get("macro:US:FRED_WRMFNS_USD").unwrap();
    assert_eq!(wrm.data_layer, DataProcessingLayer::RawUnitScaled);
    assert!(wrm.adjustment_note.as_deref().unwrap().contains("NSA"));

    let btc_usd = cat.get("etf:BTC_US:implied_flow_usd").unwrap();
    assert_eq!(btc_usd.data_layer, DataProcessingLayer::DerivedAdjusted);

    let bin_fund = cat.get("deriv:BINANCE_BTC:funding_rate_8h").unwrap();
    assert_eq!(bin_fund.data_layer, DataProcessingLayer::RawAsIs);
    let byb_fund = cat.get("deriv:BYBIT_BTC:funding_rate_8h").unwrap();
    assert_eq!(byb_fund.data_layer, DataProcessingLayer::RawAsIs);

    // Verify split_or_glitch_adjusted flag transparency without mutating raw SSGA rows
    let mk = |d: u32, sid: &str, v: f64| MetricObservation {
        observation_ts_utc: Utc.with_ymd_and_hms(2026, 5, d, 23, 59, 59).unwrap(),
        known_at_utc: Utc::now(),
        revision_seq: 0,
        series_id: sid.to_string(),
        domain: "etf_flow".to_string(),
        compartment: Some("deleveraging".to_string()),
        entity_id: "JNK".to_string(),
        metric_name: "test".to_string(),
        frequency: ObservationFrequency::Daily,
        value: v,
        raw_value: Some(v),
        unit: "usd".to_string(),
        provenance: SourceProvenance {
            source: "ssga_xlsx".to_string(),
            source_series_id: "test".to_string(),
            source_url: "https://example.com".to_string(),
            source_tier: SourceTier::OfficialFile,
            source_checksum: Some("abc".to_string()),
            batch_id: "b1".to_string(),
            extra_json: None,
        },
    };
    let aum = vec![mk(1, "etf:JNK:aum_usd", 10.8e9), mk(2, "etf:JNK:aum_usd", 10.8216e9), mk(3, "etf:JNK:aum_usd", 10.9e9)];
    let sh = vec![mk(1, "etf:JNK:shares_outstanding", 300e6), mk(2, "etf:JNK:shares_outstanding", 100.2e6), mk(3, "etf:JNK:shares_outstanding", 100.5e6)];
    let fl = compute_exact_share_flows(&aum, &sh, "JNK", "deleveraging");
    assert_eq!(fl.len(), 2);
    assert!(fl[0].split_or_glitch_adjusted, "1-for-3 split day must be flagged");
    assert!(!fl[1].split_or_glitch_adjusted, "normal day must remain unflagged");
}

#[test]
fn test_m17_deterministic_tree_routing_blind_spot_and_parquet_ledger_roundtrip() {
    let as_of = NaiveDate::from_ymd_opt(2026, 11, 28).unwrap();
    let sample_lineage = vec![SourceLineageEntry {
        series_id: "etf:BIL:shares_outstanding".to_string(),
        source: "ssga_xlsx".to_string(),
        source_series_id: "navhist-BIL:SharesOutstanding".to_string(),
        source_url: "https://www.ssga.com/navhist-us-en-bil.xlsx".to_string(),
        source_checksum: Some("deadbeef".to_string()),
        batch_id: "batch_1".to_string(),
        data_layer: DataProcessingLayer::RawAsIs,
        raw_value_preserved: true,
        adjustment_note: Some("Unadjusted raw SSGA shares".to_string()),
        known_limitation: None,
        latest_observation_utc: Some("2026-11-27T23:59:59Z".to_string()),
        stale_lag_days: Some(1),
        is_stale: false,
        used_by_nodes: vec!["Q2_DEFENSIVE_CASH_PARKING".to_string()],
    }];

    // Case 1: Risk-off + primary outflow + cash tanks flat + FINRA stale -> BLIND_SPOT + STALE_LAG
    let blind_metrics = DiagnosticTreeMetrics {
        spy_return_pct: Some(-4.5),
        btc_return_pct: Some(-8.0),
        spy_exact_flow_usd: Some(-3.0e9),
        bil_exact_flow_usd: Some(0.05e9),
        delta_wrmfns_pct: Some(0.01),
        finra_is_stale: true,
        ..Default::default()
    };
    let r1 = evaluate_liquidity_tree(DiagnosticTreeInputs {
        as_of_date: as_of,
        window_weeks: 4,
        metrics: blind_metrics.clone(),
        source_lineage: sample_lineage.clone(),
    });
    let r1_repeat = evaluate_liquidity_tree(DiagnosticTreeInputs {
        as_of_date: as_of,
        window_weeks: 4,
        metrics: blind_metrics,
        source_lineage: sample_lineage.clone(),
    });
    assert_eq!(r1.terminal_leaf_id, "LEAF_BLIND_SPOT_UNOBSERVED_DESTINATION");
    assert_eq!(r1.terminal_epistemic_tier, EpistemicTier::BlindSpot);
    assert_eq!(r1.tree_checksum_sha256, r1_repeat.tree_checksum_sha256, "Determinism violated");
    assert!(r1.blind_spots.len() >= 2, "Must emit both unobserved destination and stale FINRA lag");
    assert!(r1.nodes.iter().any(|n| n.node_id == "Q3_CREDIT_AND_MACRO_CONTRACTION" && n.status == NodeEvalStatus::StaleLag));

    // Case 2: Drawdown + primary outflow + BIL & WRMFNS inflows -> neutral LEAF_PRIMARY_OUTFLOW_WITH_CASH_INFLOWS
    let rot_rep = evaluate_liquidity_tree(DiagnosticTreeInputs {
        as_of_date: as_of,
        window_weeks: 4,
        metrics: DiagnosticTreeMetrics {
            spy_return_pct: Some(-3.8),
            spy_exact_flow_usd: Some(-2.5e9),
            bil_exact_flow_usd: Some(1.8e9),
            dtb3_latest_pct: Some(4.16),
            sofr_latest_pct: Some(3.95),
            delta_wrmfns_usd: Some(22.0e9),
            delta_wrmfns_pct: Some(0.85),
            h8_covers_latest_market_date: true,
            ..Default::default()
        },
        source_lineage: sample_lineage.clone(),
    });
    assert_eq!(rot_rep.protocol_version, "liquidity-tree-v1.1.0");
    assert_eq!(rot_rep.terminal_leaf_id, "LEAF_PRIMARY_OUTFLOW_WITH_CASH_INFLOWS");
    assert_eq!(rot_rep.terminal_epistemic_tier, EpistemicTier::Tier2Replicated);
    let q2 = rot_rep.nodes.iter().find(|n| n.node_id == "Q2_DEFENSIVE_CASH_PARKING").unwrap();
    assert!(q2.metrics.get("dash_for_cash_gld_out_and_bil_in").is_none(), "pseudo-OOS flag must be removed in v1.1.0");
    assert_eq!(q2.metrics["dtb3_latest_pct"], 4.16);

    // Case 3 (v1.1.0 Price-Neutral OI Guard): nominal USD OI drops -8B with BTC -22%,
    // but contract OI RISES +12% and SPY has primary inflows while BTC ETF outflows ->
    // MUST NOT falsely label deleveraging or Tier2Replicated, and MUST emit H.8 lag blind spot!
    let div_rep = evaluate_liquidity_tree(DiagnosticTreeInputs {
        as_of_date: as_of,
        window_weeks: 4,
        metrics: DiagnosticTreeMetrics {
            spy_return_pct: Some(-3.6),
            btc_return_pct: Some(-22.6),
            spy_exact_flow_usd: Some(7.88e9),
            btc_etf_flow_usd: Some(-5.68e9),
            gld_exact_flow_usd: Some(-1.52e9),
            bil_exact_flow_usd: Some(3.28e9),
            delta_btc_oi_usd: Some(-7.81e9),
            delta_btc_oi_contracts: Some(11_570.0),
            delta_btc_oi_contracts_pct: Some(12.0),
            btc_funding_rate_8h_avg_bps: Some(0.75),
            h8_covers_latest_market_date: false,
            finra_is_stale: true,
            ..Default::default()
        },
        source_lineage: sample_lineage,
    });
    assert_eq!(div_rep.terminal_leaf_id, "LEAF_CROSS_COMPARTMENT_DIVERGENCE_CRYPTO_OUT_EQUITY_IN");
    assert_eq!(div_rep.terminal_epistemic_tier, EpistemicTier::Tier1Accounting);
    assert!(div_rep.blind_spots.iter().any(|b| b.missing_data_gap.contains("H.8")), "must disclose H.8 lag blind spot");

    // Persistent Append-Only Parquet Ledger round-trip (isolated under lake/diagnostics/tree_runs/)
    let dir = TempDir::new().unwrap();
    let ledger = DiagnosticTreeLedger::new(&dir.path().join("lake"));
    let p1 = ledger.persist_run(&r1).unwrap();
    let p2 = ledger.persist_run(&rot_rep).unwrap();
    assert!(p1.exists() && p2.exists());
    assert!(p1.to_string_lossy().contains("diagnostics/tree_runs/year=2026"));

    let history = ledger.list_runs(10).unwrap();
    assert_eq!(history.len(), 2);
    let fetched = ledger.get_run(&r1.run_id).unwrap().expect("run_id must be retrievable");
    assert_eq!(fetched.tree_checksum_sha256, r1.tree_checksum_sha256);
    assert_eq!(fetched.source_lineage.len(), 1);
    assert_eq!(fetched.source_lineage[0].source_checksum.as_deref(), Some("deadbeef"));
}

#[tokio::test]
async fn test_m17_rest_diagnostic_tree_persist_history_and_backtrack_routes() {
    let dir = TempDir::new().unwrap();
    let cfg = LakehouseConfig {
        base_dir: dir.path().to_path_buf(),
        lake_dir: dir.path().join("lake"),
        hot_buffer_dir: dir.path().join("hot_buffer"),
        micro_batch_flush_seconds: 5,
        micro_batch_max_rows: 1000,
        row_group_size: 1024,
    };
    cfg.ensure_directories().unwrap();
    let resolver = PartitionResolver::new(cfg.lake_dir.clone());
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
        .oneshot(
            Request::builder()
                .uri("/v1/analytics/liquidity-tree?as_of=2026-11-28&window_weeks=4&persist=true&include_spillover=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let run_id = body["diagnostic_tree"]["run_id"].as_str().unwrap().to_string();
    assert!(body["diagnostic_tree"]["persisted_path"].as_str().is_some());

    let hist_res = app
        .clone()
        .oneshot(Request::builder().uri("/v1/analytics/liquidity-tree/history?limit=10").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(hist_res.status(), StatusCode::OK);
    let hist_body: serde_json::Value =
        serde_json::from_slice(&hist_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(hist_body["count"], 1);

    let run_res = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/analytics/liquidity-tree/runs/{}", run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_res.status(), StatusCode::OK);
    let run_body: serde_json::Value =
        serde_json::from_slice(&run_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(run_body["diagnostic_tree"]["run_id"], run_id);
}
