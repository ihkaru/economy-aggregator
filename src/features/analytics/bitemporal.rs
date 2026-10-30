//! Point-in-Time (PIT) Bitemporal Engine for Macroeconomic Vintages.
//!
//! Eliminates Look-Ahead Bias by enforcing strict as-of temporal filtering on revised indicators
//! (GDP, Non-Farm Payrolls, CPI, Inflation prints).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VintageStage {
    Preliminary,
    SecondEstimate,
    FinalRevised,
    AnnualBenchmark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroVintageRecord {
    pub indicator_code: String,
    pub country: String,
    pub reference_period: String, // e.g. "2024-Q1", "2024-03"
    pub release_timestamp_utc: DateTime<Utc>,
    pub reported_value: f64,
    pub vintage_stage: VintageStage,
    pub revision_number: u32,
}

pub struct PointInTimeEngine;

impl PointInTimeEngine {
    /// Queries the latest known value for an indicator and reference period strictly as of `simulated_time`.
    ///
    /// Guarantees that any revision published *after* `simulated_time` is invisible to the caller,
    /// completely preventing Look-Ahead Bias in quantitative models and backtests.
    pub fn query_as_of<'a>(
        records: &'a [MacroVintageRecord],
        indicator_code: &str,
        reference_period: &str,
        simulated_time: DateTime<Utc>,
    ) -> Option<&'a MacroVintageRecord> {
        records
            .iter()
            .filter(|r| {
                r.indicator_code == indicator_code
                    && r.reference_period == reference_period
                    && r.release_timestamp_utc <= simulated_time
            })
            .max_by_key(|r| r.release_timestamp_utc)
    }

    /// Evaluates the revision delta between the first preliminary release and the final known vintage.
    ///
    /// Returns: (initial_release, final_release, absolute_revision, pct_revision)
    pub fn compute_revision_spread(
        records: &[MacroVintageRecord],
        indicator_code: &str,
        reference_period: &str,
    ) -> Option<(f64, f64, f64, f64)> {
        let matching: Vec<&MacroVintageRecord> = records
            .iter()
            .filter(|r| r.indicator_code == indicator_code && r.reference_period == reference_period)
            .collect();

        if matching.is_empty() {
            return None;
        }

        let first = matching.iter().min_by_key(|r| r.release_timestamp_utc)?;
        let last = matching.iter().max_by_key(|r| r.release_timestamp_utc)?;

        let initial_val = first.reported_value;
        let final_val = last.reported_value;
        let delta = final_val - initial_val;
        let pct_delta = if initial_val.abs() > 1e-9 {
            (delta / initial_val.abs()) * 100.0
        } else {
            0.0
        };

        Some((initial_val, final_val, delta, pct_delta))
    }
}
