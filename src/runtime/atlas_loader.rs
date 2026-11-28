//! Zero-Lookahead Single-Shot Loader for the 6-Tank Liquidity & Capital Flow Atlas (Phase 363).
//! Strictly filters every series to `date <= as_of` so historical date-shifting has zero lookahead.

use std::path::Path;
use chrono::{Duration, NaiveDate};

use crate::core::atlas_types::{AtlasTank, LiquidityAtlasResponse};
use crate::core::observation_types::{DataProcessingLayer, MetricObservation};
use crate::core::series_catalog::SeriesCatalog;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    compute_btc_etf_usd_flows, compute_exact_share_flows, compute_fed_net_liquidity,
    evaluate_liquidity_tree,
};
use crate::features::storage::{LakeReader, ObservationFilter, ObservationReader, PartitionResolver};
use crate::runtime::atlas_series_builders::{
    candle_series, daily_avg_funding_series, fed_ex_rrp_series, lag_warning, obs_level_series,
    weekly_flow_series, weekly_obs_sum_series, weekly_stock_delta_series,
};
use crate::runtime::build_tree_inputs;

pub fn resolve_range_start(as_of: NaiveDate, range: &str) -> (String, NaiveDate) {
    match range.trim().to_ascii_lowercase().as_str() {
        "12w" | "3m" => ("12w".to_string(), as_of - Duration::days(84)),
        "6m" | "26w" => ("6m".to_string(), as_of - Duration::days(182)),
        "3y" => ("3y".to_string(), as_of - Duration::days(365 * 3)),
        "5y" => ("5y".to_string(), as_of - Duration::days(365 * 5)),
        "all" | "max" => ("all".to_string(), NaiveDate::from_ymd_opt(2016, 1, 1).unwrap()),
        _ => ("1y".to_string(), as_of - Duration::days(365)),
    }
}

pub fn build_liquidity_atlas(
    lake_dir: &Path,
    as_of: NaiveDate,
    range: &str,
    window_weeks: usize,
    include_spillover: bool,
) -> LiquidityAtlasResponse {
    let (norm_range, start_date) = resolve_range_start(as_of, range);
    let win_start = as_of - Duration::days((window_weeks as i64) * 7);
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

    let all_spy = LakeReader::read_candles(lake_dir, Some("equities"), "SPY", "1d", None, None).unwrap_or_default();
    let all_btc = LakeReader::read_candles(lake_dir, Some("crypto"), "BTCUSDT", "1d", None, None).unwrap_or_default();
    let earliest_date = all_spy
        .iter()
        .chain(all_btc.iter())
        .map(|c| c.timestamp_utc.date_naive())
        .min()
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2016, 1, 4).unwrap());
    let latest_date = all_spy
        .iter()
        .chain(all_btc.iter())
        .map(|c| c.timestamp_utc.date_naive())
        .max()
        .unwrap_or(as_of);

    let spy_px = read_px("equities", "SPY");
    let btc_px = read_px("crypto", "BTCUSDT");
    let gld_px = read_px("commodities", "GLD");

    let spy_fl = compute_exact_share_flows(&read_one("etf:SPY:aum_usd", Some("ssga_xlsx")), &read_one("etf:SPY:shares_outstanding", Some("ssga_xlsx")), "SPY", "equities");
    let gld_fl = compute_exact_share_flows(&read_one("etf:GLD:aum_usd", Some("ssga_xlsx")), &read_one("etf:GLD:shares_outstanding", Some("ssga_xlsx")), "GLD", "gold");
    let bil_fl = compute_exact_share_flows(&read_one("etf:BIL:aum_usd", Some("ssga_xlsx")), &read_one("etf:BIL:shares_outstanding", Some("ssga_xlsx")), "BIL", "fiat_park");
    let jnk_fl = compute_exact_share_flows(&read_one("etf:JNK:aum_usd", Some("ssga_xlsx")), &read_one("etf:JNK:shares_outstanding", Some("ssga_xlsx")), "JNK", "deleveraging");
    let sptl_fl = compute_exact_share_flows(&read_one("etf:SPTL:aum_usd", Some("ssga_xlsx")), &read_one("etf:SPTL:shares_outstanding", Some("ssga_xlsx")), "SPTL", "fiat_park");
    let btc_etf = compute_btc_etf_usd_flows(&read_one("etf:BTC_US:bgeometrics_flow_btc", Some("bgeometrics")), &btc_px);

    let stbl = read_one("stablecoin:ALL:mcap_usd", Some("defillama"));
    let bybit_oi = read_one("deriv:BYBIT_BTC:open_interest_contracts", Some("bybit_v5"));
    let bin_fund = read_one("deriv:BINANCE_BTC:funding_rate_8h", Some("binance_futures"));
    let bybit_fund = read_one("deriv:BYBIT_BTC:funding_rate_8h", Some("bybit_v5"));
    let fund = if !bin_fund.is_empty() { bin_fund } else { bybit_fund };
    let cot_gold = read_one("cot:GOLD:managed_money_net", Some("cftc_cot"));
    let dtb3 = read_one("macro:US:FRED_DTB3_PCT", Some("fred_api"));
    let sofr = read_one("macro:US:FRED_SOFR_PCT", Some("fred_api"));
    let wrmfns = read_one("macro:US:FRED_WRMFNS_USD", Some("fred_api"));
    let rrp = read_one("macro:US:FRED_RRPONTSYD_USD", Some("fred_api"));
    let bank_dep = read_one("macro:US:FRED_BANK_DEPOSITS_USD", Some("fred_api"));
    let bank_cred = read_one("macro:US:FRED_TOTBKCR_USD", Some("fred_api"));
    let hy_oas = read_one("macro:US:FRED_HY_OAS_PCT", Some("fred_api"));
    let finra = read_one("credit:US:FINRA_MARGIN_DEBT_USD", Some("finra_xlsx"));
    let walcl = read_one("macro:US:FRED_WALCL_USD", Some("fred_api"));
    let tga = read_one("macro:US:TGA_BALANCE_USD", Some("treasury_fiscaldata"));
    let nl_pts = compute_fed_net_liquidity(&walcl, &tga, &rrp);

    let spy_flow_s = weekly_flow_series("etf:SPY:exact_flow_usd", "SPY Primary Units ($B/wk)", "right", "#089981", &spy_fl, false, start_date, win_start, as_of, "ssga_xlsx");
    let spy_aum_s = weekly_flow_series("etf:SPY:delta_aum_usd", "SPY Weekly Delta AUM ($B)", "left", "#747d94", &spy_fl, true, start_date, win_start, as_of, "ssga_xlsx");
    let spy_px_s = candle_series("candle:SPY:1d", "SPY Close ($)", "left", "#3b82f6", &spy_px, DataProcessingLayer::UpstreamSplitAdjusted, start_date, win_start, as_of);
    let tank_equities = AtlasTank {
        tank_id: "equities_spy".to_string(),
        title: "1. US Equities (SPY)".to_string(),
        subtitle: "Primary AP Unit Creations/Redemptions vs Price Revaluation".to_string(),
        accounting_scope_note: "Tier 1 accounting identity on SSGA raw shares. Does not identify buyer identity or motive (rebalancing, dealer hedging, retail/institutional).".to_string(),
        staleness_warning: lag_warning(&spy_flow_s, 7),
        primary_series: spy_flow_s,
        overlay_series: vec![spy_px_s, spy_aum_s],
    };

    let btc_etf_s = weekly_obs_sum_series("etf:BTC_US:implied_flow_usd", "US Spot BTC ETF ($B/wk)", "right", "#f59e0b", 1e9, &btc_etf, DataProcessingLayer::DerivedAdjusted, "bgeometrics+binance", start_date, win_start, as_of);
    let stbl_delta_s = weekly_stock_delta_series("stablecoin:ALL:delta_mcap_usd", "Delta Stablecoin Mcap ($B/wk)", "right", "#06b6d4", 1e9, &stbl, &catalog, start_date, win_start, as_of);
    let bybit_oi_s = obs_level_series("deriv:BYBIT_BTC:open_interest_contracts", "Bybit BTC OI (Contracts)", "left", "#a855f7", 1.0, "contracts", &bybit_oi, &catalog, start_date, win_start, as_of);
    let fund_s = daily_avg_funding_series("deriv:BTC:funding_rate_8h", "BTC 8h Funding (bps)", "left", "#ec4899", &fund, start_date, win_start, as_of);
    let tank_crypto = AtlasTank {
        tank_id: "crypto_btc".to_string(),
        title: "2. Crypto Complex (BTC)".to_string(),
        subtitle: "Spot BTC ETF Flows, Stablecoin Net Supply, Contract OI & 8h Funding".to_string(),
        accounting_scope_note: "Bybit OI is measured in BTC contracts (free of USD price revaluation bias). Stablecoin contraction is consistent with net redemptions across DefiLlama coverage.".to_string(),
        staleness_warning: lag_warning(&btc_etf_s, 7).or_else(|| lag_warning(&fund_s, 3)),
        primary_series: btc_etf_s,
        overlay_series: vec![stbl_delta_s, bybit_oi_s, fund_s],
    };

    let gld_flow_s = weekly_flow_series("etf:GLD:exact_flow_usd", "GLD Primary Units ($B/wk)", "right", "#eab308", &gld_fl, false, start_date, win_start, as_of, "ssga_xlsx");
    let cot_gold_s = obs_level_series("cot:GOLD:managed_money_net", "CFTC Gold MM Net (Contracts)", "left", "#60a5fa", 1.0, "contracts", &cot_gold, &catalog, start_date, win_start, as_of);
    let gld_px_s = candle_series("candle:GLD:1d", "GLD Close ($)", "left", "#94a3b8", &gld_px, DataProcessingLayer::UpstreamSplitAdjusted, start_date, win_start, as_of);
    let tank_gold = AtlasTank {
        tank_id: "gold_gld".to_string(),
        title: "3. Financial Gold (GLD & COMEX)".to_string(),
        subtitle: "GLD Primary Unit Flows vs CFTC Managed Money Positioning".to_string(),
        accounting_scope_note: "Covers US financial gold ETF (GLD) and COMEX futures only; excludes central bank purchases and Asian physical demand. Does not prove seller motive.".to_string(),
        staleness_warning: lag_warning(&gld_flow_s, 7).or_else(|| lag_warning(&cot_gold_s, 10)),
        primary_series: gld_flow_s,
        overlay_series: vec![cot_gold_s, gld_px_s],
    };

    let bil_flow_s = weekly_flow_series("etf:BIL:exact_flow_usd", "BIL T-Bill Units ($B/wk)", "right", "#089981", &bil_fl, false, start_date, win_start, as_of, "ssga_xlsx");
    let dtb3_s = obs_level_series("macro:US:FRED_DTB3_PCT", "3M T-Bill Yield DTB3 (%)", "left", "#f59e0b", 1.0, "%", &dtb3, &catalog, start_date, win_start, as_of);
    let sofr_s = obs_level_series("macro:US:FRED_SOFR_PCT", "SOFR Rate (%)", "left", "#38bdf8", 1.0, "%", &sofr, &catalog, start_date, win_start, as_of);
    let wrm_s = obs_level_series("macro:US:FRED_WRMFNS_USD", "Retail MMF WRMFNS ($T)", "left", "#a78bfa", 1e12, "$T", &wrmfns, &catalog, start_date, win_start, as_of);
    let rrp_s = obs_level_series("macro:US:FRED_RRPONTSYD_USD", "Fed ON RRP ($B)", "left", "#fb7185", 1e9, "$B", &rrp, &catalog, start_date, win_start, as_of);
    let tank_cash = AtlasTank {
        tank_id: "cash_short_term".to_string(),
        title: "4. Short-Term Cash (BIL, MMF, RRP)".to_string(),
        subtitle: "T-Bill & Retail MMF Flows Alongside Short-Term Yield Context".to_string(),
        accounting_scope_note: "Elevated DTB3/SOFR yields mean BIL inflows cannot separate precautionary cash holding from yield/carry allocation. Weekly institutional MMF (WIMFNS) discontinued by Fed in 2021.".to_string(),
        staleness_warning: lag_warning(&bil_flow_s, 7).or_else(|| lag_warning(&wrm_s, 14)),
        primary_series: bil_flow_s,
        overlay_series: vec![dtb3_s, wrm_s, rrp_s, sofr_s],
    };

    let dep_s = obs_level_series("macro:US:FRED_BANK_DEPOSITS_USD", "H.8 Bank Deposits ($T, SA)", "right", "#3b82f6", 1e12, "$T", &bank_dep, &catalog, start_date, win_start, as_of);
    let hy_s = obs_level_series("macro:US:FRED_HY_OAS_PCT", "Daily HY Credit Spread (%)", "left", "#f23645", 1.0, "%", &hy_oas, &catalog, start_date, win_start, as_of);
    let cred_s = obs_level_series("macro:US:FRED_TOTBKCR_USD", "H.8 Bank Credit ($T, SA)", "right", "#10b981", 1e12, "$T", &bank_cred, &catalog, start_date, win_start, as_of);
    let jnk_s = weekly_flow_series("etf:JNK:exact_flow_usd", "JNK High-Yield Units ($B/wk)", "left", "#f59e0b", &jnk_fl, false, start_date, win_start, as_of, "ssga_xlsx");
    let finra_s = obs_level_series("credit:US:FINRA_MARGIN_DEBT_USD", "FINRA Margin Debt ($B, 1M)", "left", "#c084fc", 1e9, "$B", &finra, &catalog, start_date, win_start, as_of);
    let tank_credit = AtlasTank {
        tank_id: "credit_banks".to_string(),
        title: "5. US Banks & Credit (H.8, HY OAS, FINRA)".to_string(),
        subtitle: "Seasonally Adjusted Banking Balances, Daily Credit Spreads & Margin Debt".to_string(),
        accounting_scope_note: "Fed H.8 (~9-10d lag, upstream SA) and FINRA (~21-30d lag) stop at their actual release dates; unprinted weeks are never extrapolated. Daily HY OAS bridges recent market sessions.".to_string(),
        staleness_warning: lag_warning(&dep_s, 7).or_else(|| lag_warning(&finra_s, 21)),
        primary_series: dep_s,
        overlay_series: vec![hy_s, cred_s, jnk_s, finra_s],
    };

    let ex_rrp_s = fed_ex_rrp_series(&nl_pts, start_date, win_start, as_of);
    let tga_s = obs_level_series("macro:US:TGA_BALANCE_USD", "Treasury TGA Balance ($B)", "left", "#f59e0b", 1e9, "$B", &tga, &catalog, start_date, win_start, as_of);
    let sptl_s = weekly_flow_series("etf:SPTL:exact_flow_usd", "SPTL Long Treasury ($B/wk)", "left", "#38bdf8", &sptl_fl, false, start_date, win_start, as_of, "ssga_xlsx");
    let tank_macro = AtlasTank {
        tank_id: "macro_fed_pipes".to_string(),
        title: "6. Macro Pipes (Fed ex-RRP, TGA, SPTL)".to_string(),
        subtitle: "Weekly Fed WALCL Minus TGA Balance & Long-Duration Treasury Flows".to_string(),
        accounting_scope_note: "Uses ex-RRP specification (WALCL - TGA, same-day Wednesday alignment) to avoid RRP-MMF accounting mirror. Descriptive balance-sheet tracking only.".to_string(),
        staleness_warning: lag_warning(&ex_rrp_s, 10),
        primary_series: ex_rrp_s,
        overlay_series: vec![tga_s, sptl_s],
    };

    let tree_inputs = build_tree_inputs(lake_dir, as_of, window_weeks, include_spillover);
    let diagnostic_tree = evaluate_liquidity_tree(tree_inputs);

    LiquidityAtlasResponse {
        as_of_date: as_of.format("%Y-%m-%d").to_string(),
        range: norm_range,
        range_start_date: start_date.format("%Y-%m-%d").to_string(),
        window_weeks,
        earliest_available_date: earliest_date.format("%Y-%m-%d").to_string(),
        latest_available_date: latest_date.format("%Y-%m-%d").to_string(),
        tanks: vec![tank_equities, tank_crypto, tank_gold, tank_cash, tank_credit, tank_macro],
        diagnostic_tree,
    }
}
