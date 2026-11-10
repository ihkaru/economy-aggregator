//! High-watermark timestamp detector and gap calculator for analytical Lakehouse partitions.

use std::path::Path;
use chrono::{DateTime, NaiveDate, Utc};
use tracing::debug;

use crate::features::storage::LakeReader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatermarkStatus {
    pub canonical_symbol: String,
    pub asset_class: String,
    pub timeframe: String,
    pub latest_timestamp_utc: Option<DateTime<Utc>>,
    pub latest_date: Option<NaiveDate>,
    pub total_rows: usize,
    pub has_gap: bool,
    pub gap_duration_days: i64,
}

pub struct WatermarkScanner;

impl WatermarkScanner {
    /// Scans the latest partition of a symbol to determine its high-watermark timestamp.
    pub fn scan_watermark(
        lake_dir: &Path,
        asset_class: &str,
        canonical_symbol: &str,
        timeframe: &str,
    ) -> WatermarkStatus {
        let bounds = LakeReader::read_metadata_bounds(
            lake_dir,
            Some(asset_class),
            canonical_symbol,
            timeframe,
        );

        let (total_rows, _first_date, last_date_str) = bounds.unwrap_or((0, None, None));
        let latest_date = last_date_str.and_then(|s: String| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        let latest_timestamp_utc = latest_date.and_then(|d: NaiveDate| {
            d.and_hms_opt(0, 0, 0).map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
        });

        let now = Utc::now();
        let (has_gap, gap_duration_days) = match latest_date {
            Some(d) => {
                let today = now.date_naive();
                let diff = (today - d).num_days();
                // 1D: Gap if > 1 day behind (or > 3 days on Monday to account for weekend)
                let gap = if timeframe == "1d" {
                    diff > 1
                } else {
                    diff >= 1
                };
                (gap, diff.max(0))
            }
            None => (true, 365), // No data exists -> 1 year initial catch-up needed
        };

        debug!(
            "Watermark scan for {}: latest={:?}, total_rows={}, has_gap={}, days={}",
            canonical_symbol, latest_date, total_rows, has_gap, gap_duration_days
        );

        WatermarkStatus {
            canonical_symbol: canonical_symbol.to_string(),
            asset_class: asset_class.to_string(),
            timeframe: timeframe.to_string(),
            latest_timestamp_utc,
            latest_date,
            total_rows,
            has_gap,
            gap_duration_days,
        }
    }
}
