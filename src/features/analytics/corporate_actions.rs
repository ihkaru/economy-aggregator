//! Corporate Action Adjustment Engine for Equities & ETFs.
//!
//! Eliminates artificial price drops from stock splits and cash dividends,
//! producing institutional Total Return Index (TRI) series without look-ahead distortion.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub use crate::core::types::{DividendEvent, SplitEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustedPricePoint {
    pub date: NaiveDate,
    pub raw_close: f64,
    pub adjusted_close: f64,
    pub cumulative_split_factor: f64,
    pub cumulative_dividend_factor: f64,
    pub total_adjustment_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawBar {
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdjustedBar {
    pub date: NaiveDate,
    pub raw: RawBar,
    pub adjusted_open: f64,
    pub adjusted_high: f64,
    pub adjusted_low: f64,
    pub adjusted_close: f64,
    pub adjusted_volume: f64,
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

    /// Applies backward adjustment to full OHLCV candlestick bars based on splits and dividends.
    ///
    /// Prices are scaled by total factor (split * dividend), while historical volumes are scaled
    /// inversely by the split factor (volume / cum_split), maintaining share equivalence across splits.
    pub fn adjust_ohlcv_backward(
        raw_bars: &[RawBar],
        splits: &[SplitEvent],
        dividends: &[DividendEvent],
    ) -> Vec<AdjustedBar> {
        if raw_bars.is_empty() {
            return Vec::new();
        }

        let n = raw_bars.len();
        let mut cum_split = 1.0;
        let mut cum_div = 1.0;
        let mut result = Vec::with_capacity(n);

        for i in (0..n).rev() {
            let bar = &raw_bars[i];
            let total_factor = cum_split * cum_div;

            let adj_vol = if cum_split > 0.0 {
                bar.volume / cum_split
            } else {
                bar.volume
            };

            result.push(AdjustedBar {
                date: bar.date,
                raw: bar.clone(),
                adjusted_open: bar.open * total_factor,
                adjusted_high: bar.high * total_factor,
                adjusted_low: bar.low * total_factor,
                adjusted_close: bar.close * total_factor,
                adjusted_volume: adj_vol,
                cumulative_split_factor: cum_split,
                cumulative_dividend_factor: cum_div,
                total_adjustment_factor: total_factor,
            });

            for split in splits {
                if split.date == bar.date {
                    let ratio = split.ratio();
                    if ratio > 0.0 {
                        cum_split *= 1.0 / ratio;
                    }
                }
            }

            for div in dividends {
                if div.ex_date == bar.date && i > 0 {
                    let prev_close = raw_bars[i - 1].close;
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
