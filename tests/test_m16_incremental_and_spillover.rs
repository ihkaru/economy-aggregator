//! Integration & unit test suite for Milestone 16 (Phase 333-344):
//! Per-subscription cadence, incremental observation writes, BTC ETF USD normalization,
//! weekly flow panel, OLS VAR(p) Diebold-Yilmaz spillover, and REST caching/flags.

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
use economy_aggregator::core::subscription::{SubscriptionItem, SubscriptionManager};
use economy_aggregator::core::types::{AggregatorCandle, IngestionMode, TimeFrame};
use economy_aggregator::features::analytics::{
    build_weekly_flow_panel, compute_btc_etf_usd_flows, compute_diebold_yilmaz_spillover,
    WeeklyPanelInputs,
};
use economy_aggregator::features::ingestion::liquidity_sync::is_series_fresh;
use economy_aggregator::features::ingestion::orchestrator::{
    is_subscription_due, IngestionOrchestrator,
};
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentRegistry, ObservationReader, ObservationWriter, PartitionResolver,
};

fn mk_obs(
    series_id: &str,
    domain: &str,
    comp: &str,
    freq: ObservationFrequency,
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
        compartment: Some(comp.to_string()),
        entity_id: "US".to_string(),
        metric_name: "metric".to_string(),
        frequency: freq,
        value: val,
        raw_value: Some(val),
        unit: "usd".to_string(),
        provenance: SourceProvenance {
            source: source.to_string(),
            source_series_id: series_id.to_string(),
            source_url: "https://example.org/m16".to_string(),
            source_tier: SourceTier::OfficialApi,
            source_checksum: Some("chk".to_string()),
            batch_id: "b_m16".to_string(),
            extra_json: None,
        },
    }
}

#[test]
fn test_m16_cadence_scheduler_and_incremental_dedup_write() {
    let now = Utc.with_ymd_and_hms(2026, 11, 28, 12, 0, 0).unwrap();
    let sub = SubscriptionItem {
        source: "liquidity_macro".to_string(),
        symbol: "US_MACRO_LIQUIDITY".to_string(),
        asset_class: "observations".to_string(),
        timeframes: vec!["1d".to_string()],
        enabled: true,
        poll_interval_secs: Some(21_600),
        created_at_utc: now.to_rfc3339(),
    };
    assert!(is_subscription_due(&sub, None, now));
    assert!(!is_subscription_due(&sub, Some(now - Duration::minutes(15)), now));
    assert!(is_subscription_due(&sub, Some(now - Duration::hours(6)), now));

    // Incremental write skips unchanged rows (0 new files/rows) but persists genuine revisions.
    let dir = TempDir::new().unwrap();
    let resolver = PartitionResolver::new(dir.path().to_path_buf());
    let writer = ObservationWriter::new(resolver.clone());
    let reader = ObservationReader::new(resolver);

    let d1 = Utc.with_ymd_and_hms(2026, 11, 27, 23, 59, 59).unwrap();
    let row = mk_obs("etf:QQQ:stockanalysis_shares_outstanding", "etf_snapshot", "equities", ObservationFrequency::Daily, d1, now, 0, 390_000_000.0, "stockanalysis");
    assert_eq!(writer.write_incremental(&[row.clone()]).unwrap(), 1);
    assert_eq!(writer.write_incremental(&[row.clone()]).unwrap(), 0);

    let mut revised = row;
    revised.value = 391_500_000.0;
    revised.known_at_utc = now + Duration::hours(1);
    assert_eq!(writer.write_incremental(&[revised]).unwrap(), 1);

    let wms = reader.scan_watermarks();
    let wm = wms.get("etf:QQQ:stockanalysis_shares_outstanding").unwrap();
    assert!(is_series_fresh(Some(wm), ObservationFrequency::Daily, now + Duration::hours(2)));
    assert!(!is_series_fresh(Some(wm), ObservationFrequency::Daily, now + Duration::hours(6)));
}

#[test]
fn test_m16_btc_etf_usd_normalization_and_weekly_flow_panel() {
    let d1 = Utc.with_ymd_and_hms(2026, 11, 18, 23, 59, 59).unwrap();
    let d2 = Utc.with_ymd_and_hms(2026, 11, 25, 23, 59, 59).unwrap();
    let btc_flow = vec![mk_obs("etf:BTC_US:bgeometrics_flow_btc", "etf_flow", "crypto", ObservationFrequency::Daily, d1, d1, 0, 2_000.0, "bgeometrics")];
    let candle = AggregatorCandle {
        time: d1.timestamp(),
        timestamp_utc: d1,
        canonical_symbol: "BTCUSDT".to_string(),
        timeframe: TimeFrame::D1,
        open: 90_000.0,
        high: 92_000.0,
        low: 89_000.0,
        close: 91_000.0,
        volume_base: 100.0,
        volume_quote: 9_100_000.0,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        source: "binance".to_string(),
        raw_symbol: "BTCUSDT".to_string(),
        ingestion_mode: IngestionMode::RestBatch,
        is_settled: true,
        sequence_id: None,
        vwap: None,
        trade_count: None,
        price_type: "last".to_string(),
        raw_metadata: None,
    };
    let usd_obs = compute_btc_etf_usd_flows(&btc_flow, &[candle]);
    assert_eq!(usd_obs.len(), 1);
    assert!((usd_obs[0].value - 182_000_000.0).abs() < 1.0);
    assert_eq!(usd_obs[0].provenance.source, "economy_aggregator_stage1");

    let mmf = vec![
        mk_obs("macro:US:FRED_WRMFNS_USD", "macro_liquidity", "fiat_park", ObservationFrequency::Weekly, d1, d1, 0, 3_000.0e9, "fred_api"),
        mk_obs("macro:US:FRED_WRMFNS_USD", "macro_liquidity", "fiat_park", ObservationFrequency::Weekly, d2, d2, 0, 3_025.0e9, "fred_api"),
    ];
    let panel = build_weekly_flow_panel(
        &WeeklyPanelInputs {
            spy_flows: &[],
            gld_flows: &[],
            bil_flows: &[],
            jnk_flows: &[],
            sptl_flows: &[],
            btc_etf_usd_flows: &usd_obs,
            btc_onchain_flows: &[],
            retail_mmf_stock: &mmf,
            stablecoin_stock: &[],
            bank_credit_stock: &[],
            fed_net_liq_stock: &[],
        },
        None,
        None,
    );
    assert_eq!(panel.total_weeks, 2);
    assert!((panel.rows[1].delta_retail_mmf_usd.unwrap() - 25.0e9).abs() < 1.0);
}

#[test]
fn test_m16_diebold_yilmaz_var_gfevd_causal_invariants() {
    // Construct deterministic 2-variable system where A causally drives B: B_t = 0.75 * A_{t-1} + noise_b
    let mut rows = Vec::with_capacity(120);
    let mut a_prev = 0.0_f64;
    let mut b_prev = 0.0_f64;
    for t in 0..120 {
        let tf = t as f64;
        let eps_a = (tf * 0.37).sin() + 0.4 * (tf * 1.13).cos();
        let eps_b = 0.25 * (tf * 0.71).cos() + 0.15 * (tf * 1.91).sin();
        let a = 0.45 * a_prev + eps_a;
        let b = 0.75 * a_prev + 0.2 * b_prev + eps_b;
        rows.push(vec![a, b]);
        a_prev = a;
        b_prev = b;
    }
    let cols = vec!["transmitter_A".to_string(), "receiver_B".to_string()];
    let report = compute_diebold_yilmaz_spillover(&cols, &rows, 2, 10).unwrap();

    // Mathematical invariants: every row sums to 100%, total NET sums to 0, A is net transmitter.
    for row in &report.connectedness_matrix_pct {
        let s: f64 = row.iter().sum();
        assert!((s - 100.0).abs() < 1e-6, "Row sum was {}", s);
    }
    let net_sum: f64 = report.directional.iter().map(|d| d.net_pct).sum();
    assert!(net_sum.abs() < 1e-6, "NET sum must equal 0, got {}", net_sum);
    assert!(report.directional[0].net_pct > 5.0, "A should be net transmitter: {:?}", report.directional);
    assert_eq!(report.directional[0].role, "net_transmitter");
    assert_eq!(report.directional[1].role, "net_receiver");

    // Fail-fast on insufficient rows
    assert!(compute_diebold_yilmaz_spillover(&cols, &rows[..5], 2, 10).is_err());
}

#[tokio::test]
async fn test_m16_rest_frequency_flags_and_spillover_fail_fast() {
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
    let writer = ObservationWriter::new(resolver.clone());

    let d1 = Utc.with_ymd_and_hms(2026, 10, 31, 23, 59, 59).unwrap();
    writer
        .write_observations(&[mk_obs(
            "credit:US:FINRA_MARGIN_DEBT_USD",
            "margin_debt",
            "deleveraging",
            ObservationFrequency::Monthly,
            d1,
            d1,
            0,
            1.4e12,
            "finra_xlsx",
        )])
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

    // Monthly series forward-filled on 1w matrix must emit explicit frequency_quality_flags
    let m_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/observations/matrix?series_ids=credit:US:FINRA_MARGIN_DEBT_USD&resample=1w&forward_fill=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(m_res.status(), StatusCode::OK);
    let m_body: serde_json::Value =
        serde_json::from_slice(&m_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        m_body["frequency_quality_flags"]["credit:US:FINRA_MARGIN_DEBT_USD"],
        "1M_step_carry_on_1w"
    );

    // Spillover endpoint fails fast with 422 when lake lacks sufficient multi-compartment weeks
    let sp_res = app
        .oneshot(
            Request::builder()
                .uri("/v1/analytics/liquidity-spillover?lags=2&horizon=8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sp_res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
