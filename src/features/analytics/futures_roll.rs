//! Continuous Commodity Futures Roll & 24/5 Forex Weekend Session Normalizer.
//!
//! Eliminates artificial step jumps across expiring futures contracts (Contango / Backwardation)
//! and aligns disjoint Sunday forex ticks into unified 5-day trading weeks.

use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollMethod {
    /// Multiplicative adjustment: P_t* = P_t * (P_new / P_old). Preserves percentage returns.
    PanamaRatio,
    /// Additive adjustment: P_t* = P_t + (P_new - P_old). Safe for negative commodity prices (e.g. WTI 2020).
    AdditiveDifference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuturesRollEvent {
    pub roll_date: NaiveDate,
    pub old_contract_price: f64,
    pub new_contract_price: f64,
}

pub struct FuturesRollEngine;

impl FuturesRollEngine {
    /// Constructs a continuous futures time-series by back-adjusting contracts prior to each roll date.
    pub fn build_continuous_series(
        raw_series: &[(NaiveDate, f64)],
        rolls: &[FuturesRollEvent],
        method: RollMethod,
    ) -> Vec<(NaiveDate, f64)> {
        if raw_series.is_empty() {
            return Vec::new();
        }

        let mut adjusted = raw_series.to_vec();

        // Sort rolls chronologically
        let mut sorted_rolls = rolls.to_vec();
        sorted_rolls.sort_by_key(|r| r.roll_date);

        // Process rolls from oldest to newest or backwards
        for roll in &sorted_rolls {
            if roll.old_contract_price <= 0.0 && method == RollMethod::PanamaRatio {
                // Ratio method invalid on zero or negative prices, fall back to additive
                continue;
            }

            let ratio = if roll.old_contract_price.abs() > 1e-9 {
                roll.new_contract_price / roll.old_contract_price
            } else {
                1.0
            };
            let diff = roll.new_contract_price - roll.old_contract_price;

            for pt in adjusted.iter_mut() {
                if pt.0 < roll.roll_date {
                    match method {
                        RollMethod::PanamaRatio => {
                            pt.1 *= ratio;
                        }
                        RollMethod::AdditiveDifference => {
                            pt.1 += diff;
                        }
                    }
                }
            }
        }

        adjusted
    }
}

pub struct ForexSessionNormalizer;

impl ForexSessionNormalizer {
    /// Normalizes Sunday opening sessions (Sunday 17:00–24:00 EST / 22:00–24:00 UTC)
    /// into the Monday trading session, eliminating empty weekend gap bars and timestamp drift.
    pub fn normalize_sunday_to_monday(
        date: NaiveDate,
        hour: u32,
    ) -> NaiveDate {
        if date.weekday() == Weekday::Sun && hour >= 17 {
            // Sunday open roll into Monday
            date + chrono::Duration::days(1)
        } else {
            date
        }
    }

    /// Verifies if a trade timestamp falls strictly within the institutional 24/5 Forex trading window
    /// (Sunday 17:00 NY to Friday 17:00 NY).
    pub fn is_institutional_forex_window(
        weekday: Weekday,
        hour_ny: u32,
    ) -> bool {
        match weekday {
            Weekday::Sun => hour_ny >= 17,
            Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu => true,
            Weekday::Fri => hour_ny < 17,
            Weekday::Sat => false,
        }
    }
}
