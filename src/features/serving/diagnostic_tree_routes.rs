//! REST API Routes for Deterministic Liquidity Diagnostic Tree & Persistent Backtrack Ledger (Phase 349).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{Duration, NaiveDate, Utc};
use serde::Deserialize;

use crate::core::diagnostic_types::SourceLineageEntry;
use crate::core::observation_types::{DataProcessingLayer, MetricObservation};
use crate::core::series_catalog::SeriesCatalog;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    build_weekly_flow_panel, compute_btc_etf_usd_flows, compute_diebold_yilmaz_spillover_full,
    compute_exact_share_flows, compute_fed_net_liquidity, evaluate_liquidity_tree,
    DiagnosticTreeInputs, DiagnosticTreeMetrics, ImpliedNetFlowPoint, NullModel, WeeklyPanelInputs,
};
use crate::features::serving::auth::validate_safe_identifier;
use crate::features::serving::rest::AppState;
use crate::features::storage::{
    DiagnosticTreeLedger, LakeReader, ObservationFilter, ObservationReader, PartitionResolver,
};

#[derive(Debug, Deserialize, Default)]
pub struct DiagnosticTreeQuery {
    pub as_of: Option<String>,
    pub window_weeks: Option<usize>,
    pub persist: Option<bool>,
    pub include_spillover: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
}

pub async fn query_liquidity_tree_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DiagnosticTreeQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let as_of = match q.as_of.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Utc::now().date_naive(),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| (StatusCode::BAD_REQUEST, format!("Invalid as_of '{}', expected YYYY-MM-DD", s)))?,
    };
    let window_weeks = q.window_weeks.unwrap_or(4).clamp(1, 52);
    let persist = q.persist.unwrap_or(false);
    let include_spillover = q.include_spillover.unwrap_or(true);
    let lake_dir = state.config.lake_dir.clone();

    let report = tokio::task::spawn_blocking(move || {
        let inputs = build_tree_inputs(&lake_dir, as_of, window_weeks, include_spillover);
        let mut rep = evaluate_liquidity_tree(inputs);
        if persist {
            let ledger = DiagnosticTreeLedger::new(&lake_dir);
            let path = ledger.persist_run(&rep)?;
            rep.persisted_path = Some(path.display().to_string());
        }
        Ok::<_, String>(rep)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({ "success": true, "diagnostic_tree": report })))
}

pub async fn list_liquidity_tree_history_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let limit = q.limit.unwrap_or(20).clamp(1, 200);
    let lake_dir = state.config.lake_dir.clone();
    let runs = tokio::task::spawn_blocking(move || DiagnosticTreeLedger::new(&lake_dir).list_runs(limit))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({ "success": true, "count": runs.len(), "runs": runs })))
}

pub async fn get_liquidity_tree_run_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    validate_safe_identifier(&run_id, "run_id")?;
    let lake_dir = state.config.lake_dir.clone();
    let found = tokio::task::spawn_blocking(move || DiagnosticTreeLedger::new(&lake_dir).get_run(&run_id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    match found {
        Some(rep) => Ok(Json(serde_json::json!({ "success": true, "diagnostic_tree": rep }))),
        None => Err((StatusCode::NOT_FOUND, "Diagnostic tree run_id not found".to_string())),
    }
}

fn build_tree_inputs(
    lake_dir: &Path,
    as_of: NaiveDate,
    window_weeks: usize,
    include_spillover: bool,
) -> DiagnosticTreeInputs {
    let obs_reader = ObservationReader::new(PartitionResolver::new(lake_dir.to_path_buf()));
    let catalog = SeriesCatalog::new_canonical();
    let read_one = |sid: &str, src: Option<&str>| -> Vec<MetricObservation> {
        let mut rows = obs_reader.read_observations(&ObservationFilter {
            series_ids: vec![sid.to_string()],
            source: src.map(str::to_string),
            resolve_latest_revision: true,
            ..Default::default()
        });
        rows.retain(|o| o.observation_ts_utc.date_naive() <= as_of);
        rows.sort_by_key(|o| o.observation_ts_utc);
        rows
    };
    let read_px = |ac: &str, sym: &str| -> Vec<AggregatorCandle> {
        let mut cs = LakeReader::read_candles(lake_dir, Some(ac), sym, "1d", None, None).unwrap_or_default();
        cs.retain(|c| c.timestamp_utc.date_naive() <= as_of);
        cs.sort_by_key(|c| c.timestamp_utc);
        cs
    };

    let spy_px = read_px("equities", "SPY");
    let btc_px = read_px("crypto", "BTCUSDT");
    let move_px = read_px("equities", "^MOVE");

    let spy_aum = read_one("etf:SPY:aum_usd", Some("ssga_xlsx"));
    let spy_sh = read_one("etf:SPY:shares_outstanding", Some("ssga_xlsx"));
    let gld_aum = read_one("etf:GLD:aum_usd", Some("ssga_xlsx"));
    let gld_sh = read_one("etf:GLD:shares_outstanding", Some("ssga_xlsx"));
    let bil_aum = read_one("etf:BIL:aum_usd", Some("ssga_xlsx"));
    let bil_sh = read_one("etf:BIL:shares_outstanding", Some("ssga_xlsx"));
    let jnk_aum = read_one("etf:JNK:aum_usd", Some("ssga_xlsx"));
    let jnk_sh = read_one("etf:JNK:shares_outstanding", Some("ssga_xlsx"));
    let sptl_aum = read_one("etf:SPTL:aum_usd", Some("ssga_xlsx"));
    let sptl_sh = read_one("etf:SPTL:shares_outstanding", Some("ssga_xlsx"));

    let btc_etf_raw = read_one("etf:BTC_US:bgeometrics_flow_btc", Some("bgeometrics"));
    let btc_onchain = read_one("onchain:BTC:coinmetrics_netflow_usd", Some("coinmetrics_community"));
    let btc_oi = read_one("deriv:BTC:bgeometrics_oi_usd", Some("bgeometrics"));
    let wrmfns = read_one("macro:US:FRED_WRMFNS_USD", Some("fred_api"));
    let bank_dep = read_one("macro:US:FRED_BANK_DEPOSITS_USD", Some("fred_api"));
    let bank_cred = read_one("macro:US:FRED_TOTBKCR_USD", Some("fred_api"));
    let wm2 = read_one("macro:US:FRED_WM2NS_USD", Some("fred_api"));
    let walcl = read_one("macro:US:FRED_WALCL_USD", Some("fred_api"));
    let tga = read_one("macro:US:TGA_BALANCE_USD", Some("treasury_fiscaldata"));
    let rrp = read_one("macro:US:FRED_RRPONTSYD_USD", Some("fred_api"));
    let stbl = read_one("stablecoin:ALL:mcap_usd", Some("defillama"));
    let finra = read_one("credit:US:FINRA_MARGIN_DEBT_USD", Some("finra_xlsx"));

    let spy_fl = compute_exact_share_flows(&spy_aum, &spy_sh, "SPY", "equities");
    let gld_fl = compute_exact_share_flows(&gld_aum, &gld_sh, "GLD", "gold");
    let bil_fl = compute_exact_share_flows(&bil_aum, &bil_sh, "BIL", "fiat_park");
    let jnk_fl = compute_exact_share_flows(&jnk_aum, &jnk_sh, "JNK", "deleveraging");
    let sptl_fl = compute_exact_share_flows(&sptl_aum, &sptl_sh, "SPTL", "fiat_park");
    let btc_etf_usd = compute_btc_etf_usd_flows(&btc_etf_raw, &btc_px);
    let nl_pts = compute_fed_net_liquidity(&walcl, &tga, &rrp);

    let win_start = as_of - Duration::days((window_weeks as i64) * 7);
    let (delta_wrm, delta_wrm_pct) = stock_window_delta(&wrmfns, win_start);
    let (delta_dep, delta_dep_pct) = stock_window_delta(&bank_dep, win_start);
    let (delta_cred, _) = stock_window_delta(&bank_cred, win_start);
    let (delta_m2, _) = stock_window_delta(&wm2, win_start);
    let (delta_rrp, _) = stock_window_delta(&rrp, win_start);
    let (delta_stbl, _) = stock_window_delta(&stbl, win_start);
    let (delta_oi, _) = stock_window_delta(&btc_oi, win_start);
    let finra_is_stale = finra.last().map_or(true, |o| {
        let d = o.observation_ts_utc.date_naive();
        d <= win_start || (as_of - d).num_days() > 45
    });
    let finra_delta_pct = if finra.len() >= 2 && finra[finra.len() - 2].value > 0.0 {
        Some(((finra[finra.len() - 1].value / finra[finra.len() - 2].value) - 1.0) * 100.0)
    } else {
        None
    };

    let delta_walcl_tga = {
        let mut by_d: BTreeMap<NaiveDate, f64> = BTreeMap::new();
        for p in &nl_pts {
            by_d.insert(p.timestamp_utc.date_naive(), p.fed_assets_usd - p.tga_balance_usd);
        }
        let end_entry = by_d.iter().next_back().filter(|(d, _)| **d > win_start).map(|(_, &v)| v);
        let start_v = by_d.range(..=win_start).next_back().map(|(_, &v)| v);
        end_entry.zip(start_v).map(|(e, s)| e - s)
    };

    let (sp_z, sp_ba, sp_wp) = if include_spillover {
        let start_reg = NaiveDate::from_ymd_opt(2022, 1, 1);
        let wp = build_weekly_flow_panel(
            &WeeklyPanelInputs {
                spy_flows: &spy_fl,
                gld_flows: &gld_fl,
                bil_flows: &bil_fl,
                jnk_flows: &jnk_fl,
                sptl_flows: &sptl_fl,
                btc_etf_usd_flows: &btc_etf_usd,
                btc_onchain_flows: &btc_onchain,
                retail_mmf_stock: &wrmfns,
                stablecoin_stock: &stbl,
                bank_credit_stock: &bank_cred,
                fed_net_liq_stock: &nl_pts,
            },
            start_reg,
            Some(as_of),
        );
        let cols: Vec<String> = [
            "equities_spy", "gold_gld", "fiat_park_bil", "fiat_park_mmf",
            "credit_jnk", "crypto_stablecoin", "crypto_onchain", "fed_walcl_minus_tga",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let mut dates = Vec::new();
        let mut mat = Vec::new();
        for r in &wp.rows {
            let vals = [
                r.equities_spy_flow_usd, r.gold_gld_flow_usd, r.fiat_park_bil_flow_usd,
                r.delta_retail_mmf_usd, r.credit_jnk_flow_usd, r.delta_stablecoin_mcap_usd,
                r.crypto_onchain_netflow_usd, r.delta_fed_walcl_minus_tga_usd,
            ];
            if vals.iter().all(|v| v.is_some_and(f64::is_finite)) {
                dates.push(r.week_friday.clone());
                mat.push(vals.into_iter().flatten().collect());
            }
        }
        match compute_diebold_yilmaz_spillover_full(&cols, &mat, Some(&dates), 2, 10, 500, NullModel::CircularShift) {
            Ok(rep) => (Some(rep.tci_z_score), Some(rep.bias_adjusted_tci_pct), Some(rep.structural_break.system_wild_p_value)),
            Err(_) => (None, None, None),
        }
    } else {
        (None, None, None)
    };

    let mut lineage = Vec::new();
    let push_obs_lin = |lin: &mut Vec<SourceLineageEntry>, sid: &str, rows: &[MetricObservation], max_lag: i64, nodes: &[&str]| {
        let meta = catalog.get(sid);
        let last = rows.last();
        let lag = last.map(|o| (as_of - o.observation_ts_utc.date_naive()).num_days());
        lin.push(SourceLineageEntry {
            series_id: sid.to_string(),
            source: last.map(|o| o.provenance.source.clone()).or_else(|| meta.map(|m| m.primary_source.clone())).unwrap_or_else(|| "missing".to_string()),
            source_series_id: last.map(|o| o.provenance.source_series_id.clone()).unwrap_or_default(),
            source_url: last.map(|o| o.provenance.source_url.clone()).unwrap_or_default(),
            source_checksum: last.and_then(|o| o.provenance.source_checksum.clone()),
            batch_id: last.map(|o| o.provenance.batch_id.clone()).unwrap_or_default(),
            data_layer: meta.map(|m| m.data_layer).unwrap_or(DataProcessingLayer::RawAsIs),
            raw_value_preserved: meta.map(|m| m.raw_value_preserved).unwrap_or(true),
            adjustment_note: meta.and_then(|m| m.adjustment_note.clone()),
            known_limitation: meta.and_then(|m| m.limitation_note.clone()),
            latest_observation_utc: last.map(|o| o.observation_ts_utc.to_rfc3339()),
            stale_lag_days: lag,
            is_stale: lag.map_or(true, |d| d > max_lag),
            used_by_nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
        });
    };
    let push_px_lin = |lin: &mut Vec<SourceLineageEntry>, sid: &str, cs: &[AggregatorCandle], layer: DataProcessingLayer, adj: &str, nodes: &[&str]| {
        let last = cs.last();
        let lag = last.map(|c| (as_of - c.timestamp_utc.date_naive()).num_days());
        lin.push(SourceLineageEntry {
            series_id: sid.to_string(),
            source: last.map(|c| c.source.clone()).unwrap_or_else(|| "missing".to_string()),
            source_series_id: sid.to_string(),
            source_url: "lake://market/candles".to_string(),
            source_checksum: None,
            batch_id: "candles_parquet".to_string(),
            data_layer: layer,
            raw_value_preserved: true,
            adjustment_note: Some(adj.to_string()),
            known_limitation: None,
            latest_observation_utc: last.map(|c| c.timestamp_utc.to_rfc3339()),
            stale_lag_days: lag,
            is_stale: lag.map_or(true, |d| d > 7),
            used_by_nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
        });
    };

    push_px_lin(&mut lineage, "candle:SPY:1d", &spy_px, DataProcessingLayer::UpstreamSplitAdjusted, "Yahoo Finance daily close is retroactively split-adjusted (dividends excluded).", &["Q0_MARKET_REGIME"]);
    push_px_lin(&mut lineage, "candle:BTCUSDT:1d", &btc_px, DataProcessingLayer::RawAsIs, "Binance spot 1d close stored verbatim.", &["Q0_MARKET_REGIME"]);
    push_px_lin(&mut lineage, "candle:^MOVE:1d", &move_px, DataProcessingLayer::RawAsIs, "ICE BofA MOVE bond volatility index close stored verbatim.", &["Q6_SYSTEMIC_CONTEXT_GAUGE"]);
    push_obs_lin(&mut lineage, "etf:SPY:shares_outstanding", &spy_sh, 7, &["Q1_ETF_REVAL_VS_PRIMARY", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lin(&mut lineage, "etf:BIL:shares_outstanding", &bil_sh, 7, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lin(&mut lineage, "etf:JNK:shares_outstanding", &jnk_sh, 7, &["Q1_ETF_REVAL_VS_PRIMARY", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lin(&mut lineage, "etf:GLD:shares_outstanding", &gld_sh, 7, &["Q2_DEFENSIVE_CASH_PARKING"]);
    push_obs_lin(&mut lineage, "etf:BTC_US:bgeometrics_flow_btc", &btc_etf_raw, 10, &["Q1_ETF_REVAL_VS_PRIMARY", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lin(&mut lineage, "macro:US:FRED_WRMFNS_USD", &wrmfns, 21, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lin(&mut lineage, "macro:US:FRED_BANK_DEPOSITS_USD", &bank_dep, 21, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lin(&mut lineage, "macro:US:FRED_TOTBKCR_USD", &bank_cred, 21, &["Q3_CREDIT_AND_MACRO_CONTRACTION", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lin(&mut lineage, "macro:US:FRED_WM2NS_USD", &wm2, 35, &["Q3_CREDIT_AND_MACRO_CONTRACTION", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lin(&mut lineage, "macro:US:FRED_WALCL_USD", &walcl, 14, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lin(&mut lineage, "macro:US:TGA_BALANCE_USD", &tga, 7, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lin(&mut lineage, "macro:US:FRED_RRPONTSYD_USD", &rrp, 7, &["Q5_CASH_PARKING_RETENTION"]);
    push_obs_lin(&mut lineage, "stablecoin:ALL:mcap_usd", &stbl, 7, &["Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lin(&mut lineage, "deriv:BTC:bgeometrics_oi_usd", &btc_oi, 10, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lin(&mut lineage, "credit:US:FINRA_MARGIN_DEBT_USD", &finra, 45, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);

    DiagnosticTreeInputs {
        as_of_date: as_of,
        window_weeks,
        metrics: DiagnosticTreeMetrics {
            spy_return_pct: candle_window_return_pct(&spy_px, win_start),
            btc_return_pct: candle_window_return_pct(&btc_px, win_start),
            move_latest: move_px.last().map(|c| c.close),
            spy_delta_aum_usd: sum_flow_delta_v(&spy_fl, win_start),
            spy_exact_flow_usd: sum_flow_exact(&spy_fl, win_start),
            gld_exact_flow_usd: sum_flow_exact(&gld_fl, win_start),
            bil_exact_flow_usd: sum_flow_exact(&bil_fl, win_start),
            jnk_exact_flow_usd: sum_flow_exact(&jnk_fl, win_start),
            btc_etf_flow_usd: sum_obs_window(&btc_etf_usd, win_start),
            delta_wrmfns_usd: delta_wrm,
            delta_wrmfns_pct: delta_wrm_pct,
            delta_bank_deposits_usd: delta_dep,
            delta_bank_deposits_pct: delta_dep_pct,
            delta_bank_credit_usd: delta_cred,
            delta_m2_usd: delta_m2,
            delta_walcl_minus_tga_usd: delta_walcl_tga,
            delta_on_rrp_usd: delta_rrp,
            delta_stablecoin_usd: delta_stbl,
            delta_btc_oi_usd: delta_oi,
            finra_margin_delta_pct: finra_delta_pct,
            finra_is_stale,
            spillover_ex_rrp_z: sp_z,
            spillover_ex_rrp_bias_adj_pct: sp_ba,
            spillover_wild_break_p: sp_wp,
        },
        source_lineage: lineage,
    }
}

fn candle_window_return_pct(cs: &[AggregatorCandle], win_start: NaiveDate) -> Option<f64> {
    let end = cs.last()?;
    if end.timestamp_utc.date_naive() <= win_start {
        return None;
    }
    let start = cs.iter().rev().find(|c| c.timestamp_utc.date_naive() <= win_start)?.close;
    (start > 0.0).then(|| ((end.close / start) - 1.0) * 100.0)
}

fn sum_flow_exact(pts: &[ImpliedNetFlowPoint], win_start: NaiveDate) -> Option<f64> {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for p in pts {
        if p.timestamp_utc.date_naive() > win_start && p.implied_net_flow_usd.is_finite() {
            sum += p.implied_net_flow_usd;
            cnt += 1;
        }
    }
    (cnt > 0).then_some(sum)
}

fn sum_flow_delta_v(pts: &[ImpliedNetFlowPoint], win_start: NaiveDate) -> Option<f64> {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for p in pts {
        if p.timestamp_utc.date_naive() > win_start && p.delta_v_usd.is_finite() {
            sum += p.delta_v_usd;
            cnt += 1;
        }
    }
    (cnt > 0).then_some(sum)
}

fn sum_obs_window(obs: &[MetricObservation], win_start: NaiveDate) -> Option<f64> {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for o in obs {
        if o.observation_ts_utc.date_naive() > win_start && o.value.is_finite() {
            sum += o.value;
            cnt += 1;
        }
    }
    (cnt > 0).then_some(sum)
}

fn stock_window_delta(obs: &[MetricObservation], win_start: NaiveDate) -> (Option<f64>, Option<f64>) {
    let Some(end) = obs.last() else { return (None, None) };
    if end.observation_ts_utc.date_naive() <= win_start {
        return (None, None);
    }
    let Some(start) = obs.iter().rev().find(|o| o.observation_ts_utc.date_naive() <= win_start) else {
        return (None, None);
    };
    let diff = end.value - start.value;
    let pct = (start.value > 0.0).then(|| (diff / start.value) * 100.0);
    (Some(diff), pct)
}
