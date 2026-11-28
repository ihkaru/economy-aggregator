//! Runtime Loader for the SMH 5-Day Predictive & Risk Envelope Engine.
//! Reads historical 1d candles for SMH and its validated lag drivers from Parquet
//! with strict `<= as_of` zero-lookahead filtering.

use std::path::Path;
use chrono::NaiveDate;

use crate::core::types::AggregatorCandle;
use crate::features::analytics::{
    evaluate_smh_predictive, SmhPredictiveInputs, SmhPredictiveResponse,
};
use crate::features::storage::LakeReader;

pub fn build_smh_predictive(
    lake_dir: &Path,
    as_of: NaiveDate,
    range_days: usize,
) -> Option<SmhPredictiveResponse> {
    let read_px = |sym: &str| -> Vec<AggregatorCandle> {
        let mut cs = LakeReader::read_candles(lake_dir, Some("equities"), sym, "1d", None, None)
            .unwrap_or_default();
        cs.retain(|c| c.timestamp_utc.date_naive() <= as_of);
        cs.sort_by_key(|c| c.timestamp_utc);
        cs
    };

    let smh = read_px("SMH");
    if smh.is_empty() {
        return None;
    }
    let vxn = read_px("^VXN");
    let vix = read_px("^VIX");
    let ztf = read_px("ZT=F");
    let fvx = read_px("^FVX");
    let asml = read_px("ASML");
    let move_idx = read_px("^MOVE");

    evaluate_smh_predictive(&SmhPredictiveInputs {
        as_of,
        range_days,
        smh: &smh,
        vxn: &vxn,
        vix: &vix,
        ztf: &ztf,
        fvx: &fvx,
        asml: &asml,
        move_idx: &move_idx,
    })
}
