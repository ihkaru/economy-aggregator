//! Lake Reader: Reads, filters, and deduplicates historical candles from partitioned Parquet lake.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use chrono::{Datelike, NaiveDate};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::core::types::AggregatorCandle;
use crate::features::storage::arrow_schema::record_batch_to_candles;

pub struct LakeReader;

impl LakeReader {
    /// Reads candles from the partitioned Parquet lake for a given symbol and timeframe.
    /// Deduplicates settled records over provisional records and sorts chronologically.
    pub fn read_candles(
        lake_dir: &Path,
        asset_class: Option<&str>,
        symbol: &str,
        timeframe: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let clean_symbol = symbol.to_uppercase().replace('/', "_").replace(':', "_");
        let sym_dir = Self::resolve_symbol_dir(lake_dir, asset_class, &clean_symbol, timeframe)?;

        if !sym_dir.exists() {
            return Ok(Vec::new());
        }

        let mut parquet_files = Vec::new();
        Self::collect_parquet_files(&sym_dir, start_date.map(|d| d.year()), &mut parquet_files);

        let mut all_candles: Vec<AggregatorCandle> = Vec::new();

        for path in parquet_files {
            if let Ok(file) = File::open(&path) {
                if let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) {
                    if let Ok(mut reader) = builder.build() {
                        while let Some(Ok(batch)) = reader.next() {
                            if let Ok(candles) = record_batch_to_candles(&batch) {
                                all_candles.extend(candles);
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate: settled records take precedence over provisional ones
        let mut dedup_map = HashMap::with_capacity(all_candles.len());
        for candle in all_candles {
            let key = candle.natural_key();
            match dedup_map.entry(key) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(candle);
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    if !e.get().is_settled && candle.is_settled {
                        e.insert(candle);
                    }
                }
            }
        }

        let mut filtered: Vec<AggregatorCandle> = dedup_map
            .into_values()
            .filter(|c| {
                let d = c.timestamp_utc.date_naive();
                if let Some(s) = start_date {
                    if d < s {
                        return false;
                    }
                }
                if let Some(e) = end_date {
                    if d > e {
                        return false;
                    }
                }
                true
            })
            .collect();

        filtered.sort_by_key(|c| c.timestamp_utc);

        Ok(filtered)
    }

    /// Resolves the timeframe directory for a symbol, searching known asset classes if not specified.
    fn resolve_symbol_dir(
        lake_dir: &Path,
        asset_class: Option<&str>,
        clean_symbol: &str,
        timeframe: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let market_dir = lake_dir.join("market");
        if let Some(ac) = asset_class {
            return Ok(market_dir
                .join(format!("asset_class={}", ac))
                .join(format!("symbol={}", clean_symbol))
                .join(format!("timeframe={}", timeframe)));
        }

        // Search known asset classes
        let known_classes = ["equities", "commodities", "forex", "crypto"];
        for ac in known_classes {
            let target = market_dir
                .join(format!("asset_class={}", ac))
                .join(format!("symbol={}", clean_symbol))
                .join(format!("timeframe={}", timeframe));
            if target.exists() {
                return Ok(target);
            }
        }

        // Fallback default
        Ok(market_dir
            .join("asset_class=equities")
            .join(format!("symbol={}", clean_symbol))
            .join(format!("timeframe={}", timeframe)))
    }

    fn collect_parquet_files(dir: &Path, min_year: Option<i32>, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Some(min_y) = min_year {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if name.starts_with("year=") {
                                if let Ok(y) = name.trim_start_matches("year=").parse::<i32>() {
                                    if y < min_y {
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    Self::collect_parquet_files(&p, min_year, out);
                } else if p.extension().map_or(false, |ext| ext == "parquet") {
                    out.push(p);
                }
            }
        }
    }
}
