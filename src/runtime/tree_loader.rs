//! Shared Lake Input Loader & Watermark Fingerprinter for Diagnostic Trees (`v1.1.0`, Phase 359).
//! Single source of truth shared by REST routes and ContinuousAnalyticsSupervisor.

use std::collections::BTreeMap;
use std::path::Path;
use chrono::{Duration, NaiveDate};

use crate::core::observation_types::{sha256_hex, DataProcessingLayer, MetricObservation};
use crate::core::series_catalog::SeriesCatalog;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    build_weekly_flow_panel, compute_btc_etf_usd_flows, compute_diebold_yilmaz_spillover_full,
    compute_exact_share_flows, compute_fed_net_liquidity, DiagnosticTreeInputs,
    DiagnosticTreeMetrics, NullModel, WeeklyPanelInputs,
};
use crate::features::storage::{LakeReader, ObservationFilter, ObservationReader, PartitionResolver};
use crate::runtime::tree_loader_helpers::{
    candle_window_return_pct, funding_window_bps, push_obs_lineage, push_px_lineage,
    stock_window_delta, sum_flow_delta_v, sum_flow_exact, sum_obs_window,
};

/// Computes a deterministic SHA-256 fingerprint over all observation watermarks and core candle endpoints.
pub fn compute_lake_watermark_hash(lake_dir: &Path) -> String {
    let obs_reader = ObservationReader::new(PartitionResolver::new(lake_dir.to_path_buf()));
    let wms = obs_reader.scan_watermarks();
    let mut parts: Vec<String> = wms
        .into_iter()
        .map(|(sid, w)| {
            format!(
                "{}={}@{}#{}",
                sid,
                w.max_ts_utc.to_rfc3339(),
                w.latest_known_at_utc.to_rfc3339(),
                w.year_partitions
            )
        })
        .collect();
    for (ac, sym) in [("equities", "SPY"), ("crypto", "BTCUSDT"), ("equities", "^MOVE")] {
        let last = LakeReader::read_candles(lake_dir, Some(ac), sym, "1d", None, None)
            .ok()
            .and_then(|mut cs| {
                cs.sort_by_key(|c| c.timestamp_utc);
                cs.pop()
            });
        let stamp = last
            .map(|c| format!("{}={:.6}", c.timestamp_utc.to_rfc3339(), c.close))
            .unwrap_or_else(|| "none".to_string());
        parts.push(format!("candle:{}:{}={}", ac, sym, stamp));
    }
    parts.sort();
    sha256_hex(parts.join("\n").as_bytes())
}

/// Loads all lake observations/candles as-of `as_of` and constructs `DiagnosticTreeInputs`.
pub fn build_tree_inputs(
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
    let btc_oi_usd = read_one("deriv:BTC:bgeometrics_oi_usd", Some("bgeometrics"));
    let bybit_oi_c = read_one("deriv:BYBIT_BTC:open_interest_contracts", Some("bybit_v5"));
    let bin_fund = read_one("deriv:BINANCE_BTC:funding_rate_8h", Some("binance_futures"));
    let bybit_fund = read_one("deriv:BYBIT_BTC:funding_rate_8h", Some("bybit_v5"));

    let wrmfns = read_one("macro:US:FRED_WRMFNS_USD", Some("fred_api"));
    let bank_dep = read_one("macro:US:FRED_BANK_DEPOSITS_USD", Some("fred_api"));
    let bank_cred = read_one("macro:US:FRED_TOTBKCR_USD", Some("fred_api"));
    let wm2 = read_one("macro:US:FRED_WM2NS_USD", Some("fred_api"));
    let walcl = read_one("macro:US:FRED_WALCL_USD", Some("fred_api"));
    let tga = read_one("macro:US:TGA_BALANCE_USD", Some("treasury_fiscaldata"));
    let rrp = read_one("macro:US:FRED_RRPONTSYD_USD", Some("fred_api"));
    let sofr = read_one("macro:US:FRED_SOFR_PCT", Some("fred_api"));
    let dtb3 = read_one("macro:US:FRED_DTB3_PCT", Some("fred_api"));
    let hy_oas = read_one("macro:US:FRED_HY_OAS_PCT", Some("fred_api"));
    let stlfsi4 = read_one("macro:US:FRED_STLFSI4_IDX", Some("fred_api"));
    let cot_gold = read_one("cot:GOLD:managed_money_net", Some("cftc_cot"));
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
    let (delta_oi_usd, _) = stock_window_delta(&btc_oi_usd, win_start);
    let (delta_oi_c, delta_oi_c_pct) = stock_window_delta(&bybit_oi_c, win_start);
    let (delta_hy_pct, _) = stock_window_delta(&hy_oas, win_start);
    let (cot_gold_delta, _) = stock_window_delta(&cot_gold, win_start);
    let fund_series = if !bin_fund.is_empty() { &bin_fund } else { &bybit_fund };
    let (fund_avg_bps, fund_latest_bps) = funding_window_bps(fund_series, win_start);

    let finra_is_stale = finra.last().map_or(true, |o| {
        let d = o.observation_ts_utc.date_naive();
        d <= win_start || (as_of - d).num_days() > 45
    });
    let cot_gold_is_stale = cot_gold.last().map_or(true, |o| {
        let d = o.observation_ts_utc.date_naive();
        d <= win_start || (as_of - d).num_days() > 14
    });
    let latest_spy_date = spy_px.last().map(|c| c.timestamp_utc.date_naive());
    let h8_covers_latest = match (bank_cred.last(), latest_spy_date) {
        (Some(h8), Some(sd)) => h8.observation_ts_utc.date_naive() >= sd,
        _ => false,
    };
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
        let end_v = by_d.iter().next_back().filter(|(d, _)| **d > win_start).map(|(_, &v)| v);
        let start_v = by_d.range(..=win_start).next_back().map(|(_, &v)| v);
        end_v.zip(start_v).map(|(e, s)| e - s)
    };

    let (sp_z, sp_ba, sp_wp) = if include_spillover {
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
            NaiveDate::from_ymd_opt(2022, 1, 1),
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

    let mut lin = Vec::new();
    push_px_lineage(&mut lin, as_of, "candle:SPY:1d", &spy_px, DataProcessingLayer::UpstreamSplitAdjusted, "Yahoo Finance daily close is retroactively split-adjusted (dividends excluded).", &["Q0_MARKET_REGIME"]);
    push_px_lineage(&mut lin, as_of, "candle:BTCUSDT:1d", &btc_px, DataProcessingLayer::RawAsIs, "Binance spot 1d close stored verbatim.", &["Q0_MARKET_REGIME"]);
    push_px_lineage(&mut lin, as_of, "candle:^MOVE:1d", &move_px, DataProcessingLayer::RawAsIs, "ICE BofA MOVE bond volatility index close stored verbatim.", &["Q6_SYSTEMIC_CONTEXT_GAUGE"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "etf:SPY:shares_outstanding", &spy_sh, 7, &["Q1_ETF_REVAL_VS_PRIMARY", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "etf:BIL:shares_outstanding", &bil_sh, 7, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "etf:JNK:shares_outstanding", &jnk_sh, 7, &["Q1_ETF_REVAL_VS_PRIMARY", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "etf:GLD:shares_outstanding", &gld_sh, 7, &["Q2_DEFENSIVE_CASH_PARKING"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "etf:BTC_US:bgeometrics_flow_btc", &btc_etf_raw, 10, &["Q1_ETF_REVAL_VS_PRIMARY", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_DTB3_PCT", &dtb3, 7, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_SOFR_PCT", &sofr, 7, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_HY_OAS_PCT", &hy_oas, 7, &["Q3_CREDIT_AND_MACRO_CONTRACTION", "Q6_SYSTEMIC_CONTEXT_GAUGE"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_STLFSI4_IDX", &stlfsi4, 14, &["Q6_SYSTEMIC_CONTEXT_GAUGE"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "cot:GOLD:managed_money_net", &cot_gold, 14, &["Q2_DEFENSIVE_CASH_PARKING"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_WRMFNS_USD", &wrmfns, 21, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_BANK_DEPOSITS_USD", &bank_dep, 21, &["Q2_DEFENSIVE_CASH_PARKING", "Q5_CASH_PARKING_RETENTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_TOTBKCR_USD", &bank_cred, 21, &["Q3_CREDIT_AND_MACRO_CONTRACTION", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_WM2NS_USD", &wm2, 35, &["Q3_CREDIT_AND_MACRO_CONTRACTION", "Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_WALCL_USD", &walcl, 14, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:TGA_BALANCE_USD", &tga, 7, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "macro:US:FRED_RRPONTSYD_USD", &rrp, 7, &["Q5_CASH_PARKING_RETENTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "stablecoin:ALL:mcap_usd", &stbl, 7, &["Q4_RALLY_QUALITY_REVAL_VS_INFLOW"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "deriv:BYBIT_BTC:open_interest_contracts", &bybit_oi_c, 7, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "deriv:BINANCE_BTC:funding_rate_8h", &bin_fund, 7, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "deriv:BYBIT_BTC:funding_rate_8h", &bybit_fund, 7, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "deriv:BTC:bgeometrics_oi_usd", &btc_oi_usd, 10, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);
    push_obs_lineage(&mut lin, &catalog, as_of, "credit:US:FINRA_MARGIN_DEBT_USD", &finra, 45, &["Q3_CREDIT_AND_MACRO_CONTRACTION"]);

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
            delta_btc_oi_usd: delta_oi_usd,
            delta_btc_oi_contracts: delta_oi_c,
            delta_btc_oi_contracts_pct: delta_oi_c_pct,
            btc_funding_rate_8h_avg_bps: fund_avg_bps,
            btc_funding_rate_latest_bps: fund_latest_bps,
            sofr_latest_pct: sofr.last().map(|o| o.value),
            dtb3_latest_pct: dtb3.last().map(|o| o.value),
            hy_oas_latest_pct: hy_oas.last().map(|o| o.value),
            delta_hy_oas_bps: delta_hy_pct.map(|p| p * 100.0),
            stlfsi4_latest: stlfsi4.last().map(|o| o.value),
            cot_gold_managed_money_delta: cot_gold_delta,
            cot_gold_is_stale,
            h8_covers_latest_market_date: h8_covers_latest,
            finra_margin_delta_pct: finra_delta_pct,
            finra_is_stale,
            spillover_ex_rrp_z: sp_z,
            spillover_ex_rrp_bias_adj_pct: sp_ba,
            spillover_wild_break_p: sp_wp,
        },
        source_lineage: lin,
    }
}
