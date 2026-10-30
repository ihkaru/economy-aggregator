//! Partition path calculation for Hive-style columnar storage.

use std::path::{Path, PathBuf};
use chrono::{DateTime, Datelike, Utc};
use crate::core::types::{AssetClass, TimeFrame};

pub struct PartitionResolver {
    base_lake_dir: PathBuf,
}

impl PartitionResolver {
    pub fn new(base_lake_dir: PathBuf) -> Self {
        Self { base_lake_dir }
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
}
