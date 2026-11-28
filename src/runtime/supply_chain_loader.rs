//! Runtime Loader for the Persistent Supply-Chain Graph & Lead-Lag Analysis.

use std::path::Path;
use chrono::NaiveDate;

use serde::{Deserialize, Serialize};
use crate::core::supply_chain::SupplyChainManager;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    evaluate_supply_chain_lead_lag, SupplyChainAnalysisResponse, TradeSignal,
};
use crate::features::storage::LakeReader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainLeaderboardRow {
    pub target: String,
    pub latest_price: f64,
    pub signal: TradeSignal,
    pub current_position: f64,
    pub forecast_ret_pct: f64,
    pub strategy_sortino: f64,
    pub buy_hold_sortino: f64,
    pub strategy_max_dd_pct: f64,
    pub oos_r2_pct: f64,
    pub total_trades: usize,
    pub last_event_date: String,
    pub last_event_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainLeaderboardResponse {
    pub horizon_days: usize,
    pub no_pyramiding_enforced: bool,
    pub top5_by_sortino: Vec<String>,
    pub rows: Vec<SupplyChainLeaderboardRow>,
}

pub fn build_supply_chain_leaderboard(
    lake_dir: &Path,
    as_of: NaiveDate,
    horizon_days: usize,
) -> SupplyChainLeaderboardResponse {
    let mgr = SupplyChainManager::new(lake_dir);
    let targets = mgr
        .load_or_seed()
        .map(|g| g.tech20_targets)
        .unwrap_or_default();

    let mut rows = Vec::new();
    for t in targets {
        if let Some(res) = build_supply_chain_analysis(lake_dir, &t, as_of, horizon_days, 365) {
            let (last_date, last_act) = res
                .recent_signals
                .first()
                .map(|e| (e.time.clone(), e.action.clone()))
                .unwrap_or_else(|| (res.as_of.clone(), "NONE".to_string()));
            rows.push(SupplyChainLeaderboardRow {
                target: res.target,
                latest_price: res.latest_price,
                signal: res.signal,
                current_position: res.current_position,
                forecast_ret_pct: res.forecast_ret_pct,
                strategy_sortino: res.strategy_sortino,
                buy_hold_sortino: res.buy_hold_sortino,
                strategy_max_dd_pct: res.strategy_max_dd_pct,
                oos_r2_pct: res.oos_r2_pct,
                total_trades: res.total_trades,
                last_event_date: last_date,
                last_event_action: last_act,
            });
        }
    }
    rows.sort_by(|a, b| {
        b.strategy_sortino
            .partial_cmp(&a.strategy_sortino)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top5_by_sortino = rows.iter().take(5).map(|r| r.target.clone()).collect();
    SupplyChainLeaderboardResponse {
        horizon_days,
        no_pyramiding_enforced: true,
        top5_by_sortino,
        rows,
    }
}

pub fn build_supply_chain_analysis(
    lake_dir: &Path,
    target: &str,
    as_of: NaiveDate,
    horizon_days: usize,
    range_days: usize,
) -> Option<SupplyChainAnalysisResponse> {
    let tgt = target.trim().to_ascii_uppercase();
    let mgr = SupplyChainManager::new(lake_dir);
    let suppliers = mgr.suppliers_for(&tgt);

    let read_px = |sym: &str| -> Vec<AggregatorCandle> {
        let mut cs = LakeReader::read_candles(lake_dir, Some("equities"), sym, "1d", None, None)
            .unwrap_or_default();
        cs.retain(|c| c.timestamp_utc.date_naive() <= as_of);
        cs.sort_by_key(|c| c.timestamp_utc);
        cs
    };

    let target_candles = read_px(&tgt);
    if target_candles.is_empty() {
        return None;
    }
    let supplier_candles: Vec<(String, Vec<AggregatorCandle>)> = suppliers
        .into_iter()
        .filter(|s| !s.eq_ignore_ascii_case(&tgt))
        .map(|s| {
            let cs = read_px(&s);
            (s, cs)
        })
        .filter(|(_, cs)| !cs.is_empty())
        .collect();

    evaluate_supply_chain_lead_lag(
        &tgt,
        as_of,
        horizon_days,
        range_days,
        &target_candles,
        &supplier_candles,
    )
}
