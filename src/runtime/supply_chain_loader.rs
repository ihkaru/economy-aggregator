//! Runtime Loader for the Persistent Supply-Chain Graph & Lead-Lag Analysis.

use std::path::Path;
use chrono::NaiveDate;

use crate::core::supply_chain::SupplyChainManager;
use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    evaluate_supply_chain_lead_lag, SupplyChainAnalysisResponse,
};
use crate::features::storage::LakeReader;

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
