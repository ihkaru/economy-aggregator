//! Lake Reader: Reads, filters, and deduplicates historical candles from partitioned Parquet lake.
//! Supports Rayon-accelerated parallel reading and partition pruning by date and limit.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use chrono::{Datelike, NaiveDate};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rayon::prelude::*;

use crate::core::types::AggregatorCandle;
use crate::features::storage::arrow_schema::record_batch_to_candles;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionFile {
    pub year: i32,
    pub month: u32,
    pub path: PathBuf,
}

pub struct LakeReader;

impl LakeReader {
    /// Reads candles from the partitioned Parquet lake for a given symbol and timeframe.
    pub fn read_candles(
        lake_dir: &Path,
        asset_class: Option<&str>,
        symbol: &str,
        timeframe: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        Self::read_candles_with_limit(lake_dir, asset_class, symbol, timeframe, start_date, end_date, None, None)
    }

    /// Reads candles with parallel Rayon decompression, partition pruning, and backward timestamp cutoff.
    pub fn read_candles_with_limit(
        lake_dir: &Path,
        asset_class: Option<&str>,
        symbol: &str,
        timeframe: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        limit: Option<usize>,
        to_timestamp: Option<i64>,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let clean_symbol = symbol.to_uppercase().replace('/', "_").replace(':', "_");
        let sym_dir = Self::resolve_symbol_dir(lake_dir, asset_class, &clean_symbol, timeframe)?;

        if !sym_dir.exists() {
            return Ok(Vec::new());
        }

        let effective_end_date = end_date.or_else(|| {
            to_timestamp.and_then(|ts| {
                chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.date_naive())
            })
        });

        let mut partitions = Vec::new();
        Self::collect_partitions(&sym_dir, start_date, effective_end_date, &mut partitions);

        if partitions.is_empty() {
            return Ok(Vec::new());
        }

        // Sort descending: newest partitions first
        partitions.sort_by(|a, b| (b.year, b.month).cmp(&(a.year, a.month)));

        // If limit is specified and no start date bounding, prune older partitions
        if let Some(lim) = limit {
            if start_date.is_none() {
                // Minimum ~15 bars per month (equities ~21, crypto ~30)
                let max_partitions_needed = (lim / 15) + 6;
                if partitions.len() > max_partitions_needed {
                    partitions.truncate(max_partitions_needed);
                }
            }
        }

        // Parallel decompression across CPU cores via Rayon
        let batches: Vec<Vec<AggregatorCandle>> = partitions
            .par_iter()
            .filter_map(|pf| {
                let file = File::open(&pf.path).ok()?;
                let builder = ParquetRecordBatchReaderBuilder::try_new(file).ok()?;
                let mut reader = builder.build().ok()?;
                let mut local_candles = Vec::new();
                while let Some(Ok(batch)) = reader.next() {
                    if let Ok(candles) = record_batch_to_candles(&batch) {
                        local_candles.extend(candles);
                    }
                }
                Some(local_candles)
            })
            .collect();

        let total_count: usize = batches.iter().map(|b| b.len()).sum();
        let mut dedup_map = HashMap::with_capacity(total_count);

        for batch in batches {
            for candle in batch {
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
        }

        let mut filtered: Vec<AggregatorCandle> = dedup_map
            .into_values()
            .filter(|c| {
                if let Some(to_ts) = to_timestamp {
                    if c.time >= to_ts {
                        return false;
                    }
                }
                let d = c.timestamp_utc.date_naive();
                if let Some(s) = start_date {
                    if d < s {
                        return false;
                    }
                }
                if let Some(e) = effective_end_date {
                    if d > e {
                        return false;
                    }
                }
                true
            })
            .collect();

        filtered.sort_by_key(|c| c.timestamp_utc);

        if let Some(lim) = limit {
            if filtered.len() > lim {
                let start_idx = filtered.len() - lim;
                filtered = filtered.split_off(start_idx);
            }
        }

        Ok(filtered)
    }

    /// Fast metadata retrieval (total rows, start date, end date) without decompressing full datasets.
    pub fn read_metadata_bounds(
        lake_dir: &Path,
        asset_class: Option<&str>,
        symbol: &str,
        timeframe: &str,
    ) -> Result<(usize, Option<String>, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        let clean_symbol = symbol.to_uppercase().replace('/', "_").replace(':', "_");
        let sym_dir = Self::resolve_symbol_dir(lake_dir, asset_class, &clean_symbol, timeframe)?;

        if !sym_dir.exists() {
            return Ok((0, None, None));
        }

        let mut partitions = Vec::new();
        Self::collect_partitions(&sym_dir, None, None, &mut partitions);

        if partitions.is_empty() {
            return Ok((0, None, None));
        }

        // Sort ascending: oldest first, newest last
        partitions.sort_by(|a, b| (a.year, a.month).cmp(&(b.year, b.month)));

        // Sum row count directly from Parquet file metadata footer (zero row decompression)
        let total_rows: usize = partitions
            .par_iter()
            .map(|pf| {
                if let Ok(file) = File::open(&pf.path) {
                    if let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) {
                        return builder.metadata().file_metadata().num_rows() as usize;
                    }
                }
                0
            })
            .sum();

        // Oldest partition -> first date
        let first_date = partitions.first().and_then(|pf| {
            let file = File::open(&pf.path).ok()?;
            let builder = ParquetRecordBatchReaderBuilder::try_new(file).ok()?;
            let mut reader = builder.build().ok()?;
            if let Some(Ok(batch)) = reader.next() {
                if let Ok(candles) = record_batch_to_candles(&batch) {
                    return candles.first().map(|c| c.timestamp_utc.format("%Y-%m-%d").to_string());
                }
            }
            None
        });

        // Newest partition -> last date
        let last_date = partitions.last().and_then(|pf| {
            let file = File::open(&pf.path).ok()?;
            let builder = ParquetRecordBatchReaderBuilder::try_new(file).ok()?;
            let mut reader = builder.build().ok()?;
            let mut latest_candle = None;
            while let Some(Ok(batch)) = reader.next() {
                if let Ok(candles) = record_batch_to_candles(&batch) {
                    if let Some(c) = candles.last() {
                        latest_candle = Some(c.timestamp_utc.format("%Y-%m-%d").to_string());
                    }
                }
            }
            latest_candle
        });

        Ok((total_rows, first_date, last_date))
    }

    /// Resolves the timeframe directory for a symbol, searching known asset classes if not specified.
    fn resolve_symbol_dir(
        lake_dir: &Path,
        asset_class: Option<&str>,
        clean_symbol: &str,
        timeframe: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let market_dir = lake_dir.join("market");
        let search_classes: Vec<&str> = if let Some(ac) = asset_class {
            vec![ac]
        } else {
            vec!["equities", "commodities", "forex", "crypto"]
        };

        // Candidate symbols to check: clean_symbol first, then common vendor stripped aliases
        let mut candidates = vec![clean_symbol.to_string()];
        let stripped = clean_symbol.trim_start_matches('^').replace(".JK", "").replace("=X", "");
        if stripped != clean_symbol {
            candidates.push(stripped);
        }
        if clean_symbol == "BTCUSDT" {
            candidates.push("BTC_USDT".to_string());
        }
        if clean_symbol == "GC=F" {
            candidates.push("XAU_USD".to_string());
        }
        if clean_symbol == "USDIDR=X" {
            candidates.push("USD_IDR".to_string());
        }

        for ac in &search_classes {
            for cand in &candidates {
                let target = market_dir
                    .join(format!("asset_class={}", ac))
                    .join(format!("symbol={}", cand))
                    .join(format!("timeframe={}", timeframe));
                if target.exists() {
                    return Ok(target);
                }
            }
        }

        let default_ac = asset_class.unwrap_or("equities");
        Ok(market_dir
            .join(format!("asset_class={}", default_ac))
            .join(format!("symbol={}", clean_symbol))
            .join(format!("timeframe={}", timeframe)))
    }

    /// Collects and prunes partition files by date range directly at directory traversal time.
    fn collect_partitions(
        dir: &Path,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        out: &mut Vec<PartitionFile>,
    ) {
        let Ok(year_entries) = std::fs::read_dir(dir) else { return };

        for y_entry in year_entries.flatten() {
            let y_path = y_entry.path();
            if !y_path.is_dir() { continue; }

            let Some(y_name) = y_path.file_name().and_then(|n| n.to_str()) else { continue };
            if !y_name.starts_with("year=") { continue; }
            let Ok(year) = y_name.trim_start_matches("year=").parse::<i32>() else { continue };

            if let Some(s) = start_date {
                if year < s.year() { continue; }
            }
            if let Some(e) = end_date {
                if year > e.year() { continue; }
            }

            let Ok(month_entries) = std::fs::read_dir(&y_path) else { continue };
            for m_entry in month_entries.flatten() {
                let m_path = m_entry.path();
                if !m_path.is_dir() { continue; }

                let Some(m_name) = m_path.file_name().and_then(|n| n.to_str()) else { continue };
                if !m_name.starts_with("month=") { continue; }
                let Ok(month) = m_name.trim_start_matches("month=").parse::<u32>() else { continue };

                if let Some(s) = start_date {
                    if year == s.year() && month < s.month() { continue; }
                }
                if let Some(e) = end_date {
                    if year == e.year() && month > e.month() { continue; }
                }

                let Ok(file_entries) = std::fs::read_dir(&m_path) else { continue };
                for f_entry in file_entries.flatten() {
                    let f_path = f_entry.path();
                    if f_path.extension().map_or(false, |ext| ext == "parquet") {
                        out.push(PartitionFile { year, month, path: f_path });
                    }
                }
            }
        }
    }
}
