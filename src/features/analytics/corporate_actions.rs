//! Corporate Action Adjustment Engine for Equities & ETFs.
//!
//! Eliminates artificial price drops from stock splits and cash dividends,
//! producing institutional Total Return Index (TRI) series without look-ahead distortion.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitEvent {
    pub date: NaiveDate,
    pub from_shares: f64,
    pub to_shares: f64,
}

impl SplitEvent {
    pub fn ratio(&self) -> f64 {
        if self.from_shares > 0.0 {
            self.to_shares / self.from_shares
        } else {
            1.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividendEvent {
    pub ex_date: NaiveDate,
    pub cash_amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustedPricePoint {
    pub date: NaiveDate,
    pub raw_close: f64,
    pub adjusted_close: f64,
    pub cumulative_split_factor: f64,
    pub cumulative_dividend_factor: f64,
    pub total_adjustment_factor: f64,
}

pub struct CorporateActionAdjuster;

impl CorporateActionAdjuster {
    /// Applies backward adjustment to a chronological price series based on splits and dividends.
    ///
    /// The most recent price remains unchanged (factor = 1.0), while historical prices are
    /// scaled down retrospectively so historical percentage returns match true investor performance.
    pub fn adjust_backward(
        raw_series: &[(NaiveDate, f64)],
        splits: &[SplitEvent],
        dividends: &[DividendEvent],
    ) -> Vec<AdjustedPricePoint> {
        if raw_series.is_empty() {
            return Vec::new();
        }

        let n = raw_series.len();
        let mut cum_split = 1.0;
        let mut cum_div = 1.0;
        let mut result = Vec::with_capacity(n);

        // Walk backwards from latest observation to oldest
        for i in (0..n).rev() {
            let (date, raw_close) = raw_series[i];
            let total_factor = cum_split * cum_div;

            result.push(AdjustedPricePoint {
                date,
                raw_close,
                adjusted_close: raw_close * total_factor,
                cumulative_split_factor: cum_split,
                cumulative_dividend_factor: cum_div,
                total_adjustment_factor: total_factor,
            });

            // Update cumulative factors for prices strictly preceding this event date
            for split in splits {
                if split.date == date {
                    let ratio = split.ratio();
                    if ratio > 0.0 {
                        cum_split *= 1.0 / ratio;
                    }
                }
            }

            for div in dividends {
                if div.ex_date == date && i > 0 {
                    let prev_close = raw_series[i - 1].1;
                    if prev_close > div.cash_amount && prev_close > 0.0 {
                        cum_div *= 1.0 - (div.cash_amount / prev_close);
                    }
                }
            }
        }

        result.reverse();
        result
    }

    /// Computes discrete return on split date to verify return continuity.
    pub fn is_return_continuous(
        prev_adj: f64,
        curr_adj: f64,
        max_allowed_delta_pct: f64,
    ) -> bool {
        if prev_adj <= 0.0 {
            return false;
        }
        let ret = ((curr_adj - prev_adj) / prev_adj).abs();
        ret <= max_allowed_delta_pct
    }
}
