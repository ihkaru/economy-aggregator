//! Multi-Exchange Trading Calendar Aligner with deterministic forward-fill.
//!
//! Solves calendar asynchrony across global exchanges (NYSE, IDX, TSE, Commodities, Crypto).

use std::collections::{BTreeSet, HashMap};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignedPanel {
    pub dates: Vec<NaiveDate>,
    pub symbols: Vec<String>,
    /// Matrix of shape [dates.len()][symbols.len()]
    pub prices: Vec<Vec<f64>>,
    /// Matrix of shape [dates.len() - 1][symbols.len()]
    pub returns: Vec<Vec<f64>>,
}

impl AlignedPanel {
    pub fn row_count(&self) -> usize {
        self.dates.len()
    }

    pub fn symbol_index(&self, symbol: &str) -> Option<usize> {
        self.symbols.iter().position(|s| s == symbol)
    }

    pub fn symbol_prices(&self, symbol: &str) -> Option<Vec<f64>> {
        let idx = self.symbol_index(symbol)?;
        Some(self.prices.iter().map(|row| row[idx]).collect())
    }

    pub fn symbol_returns(&self, symbol: &str) -> Option<Vec<f64>> {
        let idx = self.symbol_index(symbol)?;
        Some(self.returns.iter().map(|row| row[idx]).collect())
    }
}

pub struct TradingCalendarAligner;

impl TradingCalendarAligner {
    /// Aligns multiple time-series into a synchronized panel using forward-fill for missing dates.
    ///
    /// If `common_range` is true, the panel will start at the latest start date among all symbols.
    pub fn align(
        series_map: &HashMap<String, Vec<(NaiveDate, f64)>>,
        common_range: bool,
    ) -> Result<AlignedPanel, String> {
        if series_map.is_empty() {
            return Err("Cannot align empty series map".to_string());
        }

        let mut symbols: Vec<String> = series_map.keys().cloned().collect();
        symbols.sort();

        // 1. Collect all distinct dates and find bounds per symbol
        let mut all_dates_set = BTreeSet::new();
        let mut start_dates = Vec::new();
        let mut end_dates = Vec::new();

        for (sym, pts) in series_map {
            if pts.is_empty() {
                return Err(format!("Symbol {} has empty observations", sym));
            }
            let first = pts.first().unwrap().0;
            let last = pts.last().unwrap().0;
            start_dates.push(first);
            end_dates.push(last);

            for (d, _) in pts {
                all_dates_set.insert(*d);
            }
        }

        let common_start = *start_dates.iter().max().unwrap();
        let common_end = *end_dates.iter().min().unwrap();

        let dates: Vec<NaiveDate> = all_dates_set
            .into_iter()
            .filter(|d| !common_range || (*d >= common_start && *d <= common_end))
            .collect();

        if dates.len() < 2 {
            return Err("Insufficient aligned date points (< 2)".to_string());
        }

        // 2. Build forward-filled price matrix
        let mut prices: Vec<Vec<f64>> = Vec::with_capacity(dates.len());
        let mut lookup_maps: Vec<HashMap<NaiveDate, f64>> = Vec::with_capacity(symbols.len());
        let mut last_known_prices: Vec<f64> = Vec::with_capacity(symbols.len());

        for sym in &symbols {
            let pts = &series_map[sym];
            let mut map = HashMap::with_capacity(pts.len());
            for (d, p) in pts {
                map.insert(*d, *p);
            }
            lookup_maps.push(map);
            // Default initial price is first point
            last_known_prices.push(pts.first().unwrap().1);
        }

        for d in &dates {
            let mut row = Vec::with_capacity(symbols.len());
            for (col_idx, map) in lookup_maps.iter().enumerate() {
                if let Some(val) = map.get(d) {
                    last_known_prices[col_idx] = *val;
                    row.push(*val);
                } else {
                    // Forward-fill with previous known price
                    row.push(last_known_prices[col_idx]);
                }
            }
            prices.push(row);
        }

        // 3. Compute daily returns: (p_t - p_{t-1}) / p_{t-1}
        let mut returns = Vec::with_capacity(dates.len() - 1);
        for t in 1..prices.len() {
            let mut ret_row = Vec::with_capacity(symbols.len());
            for s in 0..symbols.len() {
                let prev = prices[t - 1][s];
                let curr = prices[t][s];
                let ret = if prev > 0.0 { (curr - prev) / prev } else { 0.0 };
                ret_row.push(ret);
            }
            returns.push(ret_row);
        }

        Ok(AlignedPanel {
            dates,
            symbols,
            prices,
            returns,
        })
    }
}
