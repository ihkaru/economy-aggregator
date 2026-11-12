//! Lake Prediction Reader: Scans and deserializes partitioned Parquet archives of prediction market candles.

use std::fs;
use std::path::Path;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::core::prediction_types::PredictionMarketCandle;
use crate::features::storage::prediction_arrow_schema::record_batch_to_prediction_candles;

pub struct LakePredictionReader;

impl LakePredictionReader {
    /// Reads historical prediction candles from the partitioned Parquet lakehouse.
    pub fn read_candles(
        lake_dir: &Path,
        venue: Option<&str>,
        market_id: &str,
        timeframe: &str,
        from_timestamp: Option<i64>,
        to_timestamp: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<PredictionMarketCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let base_dir = lake_dir.join("prediction_markets").join("candles");
        if !base_dir.exists() {
            return Ok(Vec::new());
        }

        let clean_market = market_id.replace('/', "_").replace(':', "_");
        let venues = if let Some(v) = venue {
            vec![v.to_lowercase()]
        } else {
            vec!["kalshi".to_string(), "polymarket".to_string()]
        };

        let mut matching_files = Vec::new();

        for v in venues {
            let market_dir = base_dir
                .join(format!("venue={}", v))
                .join(format!("market={}", clean_market))
                .join(format!("timeframe={}", timeframe));

            if !market_dir.exists() {
                continue;
            }

            let Ok(y_entries) = fs::read_dir(&market_dir) else { continue };
            for y_entry in y_entries.flatten() {
                let y_path = y_entry.path();
                if !y_path.is_dir() { continue; }
                let y_name = y_path.file_name().unwrap_or_default().to_string_lossy();
                let Some(parsed_year) = y_name.strip_prefix("year=").and_then(|s| s.parse::<i32>().ok()) else { continue };

                let Ok(m_entries) = fs::read_dir(&y_path) else { continue };
                for m_entry in m_entries.flatten() {
                    let m_path = m_entry.path();
                    if !m_path.is_dir() { continue; }
                    let m_name = m_path.file_name().unwrap_or_default().to_string_lossy();
                    let Some(parsed_month) = m_name.strip_prefix("month=").and_then(|s| s.parse::<u32>().ok()) else { continue };

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

        matching_files.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));

        let mut candles = Vec::new();

        for (_, _, file_path) in matching_files {
            let Ok(file) = fs::File::open(&file_path) else { continue };
            let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) else { continue };
            let Ok(mut reader) = builder.build() else { continue };

            while let Some(Ok(batch)) = reader.next() {
                if let Ok(batch_candles) = record_batch_to_prediction_candles(&batch) {
                    for c in batch_candles {
                        if let Some(from_ts) = from_timestamp {
                            if c.time < from_ts { continue; }
                        }
                        if let Some(to_ts) = to_timestamp {
                            if c.time >= to_ts { continue; }
                        }
                        candles.push(c);
                        if let Some(lim) = limit {
                            if candles.len() >= lim {
                                candles.sort_by_key(|c| c.time);
                                return Ok(candles);
                            }
                        }
                    }
                }
            }
        }

        candles.sort_by_key(|c| c.time);
        if let Some(lim) = limit {
            if candles.len() > lim {
                let start_idx = candles.len() - lim;
                candles = candles.split_off(start_idx);
            }
        }

        Ok(candles)
    }
}
