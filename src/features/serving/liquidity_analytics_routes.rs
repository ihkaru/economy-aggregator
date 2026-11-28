//! Stage-1 & Stage-2 Closed-System Liquidity Panel and Diebold-Yilmaz Spillover REST Routes (Phase 343).

use std::path::Path;
use std::sync::Arc;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::features::analytics::{
    build_weekly_flow_panel, compute_btc_etf_usd_flows,
    compute_diebold_yilmaz_spillover_with_options, compute_exact_share_flows,
    compute_fed_net_liquidity, compute_implied_net_flows, WeeklyFlowPanel, WeeklyPanelInputs,
};
use crate::features::serving::rest::AppState;
use crate::features::storage::{LakeReader, ObservationFilter, ObservationReader, PartitionResolver};

#[derive(Debug, Deserialize, Default)]
pub struct SpilloverQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub lags: Option<usize>,
    pub horizon: Option<usize>,
    /// "ex_rrp" (default: WALCL-TGA, avoids RRP<->MMF counterparty overlap) or "net_liquidity" (WALCL-TGA-RRP).
    pub liquidity_mode: Option<String>,
    /// Set true to drop fiat_park_mmf from the VAR (7-variable non-MMF system).
    pub exclude_mmf: Option<bool>,
    /// Permutation placebo resolution (default 1000, clamped 100..=2000).
    pub permutations: Option<usize>,
}

struct LoadedLiquidityData {
    spy_count: usize,
    gld_count: usize,
    bil_count: usize,
    jnk_count: usize,
    sptl_count: usize,
    btc_etf_usd_count: usize,
    fed_series_used: &'static str,
    rrp_series_used: &'static str,
    nl_points: Vec<crate::features::analytics::FedNetLiquidityPoint>,
    spy_latest: Vec<crate::features::analytics::ImpliedNetFlowPoint>,
    gld_latest: Vec<crate::features::analytics::ImpliedNetFlowPoint>,
    bil_latest: Vec<crate::features::analytics::ImpliedNetFlowPoint>,
    jnk_latest: Vec<crate::features::analytics::ImpliedNetFlowPoint>,
    sptl_latest: Vec<crate::features::analytics::ImpliedNetFlowPoint>,
    weekly_panel: WeeklyFlowPanel,
}

fn load_liquidity_bundle(
    lake_dir: &Path,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> LoadedLiquidityData {
    let obs_reader = ObservationReader::new(PartitionResolver::new(lake_dir.to_path_buf()));
    let read_one = |sid: &str, src: Option<&str>| {
        obs_reader.read_observations(&ObservationFilter {
            series_ids: vec![sid.to_string()],
            source: src.map(str::to_string),
            resolve_latest_revision: true,
            ..Default::default()
        })
    };

    let spy_aum = read_one("etf:SPY:aum_usd", Some("ssga_xlsx"));
    let gld_aum = read_one("etf:GLD:aum_usd", Some("ssga_xlsx"));
    let bil_aum = read_one("etf:BIL:aum_usd", Some("ssga_xlsx"));
    let jnk_aum = read_one("etf:JNK:aum_usd", Some("ssga_xlsx"));
    let sptl_aum = read_one("etf:SPTL:aum_usd", Some("ssga_xlsx"));

    let spy_sh = read_one("etf:SPY:shares_outstanding", Some("ssga_xlsx"));
    let gld_sh = read_one("etf:GLD:shares_outstanding", Some("ssga_xlsx"));
    let bil_sh = read_one("etf:BIL:shares_outstanding", Some("ssga_xlsx"));
    let jnk_sh = read_one("etf:JNK:shares_outstanding", Some("ssga_xlsx"));
    let sptl_sh = read_one("etf:SPTL:shares_outstanding", Some("ssga_xlsx"));

    let fred_walcl = read_one("macro:US:FRED_WALCL_USD", Some("fred_api"));
    let (fed_assets, fed_series_used) = if !fred_walcl.is_empty() {
        (fred_walcl, "macro:US:FRED_WALCL_USD (fred_api)")
    } else {
        (read_one("macro:US:FED_TOTAL_ASSETS_USD", Some("fed_h41_html")), "macro:US:FED_TOTAL_ASSETS_USD (fed_h41_html)")
    };
    let tga = read_one("macro:US:TGA_BALANCE_USD", Some("treasury_fiscaldata"));
    let fred_rrp = read_one("macro:US:FRED_RRPONTSYD_USD", Some("fred_api"));
    let (rrp, rrp_series_used) = if !fred_rrp.is_empty() {
        (fred_rrp, "macro:US:FRED_RRPONTSYD_USD (fred_api)")
    } else {
        (read_one("macro:US:ON_RRP_USD", Some("nyfed_api")), "macro:US:ON_RRP_USD (nyfed_api)")
    };

    let retail_mmf = read_one("macro:US:FRED_WRMFNS_USD", Some("fred_api"));
    let stablecoin_all = read_one("stablecoin:ALL:mcap_usd", Some("defillama"));
    let bank_credit = read_one("macro:US:FRED_TOTBKCR_USD", Some("fred_api"));
    let btc_etf_raw = read_one("etf:BTC_US:bgeometrics_flow_btc", Some("bgeometrics"));
    let btc_onchain = read_one("onchain:BTC:coinmetrics_netflow_usd", Some("coinmetrics_community"));

    let read_px = |ac: &str, sym: &str| {
        LakeReader::read_candles(lake_dir, Some(ac), sym, "1d", None, None).unwrap_or_default()
    };
    let resolve_flow = |aum: &[crate::core::observation_types::MetricObservation],
                        sh: &[crate::core::observation_types::MetricObservation],
                        ac: &str,
                        sym: &str,
                        comp: &str| {
        let exact = compute_exact_share_flows(aum, sh, sym, comp);
        if !exact.is_empty() {
            exact
        } else {
            compute_implied_net_flows(aum, &read_px(ac, sym), sym, comp)
        }
    };
    let spy_flows = resolve_flow(&spy_aum, &spy_sh, "equities", "SPY", "equities");
    let gld_flows = resolve_flow(&gld_aum, &gld_sh, "commodities", "GLD", "gold");
    let bil_flows = resolve_flow(&bil_aum, &bil_sh, "equities", "BIL", "fiat_park");
    let jnk_flows = resolve_flow(&jnk_aum, &jnk_sh, "equities", "JNK", "deleveraging");
    let sptl_flows = resolve_flow(&sptl_aum, &sptl_sh, "equities", "SPTL", "fiat_park");
    let btc_etf_usd = compute_btc_etf_usd_flows(&btc_etf_raw, &read_px("crypto", "BTCUSDT"));
    let nl_points = compute_fed_net_liquidity(&fed_assets, &tga, &rrp);

    let weekly_panel = build_weekly_flow_panel(
        &WeeklyPanelInputs {
            spy_flows: &spy_flows,
            gld_flows: &gld_flows,
            bil_flows: &bil_flows,
            jnk_flows: &jnk_flows,
            sptl_flows: &sptl_flows,
            btc_etf_usd_flows: &btc_etf_usd,
            btc_onchain_flows: &btc_onchain,
            retail_mmf_stock: &retail_mmf,
            stablecoin_stock: &stablecoin_all,
            bank_credit_stock: &bank_credit,
            fed_net_liq_stock: &nl_points,
        },
        start_date,
        end_date,
    );

    LoadedLiquidityData {
        spy_count: spy_flows.len(),
        gld_count: gld_flows.len(),
        bil_count: bil_flows.len(),
        jnk_count: jnk_flows.len(),
        sptl_count: sptl_flows.len(),
        btc_etf_usd_count: btc_etf_usd.len(),
        fed_series_used,
        rrp_series_used,
        nl_points,
        spy_latest: spy_flows.iter().rev().take(30).cloned().collect(),
        gld_latest: gld_flows.iter().rev().take(30).cloned().collect(),
        bil_latest: bil_flows.iter().rev().take(30).cloned().collect(),
        jnk_latest: jnk_flows.iter().rev().take(30).cloned().collect(),
        sptl_latest: sptl_flows.iter().rev().take(30).cloned().collect(),
        weekly_panel,
    }
}

pub async fn query_liquidity_panel_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let lake_dir = state.config.lake_dir.clone();
    let b = tokio::task::spawn_blocking(move || load_liquidity_bundle(&lake_dir, None, None))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "stage1_implied_flows": {
            "SPY_count": b.spy_count,
            "GLD_count": b.gld_count,
            "BIL_count": b.bil_count,
            "JNK_count": b.jnk_count,
            "SPTL_count": b.sptl_count,
            "BTC_ETF_USD_count": b.btc_etf_usd_count,
            "SPY_latest": b.spy_latest,
            "GLD_latest": b.gld_latest,
            "BIL_latest": b.bil_latest,
            "JNK_latest": b.jnk_latest,
            "SPTL_latest": b.sptl_latest,
        },
        "fed_net_liquidity": {
            "count": b.nl_points.len(),
            "fed_assets_series_used": b.fed_series_used,
            "tga_series_used": "macro:US:TGA_BALANCE_USD (treasury_fiscaldata)",
            "on_rrp_series_used": b.rrp_series_used,
            "latest": b.nl_points.iter().rev().take(30).cloned().collect::<Vec<_>>(),
        },
        "weekly_flow_panel": {
            "total_weeks": b.weekly_panel.total_weeks,
            "start_week": b.weekly_panel.start_week,
            "end_week": b.weekly_panel.end_week,
            "latest_12_weeks": b.weekly_panel.rows.iter().rev().take(12).cloned().collect::<Vec<_>>(),
        }
    })))
}

pub async fn query_liquidity_spillover_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SpilloverQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let start = parse_naive_opt(&q.start_date)?;
    let end = parse_naive_opt(&q.end_date)?;
    let lags = q.lags.unwrap_or(2);
    let horizon = q.horizon.unwrap_or(10);
    let permutations = q.permutations.unwrap_or(1000).clamp(100, 2000);
    let exclude_mmf = q.exclude_mmf.unwrap_or(false);
    let liq_mode = match q.liquidity_mode.as_deref().map(str::trim) {
        None | Some("") | Some("ex_rrp") => "ex_rrp",
        Some("net_liquidity") => "net_liquidity",
        Some(other) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Invalid liquidity_mode '{}'; expected 'ex_rrp' or 'net_liquidity'", other),
            ));
        }
    };
    let lake_dir = state.config.lake_dir.clone();

    let result = tokio::task::spawn_blocking(move || {
        let b = load_liquidity_bundle(&lake_dir, start, end);
        let mut candidates: Vec<(&str, fn(&crate::features::analytics::WeeklyFlowRow) -> Option<f64>)> = vec![
            ("equities_spy", |r| r.equities_spy_flow_usd),
            ("gold_gld", |r| r.gold_gld_flow_usd),
            ("fiat_park_bil", |r| r.fiat_park_bil_flow_usd),
        ];
        if !exclude_mmf {
            candidates.push(("fiat_park_mmf", |r| r.delta_retail_mmf_usd));
        }
        candidates.push(("credit_jnk", |r| r.credit_jnk_flow_usd));
        candidates.push(("crypto_stablecoin", |r| r.delta_stablecoin_mcap_usd));
        candidates.push(("crypto_onchain", |r| r.crypto_onchain_netflow_usd));
        if liq_mode == "ex_rrp" {
            candidates.push(("fed_walcl_minus_tga", |r| r.delta_fed_walcl_minus_tga_usd));
        } else {
            candidates.push(("fed_net_liquidity", |r| r.delta_fed_net_liquidity_usd));
        }

        // Keep every compartment with >= 25 valid weekly points in the window so the VAR is well-posed.
        let active: Vec<(&str, fn(&crate::features::analytics::WeeklyFlowRow) -> Option<f64>)> = candidates
            .into_iter()
            .filter(|(_, f)| b.weekly_panel.rows.iter().filter(|r| f(r).is_some()).count() >= 25)
            .collect();

        let col_names: Vec<String> = active.iter().map(|(name, _)| (*name).to_string()).collect();
        let mut aligned_dates = Vec::new();
        let mut matrix = Vec::new();
        for row in &b.weekly_panel.rows {
            let mut vals = Vec::with_capacity(active.len());
            let mut complete = true;
            for (_, f) in &active {
                match f(row) {
                    Some(v) if v.is_finite() => vals.push(v),
                    _ => {
                        complete = false;
                        break;
                    }
                }
            }
            if complete && vals.len() == active.len() {
                aligned_dates.push(row.week_friday.clone());
                matrix.push(vals);
            }
        }
        let report =
            compute_diebold_yilmaz_spillover_with_options(&col_names, &matrix, lags, horizon, permutations)?;
        Ok::<_, String>((aligned_dates.first().cloned(), aligned_dates.last().cloned(), report))
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match result {
        Ok((first_w, last_w, report)) => Ok(Json(serde_json::json!({
            "success": true,
            "liquidity_mode": liq_mode,
            "exclude_mmf": exclude_mmf,
            "aligned_start_week": first_w,
            "aligned_end_week": last_w,
            "spillover": report
        }))),
        Err(msg) => Err((StatusCode::UNPROCESSABLE_ENTITY, msg)),
    }
}

fn parse_naive_opt(raw: &Option<String>) -> Result<Option<NaiveDate>, (StatusCode, String)> {
    match raw.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| (StatusCode::BAD_REQUEST, format!("Invalid date '{}', expected YYYY-MM-DD", s))),
    }
}
