//! Currency Normalization Engine for cross-asset multi-currency alignment.
//!
//! Converts foreign currency denominated asset series (USD, JPY) into base currency (IDR)
//! using historical FX exchange rates with forward-fill interpolation.

use std::collections::HashMap;
use chrono::NaiveDate;

pub struct FxNormalizer;

impl FxNormalizer {
    /// Converts a foreign price series to base currency by multiplying by FX rate:
    /// e.g. Price_IDR = Price_USD * (USD/IDR)
    pub fn normalize_by_multiplication(
        asset_series: &[(NaiveDate, f64)],
        fx_series: &[(NaiveDate, f64)],
    ) -> Vec<(NaiveDate, f64)> {
        if asset_series.is_empty() || fx_series.is_empty() {
            return Vec::new();
        }

        let mut fx_map = HashMap::with_capacity(fx_series.len());
        for (d, r) in fx_series {
            fx_map.insert(*d, *r);
        }

        let mut last_known_fx = fx_series.first().unwrap().1;
        let mut normalized = Vec::with_capacity(asset_series.len());

        for (d, price) in asset_series {
            if let Some(fx) = fx_map.get(d) {
                last_known_fx = *fx;
            }
            let base_price = price * last_known_fx;
            normalized.push((*d, base_price));
        }

        normalized
    }

    /// Converts a price series by dividing by FX rate:
    /// e.g. Price_USD = Price_IDR / (USD/IDR)
    pub fn normalize_by_division(
        asset_series: &[(NaiveDate, f64)],
        fx_series: &[(NaiveDate, f64)],
    ) -> Vec<(NaiveDate, f64)> {
        if asset_series.is_empty() || fx_series.is_empty() {
            return Vec::new();
        }

        let mut fx_map = HashMap::with_capacity(fx_series.len());
        for (d, r) in fx_series {
            fx_map.insert(*d, *r);
        }

        let mut last_known_fx = fx_series.first().unwrap().1;
        let mut normalized = Vec::with_capacity(asset_series.len());

        for (d, price) in asset_series {
            if let Some(fx) = fx_map.get(d) {
                last_known_fx = *fx;
            }
            let base_price = if last_known_fx > 0.0 { price / last_known_fx } else { 0.0 };
            normalized.push((*d, base_price));
        }

        normalized
    }
}
