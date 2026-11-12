//! Lake Macro Reader: Scans and deserializes partitioned Parquet archives of macroeconomic events.

use std::fs;
use std::path::Path;
use chrono::{Datelike, NaiveDate};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::core::macro_types::RichEconomicEvent;
use crate::features::storage::macro_arrow_schema::record_batch_to_macro_events;

pub struct LakeMacroReader;

impl LakeMacroReader {
    /// Reads and filters historical macroeconomic events from the partitioned Parquet lakehouse.
    pub fn read_events(
        lake_dir: &Path,
        country: Option<&str>,
        year: Option<i32>,
        month: Option<u32>,
        indicator: Option<&str>,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
        importance: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<RichEconomicEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let macro_base = lake_dir.join("macro").join("calendar");
        if !macro_base.exists() {
            return Ok(Vec::new());
        }

        let mut matching_files = Vec::new();

        // 1. Resolve country directories
        let country_dirs: Vec<_> = if let Some(c) = country {
            vec![macro_base.join(format!("country={}", c.to_uppercase()))]
        } else if let Ok(entries) = fs::read_dir(&macro_base) {
            entries.flatten().map(|e| e.path()).collect()
        } else {
            Vec::new()
        };

        // 2. Scan year and month partitions with pruning
        for c_dir in country_dirs {
            if !c_dir.is_dir() { continue; }
            let Ok(y_entries) = fs::read_dir(&c_dir) else { continue };
            for y_entry in y_entries.flatten() {
                let y_path = y_entry.path();
                if !y_path.is_dir() { continue; }
                let y_name = y_path.file_name().unwrap_or_default().to_string_lossy();
                let Some(parsed_year) = y_name.strip_prefix("year=").and_then(|s| s.parse::<i32>().ok()) else { continue };

                if let Some(target_y) = year {
                    if parsed_year != target_y { continue; }
                }

                let Ok(m_entries) = fs::read_dir(&y_path) else { continue };
                for m_entry in m_entries.flatten() {
                    let m_path = m_entry.path();
                    if !m_path.is_dir() { continue; }
                    let m_name = m_path.file_name().unwrap_or_default().to_string_lossy();
                    let Some(parsed_month) = m_name.strip_prefix("month=").and_then(|s| s.parse::<u32>().ok()) else { continue };

                    if let Some(target_m) = month {
                        if parsed_month != target_m { continue; }
                    }

                    // Check date range boundary
                    if let Some(from) = from_date {
                        if parsed_year < from.year() || (parsed_year == from.year() && parsed_month < from.month()) {
                            continue;
                        }
                    }
                    if let Some(to) = to_date {
                        if parsed_year > to.year() || (parsed_year == to.year() && parsed_month > to.month()) {
                            continue;
                        }
                    }

                    // Collect parquet files
                    if let Ok(f_entries) = fs::read_dir(&m_path) {
                        for f_entry in f_entries.flatten() {
                            let p = f_entry.path();
                            if p.extension().and_then(|s| s.to_str()) == Some("parquet") {
                                matching_files.push((parsed_year, parsed_month, p));
                            }
                        }
                    }
                }
            }
        }

        // Sort descending: newest partitions first
        matching_files.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));

        let mut events = Vec::new();
        let target_indicator = indicator.map(|s| s.trim().to_lowercase());
        let target_importance = importance.map(|s| s.trim().to_lowercase());

        for (_, _, file_path) in matching_files {
            let Ok(file) = fs::File::open(&file_path) else { continue };
            let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) else { continue };
            let Ok(mut reader) = builder.build() else { continue };

            while let Some(Ok(batch)) = reader.next() {
                if let Ok(batch_events) = record_batch_to_macro_events(&batch) {
                    for ev in batch_events {
                        // Filter by indicator
                        if let Some(ref ind) = target_indicator {
                            let matches_code = ev.indicator_code.to_lowercase().contains(ind);
                            let matches_title = ev.title.to_lowercase().contains(ind);
                            let matches_cat = ev.category.to_lowercase().contains(ind);
                            if !matches_code && !matches_title && !matches_cat {
                                continue;
                            }
                        }

                        // Filter by importance
                        if let Some(ref imp) = target_importance {
                            let match_imp = match imp.as_str() {
                                "high" | "3" => ev.impact_tier == 3,
                                "medium" | "2" => ev.impact_tier == 2,
                                "low" | "1" => ev.impact_tier <= 1,
                                _ => true,
                            };
                            if !match_imp { continue; }
                        }

                        // Filter by date range
                        let d = ev.scheduled_utc.date_naive();
                        if let Some(from) = from_date {
                            if d < from { continue; }
                        }
                        if let Some(to) = to_date {
                            if d > to { continue; }
                        }

                        events.push(ev);
                        if let Some(lim) = limit {
                            if events.len() >= lim {
                                return Ok(events);
                            }
                        }
                    }
                }
            }
        }

        Ok(events)
    }
}
