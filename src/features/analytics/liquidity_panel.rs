//! Synchronized Weekly Closed-System Liquidity Flow Panel Builder (Phase 341).
//! Aligns Stage-1 Implied Net Flows & Weekly Stock First-Differences (Delta USD) on Friday EOW grid.

use std::collections::BTreeMap;
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::core::observation_types::MetricObservation;
use crate::features::analytics::liquidity_flows::{FedNetLiquidityPoint, ImpliedNetFlowPoint};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyFlowRow {
    pub week_friday: String,
    pub equities_spy_flow_usd: Option<f64>,
    pub gold_gld_flow_usd: Option<f64>,
    pub fiat_park_bil_flow_usd: Option<f64>,
    pub fiat_park_sptl_flow_usd: Option<f64>,
    pub credit_jnk_flow_usd: Option<f64>,
    pub crypto_btc_etf_flow_usd: Option<f64>,
    pub crypto_onchain_netflow_usd: Option<f64>,
    pub delta_retail_mmf_usd: Option<f64>,
    pub delta_stablecoin_mcap_usd: Option<f64>,
    pub delta_bank_credit_usd: Option<f64>,
    pub delta_fed_net_liquidity_usd: Option<f64>,
    pub delta_fed_walcl_minus_tga_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyFlowPanel {
    pub total_weeks: usize,
    pub start_week: Option<String>,
    pub end_week: Option<String>,
    pub rows: Vec<WeeklyFlowRow>,
}

pub struct WeeklyPanelInputs<'a> {
    pub spy_flows: &'a [ImpliedNetFlowPoint],
    pub gld_flows: &'a [ImpliedNetFlowPoint],
    pub bil_flows: &'a [ImpliedNetFlowPoint],
    pub jnk_flows: &'a [ImpliedNetFlowPoint],
    pub sptl_flows: &'a [ImpliedNetFlowPoint],
    pub btc_etf_usd_flows: &'a [MetricObservation],
    pub btc_onchain_flows: &'a [MetricObservation],
    pub retail_mmf_stock: &'a [MetricObservation],
    pub stablecoin_stock: &'a [MetricObservation],
    pub bank_credit_stock: &'a [MetricObservation],
    pub fed_net_liq_stock: &'a [FedNetLiquidityPoint],
}

pub fn build_weekly_flow_panel(
    inputs: &WeeklyPanelInputs<'_>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> WeeklyFlowPanel {
    let spy = sum_implied_to_week(inputs.spy_flows);
    let gld = sum_implied_to_week(inputs.gld_flows);
    let bil = sum_implied_to_week(inputs.bil_flows);
    let jnk = sum_implied_to_week(inputs.jnk_flows);
    let sptl = sum_implied_to_week(inputs.sptl_flows);
    let btc_etf = sum_obs_to_week(inputs.btc_etf_usd_flows);
    let btc_onchain = sum_obs_to_week(inputs.btc_onchain_flows);

    let d_mmf = diff_obs_weekly_stock(inputs.retail_mmf_stock);
    let d_stable = diff_obs_weekly_stock(inputs.stablecoin_stock);
    let d_credit = diff_obs_weekly_stock(inputs.bank_credit_stock);
    let d_nl = diff_fed_nl_weekly(inputs.fed_net_liq_stock, false);
    let d_nl_ex_rrp = diff_fed_nl_weekly(inputs.fed_net_liq_stock, true);

    let mut weeks: BTreeMap<NaiveDate, ()> = BTreeMap::new();
    for k in spy
        .keys()
        .chain(gld.keys())
        .chain(bil.keys())
        .chain(jnk.keys())
        .chain(sptl.keys())
        .chain(btc_etf.keys())
        .chain(btc_onchain.keys())
        .chain(d_mmf.keys())
        .chain(d_stable.keys())
        .chain(d_credit.keys())
        .chain(d_nl.keys())
    {
        if start_date.is_some_and(|s| *k < s) || end_date.is_some_and(|e| *k > e) {
            continue;
        }
        weeks.insert(*k, ());
    }

    let rows: Vec<WeeklyFlowRow> = weeks
        .into_keys()
        .map(|w| WeeklyFlowRow {
            week_friday: w.format("%Y-%m-%d").to_string(),
            equities_spy_flow_usd: spy.get(&w).copied(),
            gold_gld_flow_usd: gld.get(&w).copied(),
            fiat_park_bil_flow_usd: bil.get(&w).copied(),
            fiat_park_sptl_flow_usd: sptl.get(&w).copied(),
            credit_jnk_flow_usd: jnk.get(&w).copied(),
            crypto_btc_etf_flow_usd: btc_etf.get(&w).copied(),
            crypto_onchain_netflow_usd: btc_onchain.get(&w).copied(),
            delta_retail_mmf_usd: d_mmf.get(&w).copied(),
            delta_stablecoin_mcap_usd: d_stable.get(&w).copied(),
            delta_bank_credit_usd: d_credit.get(&w).copied(),
            delta_fed_net_liquidity_usd: d_nl.get(&w).copied(),
            delta_fed_walcl_minus_tga_usd: d_nl_ex_rrp.get(&w).copied(),
        })
        .collect();

    WeeklyFlowPanel {
        total_weeks: rows.len(),
        start_week: rows.first().map(|r| r.week_friday.clone()),
        end_week: rows.last().map(|r| r.week_friday.clone()),
        rows,
    }
}

pub fn align_to_friday_date(d: NaiveDate) -> NaiveDate {
    let days_from_mon = d.weekday().num_days_from_monday() as i64;
    let fri_offset = 4 - days_from_mon;
    if fri_offset >= 0 {
        d + Duration::days(fri_offset)
    } else {
        d + Duration::days(7 + fri_offset)
    }
}

fn sum_implied_to_week(pts: &[ImpliedNetFlowPoint]) -> BTreeMap<NaiveDate, f64> {
    let mut map = BTreeMap::new();
    for p in pts {
        if p.implied_net_flow_usd.is_finite() {
            let w = align_to_friday_date(p.timestamp_utc.date_naive());
            *map.entry(w).or_insert(0.0) += p.implied_net_flow_usd;
        }
    }
    map
}

fn sum_obs_to_week(obs: &[MetricObservation]) -> BTreeMap<NaiveDate, f64> {
    let mut map = BTreeMap::new();
    for o in obs {
        if o.value.is_finite() {
            let w = align_to_friday_date(o.observation_ts_utc.date_naive());
            *map.entry(w).or_insert(0.0) += o.value;
        }
    }
    map
}

fn diff_obs_weekly_stock(obs: &[MetricObservation]) -> BTreeMap<NaiveDate, f64> {
    let mut last_in_week: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for o in obs {
        if o.value.is_finite() {
            let w = align_to_friday_date(o.observation_ts_utc.date_naive());
            last_in_week.insert(w, o.value);
        }
    }
    let pairs: Vec<(NaiveDate, f64)> = last_in_week.into_iter().collect();
    let mut out = BTreeMap::new();
    for win in pairs.windows(2) {
        out.insert(win[1].0, win[1].1 - win[0].1);
    }
    out
}

fn diff_fed_nl_weekly(pts: &[FedNetLiquidityPoint], exclude_rrp: bool) -> BTreeMap<NaiveDate, f64> {
    let mut last_in_week: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for p in pts {
        let val = if exclude_rrp {
            p.fed_assets_usd - p.tga_balance_usd
        } else {
            p.net_liquidity_usd
        };
        if val.is_finite() {
            let w = align_to_friday_date(p.timestamp_utc.date_naive());
            last_in_week.insert(w, val);
        }
    }
    let pairs: Vec<(NaiveDate, f64)> = last_in_week.into_iter().collect();
    let mut out = BTreeMap::new();
    for win in pairs.windows(2) {
        out.insert(win[1].0, win[1].1 - win[0].1);
    }
    out
}
