//! Partition path calculation for Hive-style columnar storage.

use std::path::PathBuf;
use chrono::{DateTime, Datelike, Utc};
use crate::core::types::{AssetClass, TimeFrame};

#[derive(Debug, Clone)]
pub struct PartitionResolver {
    base_lake_dir: PathBuf,
}

impl PartitionResolver {
    pub fn new(base_lake_dir: PathBuf) -> Self {
        Self { base_lake_dir }
    }

    pub fn base_lake_dir(&self) -> &PathBuf {
        &self.base_lake_dir
    }

    /// Resolves: base/market/asset_class=<ac>/symbol=<sym>/timeframe=<tf>/year=<YYYY>/month=<MM>
    pub fn resolve_candle_partition(
        &self,
        asset_class: AssetClass,
        canonical_symbol: &str,
        timeframe: TimeFrame,
        timestamp_utc: DateTime<Utc>,
    ) -> PathBuf {
        let clean_symbol = canonical_symbol
            .to_uppercase()
            .replace('/', "_")
            .replace(':', "_");
        let year_str = format!("{:04}", timestamp_utc.year());
        let month_str = format!("{:02}", timestamp_utc.month());

        self.base_lake_dir
            .join("market")
            .join(format!("asset_class={}", asset_class.as_str()))
            .join(format!("symbol={}", clean_symbol))
            .join(format!("timeframe={}", timeframe.as_str()))
            .join(format!("year={}", year_str))
            .join(format!("month={}", month_str))
    }

    /// Resolves glob pattern for market candles.
    pub fn resolve_glob_pattern(
        &self,
        asset_class: AssetClass,
        canonical_symbol: &str,
        timeframe: TimeFrame,
        year: &str,
        month: &str,
    ) -> String {
        let clean_symbol = canonical_symbol
            .to_uppercase()
            .replace('/', "_")
            .replace(':', "_");
        let target = self
            .base_lake_dir
            .join("market")
            .join(format!("asset_class={}", asset_class.as_str()))
            .join(format!("symbol={}", clean_symbol))
            .join(format!("timeframe={}", timeframe.as_str()))
            .join(format!("year={}", year))
            .join(format!("month={}", month));
        format!("{}/chunk_*.parquet", target.display())
    }

    /// Resolves: base/macro/calendar/country=<CC>/year=<YYYY>/month=<MM>
    pub fn resolve_macro_partition(&self, country: &str, year: i32, month: u32) -> PathBuf {
        let clean_country = country.to_uppercase().replace(['/', ':', ' '], "_");
        self.base_lake_dir
            .join("macro")
            .join("calendar")
            .join(format!("country={}", clean_country))
            .join(format!("year={:04}", year))
            .join(format!("month={:02}", month))
    }

    /// Resolves: base/prediction_markets/contracts/venue=<venue>/year=<YYYY>
    pub fn resolve_prediction_contracts_partition(&self, venue: &str, year: i32) -> PathBuf {
        let clean_venue = venue.to_lowercase().replace(['/', ':', ' '], "_");
        self.base_lake_dir
            .join("prediction_markets")
            .join("contracts")
            .join(format!("venue={}", clean_venue))
            .join(format!("year={:04}", year))
    }

    /// Resolves: base/prediction_markets/candles/venue=<venue>/market=<id>/timeframe=<tf>/year=<YYYY>/month=<MM>
    pub fn resolve_prediction_candles_partition(
        &self,
        venue: &str,
        market_id: &str,
        timeframe: &str,
        year: i32,
        month: u32,
    ) -> PathBuf {
        let clean_venue = venue.to_lowercase().replace(['/', ':', ' '], "_");
        let clean_market = market_id.to_uppercase().replace(['/', ':', ' '], "_");
        self.base_lake_dir
            .join("prediction_markets")
            .join("candles")
            .join(format!("venue={}", clean_venue))
            .join(format!("market={}", clean_market))
            .join(format!("timeframe={}", timeframe.to_lowercase()))
            .join(format!("year={:04}", year))
            .join(format!("month={:02}", month))
    }
}
