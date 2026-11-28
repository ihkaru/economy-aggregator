//! Lake Input Loader for Frozen-Protocol Prospective OOS Scorecard (Phase 355).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use chrono::NaiveDate;

use crate::core::observation_types::MetricObservation;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    align_to_friday_date, build_weekly_flow_panel, compute_btc_etf_usd_flows,
    compute_diebold_yilmaz_spillover_full, compute_exact_share_flows, compute_fed_net_liquidity,
    NullModel, ProspectiveScorecardInputs, ProspectiveWeeklyObs, WeeklyPanelInputs,
};
use crate::features::storage::{LakeReader, ObservationFilter, ObservationReader, PartitionResolver};

pub fn build_prospective_inputs(
    lake_dir: &Path,
    as_of: NaiveDate,
    include_spillover: bool,
) -> ProspectiveScorecardInputs {
    let obs_reader = ObservationReader::new(PartitionResolver::new(lake_dir.to_path_buf()));
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

    let bil_fl = compute_exact_share_flows(
        &read_one("etf:BIL:aum_usd", Some("ssga_xlsx")),
        &read_one("etf:BIL:shares_outstanding", Some("ssga_xlsx")),
        "BIL",
        "fiat_park",
    );
    let jnk_fl = compute_exact_share_flows(
        &read_one("etf:JNK:aum_usd", Some("ssga_xlsx")),
        &read_one("etf:JNK:shares_outstanding", Some("ssga_xlsx")),
        "JNK",
        "deleveraging",
    );
    let gld_fl = compute_exact_share_flows(
        &read_one("etf:GLD:aum_usd", Some("ssga_xlsx")),
        &read_one("etf:GLD:shares_outstanding", Some("ssga_xlsx")),
        "GLD",
        "gold",
    );
    let btc_etf_usd = compute_btc_etf_usd_flows(
        &read_one("etf:BTC_US:bgeometrics_flow_btc", Some("bgeometrics")),
        &btc_px,
    );
    let btc_oi = read_one("deriv:BTC:bgeometrics_oi_usd", Some("bgeometrics"));

    let spy_wk_ret = weekly_candle_returns(&spy_px);
    let btc_wk_ret = weekly_candle_returns(&btc_px);
    let bil_wk = weekly_sum_flows(&bil_fl);
    let jnk_wk = weekly_sum_flows(&jnk_fl);
    let gld_wk = weekly_sum_flows(&gld_fl);
    let btc_etf_wk = weekly_sum_obs(&btc_etf_usd);
    let btc_oi_wk = weekly_stock_deltas(&btc_oi);

    let mut all_weeks: BTreeSet<NaiveDate> = BTreeSet::new();
    all_weeks.extend(spy_wk_ret.keys().copied());
    all_weeks.extend(btc_wk_ret.keys().copied());
    all_weeks.extend(bil_wk.keys().copied());
    all_weeks.extend(gld_wk.keys().copied());

    let weekly_rows: Vec<ProspectiveWeeklyObs> = all_weeks
        .into_iter()
        .map(|w| ProspectiveWeeklyObs {
            week_friday: w.format("%Y-%m-%d").to_string(),
            spy_weekly_return_pct: spy_wk_ret.get(&w).copied(),
            btc_weekly_return_pct: btc_wk_ret.get(&w).copied(),
            btc_etf_flow_usd: btc_etf_wk.get(&w).copied(),
            delta_btc_oi_usd: btc_oi_wk.get(&w).copied(),
            bil_exact_flow_usd: bil_wk.get(&w).copied(),
            jnk_exact_flow_usd: jnk_wk.get(&w).copied(),
            gld_exact_flow_usd: gld_wk.get(&w).copied(),
        })
        .collect();

    // Daily OOS aligned ^MOVE vs BIL count & Pearson correlation (> 2026-11-27)
    let burn_end = NaiveDate::from_ymd_opt(2026, 11, 27).unwrap();
    let move_by_d: BTreeMap<NaiveDate, f64> = move_px
        .iter()
        .filter(|c| c.timestamp_utc.date_naive() > burn_end && c.close.is_finite())
        .map(|c| (c.timestamp_utc.date_naive(), c.close))
        .collect();
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for p in &bil_fl {
        let d = p.timestamp_utc.date_naive();
        if d > burn_end && p.implied_net_flow_usd.is_finite() {
            if let Some(&mv) = move_by_d.get(&d) {
                xs.push(mv);
                ys.push(p.implied_net_flow_usd);
            }
        }
    }
    let daily_corr = pearson_corr(&xs, &ys);

    let (sp_z, sp_ba) = if include_spillover {
        let spy_fl = compute_exact_share_flows(
            &read_one("etf:SPY:aum_usd", Some("ssga_xlsx")),
            &read_one("etf:SPY:shares_outstanding", Some("ssga_xlsx")),
            "SPY",
            "equities",
        );
        let sptl_fl = compute_exact_share_flows(
            &read_one("etf:SPTL:aum_usd", Some("ssga_xlsx")),
            &read_one("etf:SPTL:shares_outstanding", Some("ssga_xlsx")),
            "SPTL",
            "fiat_park",
        );
        let btc_onchain = read_one("onchain:BTC:coinmetrics_netflow_usd", Some("coinmetrics_community"));
        let wrmfns = read_one("macro:US:FRED_WRMFNS_USD", Some("fred_api"));
        let stbl = read_one("stablecoin:ALL:mcap_usd", Some("defillama"));
        let bank_cred = read_one("macro:US:FRED_TOTBKCR_USD", Some("fred_api"));
        let walcl = read_one("macro:US:FRED_WALCL_USD", Some("fred_api"));
        let tga = read_one("macro:US:TGA_BALANCE_USD", Some("treasury_fiscaldata"));
        let rrp = read_one("macro:US:FRED_RRPONTSYD_USD", Some("fred_api"));
        let nl = compute_fed_net_liquidity(&walcl, &tga, &rrp);
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
                fed_net_liq_stock: &nl,
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
            Ok(rep) => (Some(rep.tci_z_score), Some(rep.bias_adjusted_tci_pct)),
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };

    ProspectiveScorecardInputs {
        as_of_date: as_of,
        weekly_rows,
        daily_move_bil_oos_days: xs.len(),
        daily_move_bil_running_corr: daily_corr,
        latest_ex_rrp_tci_z: sp_z,
        latest_ex_rrp_bias_adj_pct: sp_ba,
    }
}

fn weekly_candle_returns(cs: &[AggregatorCandle]) -> BTreeMap<NaiveDate, f64> {
    let mut eow: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for c in cs {
        if c.close.is_finite() && c.close > 0.0 {
            eow.insert(align_to_friday_date(c.timestamp_utc.date_naive()), c.close);
        }
    }
    let mut out = BTreeMap::new();
    let mut prev: Option<f64> = None;
    for (wk, close) in eow {
        if let Some(p) = prev {
            out.insert(wk, ((close / p) - 1.0) * 100.0);
        }
        prev = Some(close);
    }
    out
}

fn weekly_sum_flows(pts: &[crate::features::analytics::ImpliedNetFlowPoint]) -> BTreeMap<NaiveDate, f64> {
    let mut out: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for p in pts {
        if p.implied_net_flow_usd.is_finite() {
            *out.entry(align_to_friday_date(p.timestamp_utc.date_naive())).or_insert(0.0) += p.implied_net_flow_usd;
        }
    }
    out
}

fn weekly_sum_obs(obs: &[MetricObservation]) -> BTreeMap<NaiveDate, f64> {
    let mut out: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for o in obs {
        if o.value.is_finite() {
            *out.entry(align_to_friday_date(o.observation_ts_utc.date_naive())).or_insert(0.0) += o.value;
        }
    }
    out
}

fn weekly_stock_deltas(obs: &[MetricObservation]) -> BTreeMap<NaiveDate, f64> {
    let mut eow: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for o in obs {
        if o.value.is_finite() {
            eow.insert(align_to_friday_date(o.observation_ts_utc.date_naive()), o.value);
        }
    }
    let mut out = BTreeMap::new();
    let mut prev: Option<f64> = None;
    for (wk, val) in eow {
        if let Some(p) = prev {
            out.insert(wk, val - p);
        }
        prev = Some(val);
    }
    out
}

fn pearson_corr(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() < 10 || xs.len() != ys.len() {
        return None;
    }
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut dx2 = 0.0;
    let mut dy2 = 0.0;
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        let dx = x - mx;
        let dy = y - my;
        num += dx * dy;
        dx2 += dx * dx;
        dy2 += dy * dy;
    }
    let denom = (dx2 * dy2).sqrt();
    (denom > 1e-12).then(|| num / denom)
}
