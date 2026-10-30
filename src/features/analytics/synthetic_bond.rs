//! Synthetic Bond & Coupon Accumulation Engine for Fixed Income Assets.
//!
//! Generates continuous Total Return Index (TRI) series from yield and coupon rates
//! for Indonesian Government Bonds (SBN), Corporate Bonds, and Money Market instruments.

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DayCountConvention {
    Actual365,
    Actual360,
    Bus252,
}

impl DayCountConvention {
    pub fn divisor(&self) -> f64 {
        match self {
            Self::Actual365 => 365.0,
            Self::Actual360 => 360.0,
            Self::Bus252 => 252.0,
        }
    }
}

pub struct SyntheticBondAccumulator;

impl SyntheticBondAccumulator {
    /// Generates a continuous daily compounding Total Return Index (TRI) from a constant annual yield.
    pub fn generate_constant_yield_tri(
        start_date: NaiveDate,
        end_date: NaiveDate,
        annual_yield: f64,
        base_value: f64,
        convention: DayCountConvention,
    ) -> Vec<(NaiveDate, f64)> {
        if start_date > end_date {
            return Vec::new();
        }

        let divisor = convention.divisor();
        // Geometric daily compounding factor: (1 + y)^(1 / divisor)
        let daily_factor = (1.0 + annual_yield).powf(1.0 / divisor);

        let mut current_date = start_date;
        let mut current_value = base_value;
        let mut series = Vec::new();

        while current_date <= end_date {
            series.push((current_date, current_value));
            current_value *= daily_factor;
            current_date += Duration::days(1);
        }

        series
    }

    /// Generates a TRI series from an evolving time-series of yield/interest rates.
    pub fn generate_variable_yield_tri(
        yield_series: &[(NaiveDate, f64)],
        base_value: f64,
        convention: DayCountConvention,
    ) -> Vec<(NaiveDate, f64)> {
        if yield_series.is_empty() {
            return Vec::new();
        }

        let divisor = convention.divisor();
        let mut series = Vec::with_capacity(yield_series.len());
        let mut current_value = base_value;

        series.push((yield_series[0].0, current_value));

        for i in 1..yield_series.len() {
            let prev_date = yield_series[i - 1].0;
            let curr_date = yield_series[i].0;
            let yield_rate = yield_series[i - 1].1;

            let days = (curr_date - prev_date).num_days() as f64;
            if days > 0.0 {
                let daily_factor = (1.0 + yield_rate).powf(days / divisor);
                current_value *= daily_factor;
            }

            series.push((curr_date, current_value));
        }

        series
    }
}
