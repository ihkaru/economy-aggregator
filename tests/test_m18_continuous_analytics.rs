//! Milestone 18 Integration & Unit Tests:
//! Regime Transition Event Diffing, Prospective OOS Anti-Peeking Scorecard (`v1.1.0`),
//! Atomic Checkpoint & Non-Destructive Parquet Bin-Packing Compaction,
//! and ContinuousAnalyticsSupervisor Idempotency & REST API Routes.

use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::continuous_types::TransitionKind;
use economy_aggregator::core::diagnostic_types::SourceLineageEntry;
use economy_aggregator::core::observation_types::DataProcessingLayer;
use economy_aggregator::core::subscription::SubscriptionManager;
use economy_aggregator::features::analytics::{
    detect_regime_transition, evaluate_liquidity_tree, evaluate_prospective_scorecard,
    DiagnosticTreeInputs, DiagnosticTreeMetrics, ProspectiveScorecardInputs, ProspectiveWeeklyObs,
    PROSPECTIVE_PROTOCOL_SHA256,
};
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::serving::rest::{create_router, AppState};
use economy_aggregator::features::storage::{
    ContinuousAnalyticsStore, DiagnosticTreeLedger, InstrumentRegistry, PartitionResolver,
};

fn sample_lineage(finra_stale: bool) -> Vec<SourceLineageEntry> {
    vec![SourceLineageEntry {
        series_id: "credit:US:FINRA_MARGIN_DEBT_USD".to_string(),
        source: "finra_xlsx".to_string(),
        source_series_id: "margin-statistics".to_string(),
        source_url: "https://www.finra.org/margin.xlsx".to_string(),
        source_checksum: Some("sha123".to_string()),
        batch_id: "batch_m18".to_string(),
        data_layer: DataProcessingLayer::RawUnitScaled,
        raw_value_preserved: true,
        adjustment_note: Some("Scaled from USD millions".to_string()),
        known_limitation: Some("Monthly ~21-day lag".to_string()),
        latest_observation_utc: Some("2026-10-31T23:59:59Z".to_string()),
        stale_lag_days: Some(if finra_stale { 28 } else { 5 }),
        is_stale: finra_stale,
        used_by_nodes: vec!["Q3_CREDIT_AND_MACRO_CONTRACTION".to_string()],
    }]
}

#[test]
fn test_m18_regime_transition_diff_and_prospective_oos_anti_peeking() {
    let d1 = NaiveDate::from_ymd_opt(2026, 11, 28).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 12, 5).unwrap();

    // Baseline Risk-Off + Stale FINRA
    let r1 = evaluate_liquidity_tree(DiagnosticTreeInputs {
        as_of_date: d1,
        window_weeks: 4,
        metrics: DiagnosticTreeMetrics {
            spy_return_pct: Some(-3.5),
            spy_exact_flow_usd: Some(-2.5e9),
            bil_exact_flow_usd: Some(0.02e9),
            finra_is_stale: true,
            ..Default::default()
        },
        source_lineage: sample_lineage(true),
    });
    let init_ev = detect_regime_transition(None, &r1).expect("initial baseline event");
    assert_eq!(init_ev.transition_kind, TransitionKind::InitialBaseline);

    // Identical repeat must return None (zero duplicate events)
    assert!(detect_regime_transition(Some(&r1), &r1).is_none());

    // Shift to Risk-On + FINRA freshened -> LeafChanged + RootRegimeChanged + BlindSpotResolved + StaleSourceFreshened
    let r2 = evaluate_liquidity_tree(DiagnosticTreeInputs {
        as_of_date: d2,
        window_weeks: 4,
        metrics: DiagnosticTreeMetrics {
            spy_return_pct: Some(4.2),
            spy_exact_flow_usd: Some(3.1e9),
            bil_exact_flow_usd: Some(0.8e9),
            finra_is_stale: false,
            ..Default::default()
        },
        source_lineage: sample_lineage(false),
    });
    let shift = detect_regime_transition(Some(&r1), &r2).expect("regime shift event");
    assert_eq!(shift.transition_kind, TransitionKind::LeafChanged);
    assert!(shift.secondary_kinds.contains(&TransitionKind::RootRegimeChanged));
    assert!(shift.secondary_kinds.contains(&TransitionKind::BlindSpotResolved));
    assert!(shift.secondary_kinds.contains(&TransitionKind::StaleSourceFreshened));
    assert_eq!(shift.newly_freshened_sources, vec!["credit:US:FINRA_MARGIN_DEBT_USD"]);
    assert!(!shift.metric_shifts.is_empty());

    // Prospective OOS Scorecard: pre-OOS burn-in complete on 2026-11-28, then accumulating OOS >= 2026-12-04
    let mut rows = Vec::new();
    let start = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    for i in 0..25 {
        let wk = start + chrono::Duration::days(i * 7);
        rows.push(ProspectiveWeeklyObs {
            week_friday: wk.format("%Y-%m-%d").to_string(),
            spy_weekly_return_pct: Some(if i % 5 == 0 { -2.5 } else { 0.8 }),
            btc_weekly_return_pct: Some(if i % 6 == 0 { -6.0 } else { 1.5 }),
            btc_etf_flow_usd: Some(if i % 6 == 0 { -1.2e9 } else { 0.4e9 }),
            delta_btc_oi_usd: Some(if i % 6 == 0 { -2.0e9 } else { 0.5e9 }),
            bil_exact_flow_usd: Some((i as f64) * 50e6),
            jnk_exact_flow_usd: Some(-100e6),
            gld_exact_flow_usd: Some(-((i as f64) * 20e6)),
        });
    }
    let pre_rep = evaluate_prospective_scorecard(&ProspectiveScorecardInputs {
        as_of_date: d1,
        weekly_rows: rows.clone(),
        daily_move_bil_oos_days: 0,
        daily_move_bil_running_corr: None,
        latest_ex_rrp_tci_z: Some(7.15),
        latest_ex_rrp_bias_adj_pct: Some(9.14),
    });
    assert_eq!(pre_rep.protocol_sha256, PROSPECTIVE_PROTOCOL_SHA256);
    assert_eq!(pre_rep.oos_weeks_collected, 0);
    assert_eq!(pre_rep.overall_readout_status, "PRE_OOS_BURN_IN_COMPLETE");
    assert!(pre_rep.optional_stopping_forbidden);

    // Add 2 OOS weeks (>= 2026-12-04) — must enforce anti-peeking ACCUMULATING_OOS_NO_PEEKING
    rows.push(ProspectiveWeeklyObs {
        week_friday: "2026-12-04".to_string(),
        spy_weekly_return_pct: Some(-3.1),
        btc_weekly_return_pct: Some(-7.2),
        btc_etf_flow_usd: Some(-1.5e9),
        delta_btc_oi_usd: Some(-2.4e9),
        bil_exact_flow_usd: Some(3.0e9),
        jnk_exact_flow_usd: Some(-0.8e9),
        gld_exact_flow_usd: Some(-1.2e9),
    });
    rows.push(ProspectiveWeeklyObs {
        week_friday: "2026-12-11".to_string(),
        spy_weekly_return_pct: Some(1.4),
        btc_weekly_return_pct: Some(2.1),
        btc_etf_flow_usd: Some(0.6e9),
        delta_btc_oi_usd: Some(0.9e9),
        bil_exact_flow_usd: Some(0.1e9),
        jnk_exact_flow_usd: Some(0.3e9),
        gld_exact_flow_usd: Some(0.4e9),
    });
    let oos_rep = evaluate_prospective_scorecard(&ProspectiveScorecardInputs {
        as_of_date: NaiveDate::from_ymd_opt(2026, 12, 12).unwrap(),
        weekly_rows: rows,
        daily_move_bil_oos_days: 10,
        daily_move_bil_running_corr: Some(0.31),
        latest_ex_rrp_tci_z: Some(6.8),
        latest_ex_rrp_bias_adj_pct: Some(8.9),
    });
    assert_eq!(oos_rep.oos_weeks_collected, 2);
    assert_eq!(oos_rep.primary_weeks_remaining, 102);
    assert_eq!(oos_rep.overall_readout_status, "ACCUMULATING_OOS_NO_PEEKING");
    let conf1 = &oos_rep.primary_rules_m2[0];
    assert_eq!(conf1.trigger_count, 1);
    assert!((conf1.running_metric_value.unwrap() - (-2.1)).abs() < 1e-6);
    let sec5 = &oos_rep.secondary_rules_m5[4];
    assert_eq!(sec5.trigger_count, 1, "strictly expanding Q80/Q20 must flag 2026-12-04 alert");
    assert_eq!(sec5.hit_count, Some(1));
}

#[test]
fn test_m18_event_sourced_parquet_checkpoint_and_binpack_compaction() {
    let dir = TempDir::new().unwrap();
    let lake = dir.path().join("lake");
    let tree_ledger = DiagnosticTreeLedger::new(&lake);
    let store = ContinuousAnalyticsStore::new(&lake);

    let mut prev = None;
    let mut ids = Vec::new();
    for i in 0..6 {
        let d = NaiveDate::from_ymd_opt(2026, 11, 20 + i as u32).unwrap();
        let rep = evaluate_liquidity_tree(DiagnosticTreeInputs {
            as_of_date: d,
            window_weeks: 4,
            metrics: DiagnosticTreeMetrics {
                spy_return_pct: Some(if i % 2 == 0 { -3.0 } else { 2.5 }),
                spy_exact_flow_usd: Some((i as f64 - 2.0) * 1e9),
                ..Default::default()
            },
            source_lineage: sample_lineage(i % 2 == 0),
        });
        tree_ledger.persist_run(&rep).unwrap();
        if let Some(ev) = detect_regime_transition(prev.as_ref(), &rep) {
            store.persist_transition(&ev).unwrap();
        }
        ids.push(rep.run_id.clone());
        prev = Some(rep);
    }

    // Compact 6 fragments into 1 compacted ZSTD(3) file — zero historical runs lost!
    let compacted_runs = tree_ledger.compact_year_partitions(4).unwrap();
    assert_eq!(compacted_runs, 6);
    let history = tree_ledger.list_runs(20).unwrap();
    assert_eq!(history.len(), 6);
    for id in &ids {
        assert!(tree_ledger.get_run(id).unwrap().is_some(), "run {} lost during compaction", id);
    }

    let compacted_tr = store.compact_transition_partitions(4).unwrap();
    assert_eq!(compacted_tr, 6);
    let transitions = store.list_transitions(Some(4), 20).unwrap();
    assert_eq!(transitions.len(), 6);

    // Atomic checkpoint save/load roundtrip
    let mut cp = store.load_checkpoint();
    cp.total_cycles_executed = 7;
    cp.last_data_watermark_hash = Some("wm_abc".to_string());
    store.save_checkpoint(&cp).unwrap();
    let loaded = store.load_checkpoint();
    assert_eq!(loaded.total_cycles_executed, 7);
    assert_eq!(loaded.last_data_watermark_hash.as_deref(), Some("wm_abc"));
}

#[tokio::test]
async fn test_m18_continuous_supervisor_idempotency_and_rest_routes() {
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

    // Pass 1: initial baseline persisted (1w + 4w + prospective scorecard)
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analytics/continuous/run?as_of=2026-11-28&include_spillover=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let b1: serde_json::Value =
        serde_json::from_slice(&res1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(b1["outcome"]["skipped_unchanged_watermark"], false);
    assert_eq!(b1["outcome"]["persisted_run_ids"].as_array().unwrap().len(), 2);
    assert_eq!(b1["outcome"]["emitted_transition_ids"].as_array().unwrap().len(), 2);
    assert_eq!(b1["outcome"]["prospective_oos_persisted"], true);

    // Pass 2: unchanged lake watermark & same date -> idempotent skip (ZERO duplicate writes!)
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analytics/continuous/run?as_of=2026-11-28&include_spillover=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let b2: serde_json::Value =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(b2["outcome"]["skipped_unchanged_watermark"], true);
    assert_eq!(b2["outcome"]["persisted_run_ids"].as_array().unwrap().len(), 0);

    // Verify status, transitions, and prospective-scorecard GET endpoints
    let st = app
        .clone()
        .oneshot(Request::builder().uri("/v1/analytics/continuous/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(st.status(), StatusCode::OK);
    let stb: serde_json::Value =
        serde_json::from_slice(&st.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(stb["continuous_analytics"]["checkpoint"]["total_runs_persisted"], 2);

    let tr = app
        .clone()
        .oneshot(Request::builder().uri("/v1/analytics/liquidity-tree/transitions?limit=10").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(tr.status(), StatusCode::OK);
    let trb: serde_json::Value =
        serde_json::from_slice(&tr.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(trb["count"], 2);

    let sc = app
        .oneshot(
            Request::builder()
                .uri("/v1/analytics/prospective-scorecard?as_of=2026-11-28&include_spillover=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sc.status(), StatusCode::OK);
    let scb: serde_json::Value =
        serde_json::from_slice(&sc.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(scb["prospective_scorecard"]["protocol_sha256"], PROSPECTIVE_PROTOCOL_SHA256);
    assert_eq!(scb["prospective_scorecard"]["overall_readout_status"], "PRE_OOS_BURN_IN_COMPLETE");
}
