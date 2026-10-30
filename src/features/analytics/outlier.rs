//! Statistical Outlier Detection using Rolling Median Absolute Deviation (Hampel Filter).
//!
//! Industry-standard, non-parametric cleaning for financial time series without hardcoded thresholds.

use chrono::NaiveDate;

pub struct HampelFilter;

impl HampelFilter {
    /// Cleans a time-series by filtering out points exceeding `n_sigmas * 1.4826 * MAD`
    /// relative to the rolling median within a sliding window of `2 * half_window + 1`.
    pub fn filter_outliers(
        series: &[(NaiveDate, f64)],
        half_window: usize,
        n_sigmas: f64,
    ) -> Vec<(NaiveDate, f64)> {
        if series.len() < half_window * 2 + 1 {
            return series.to_vec();
        }

        let mut cleaned = Vec::with_capacity(series.len());

        for i in 0..series.len() {
            let start = i.saturating_sub(half_window);
            let end = (i + half_window + 1).min(series.len());
            let mut window_vals: Vec<f64> = series[start..end].iter().map(|(_, p)| *p).collect();
            window_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let median = Self::median_sorted(&window_vals);

            let mut abs_devs: Vec<f64> = window_vals.iter().map(|v| (v - median).abs()).collect();
            abs_devs.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let mad = Self::median_sorted(&abs_devs);
            let val = series[i].1;
            let diff = (val - median).abs();

            let is_outlier = if mad > 1e-9 {
                // Standard Hampel criterion: diff > n * 1.4826 * MAD
                diff > n_sigmas * 1.4826 * mad
            } else if median > 1e-9 {
                // Fallback for uniform local clusters: deviation > 50% from local median
                (diff / median) > 0.50
            } else {
                false
            };

            if !is_outlier {
                cleaned.push(series[i]);
            }
        }

        cleaned
    }

    fn median_sorted(sorted: &[f64]) -> f64 {
        let n = sorted.len();
        if n == 0 {
            return 0.0;
        }
        if n % 2 == 1 {
            sorted[n / 2]
        } else {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        }
    }
}
