//! Lake Compactor: Merges high-frequency micro-parquet chunks into unified, deduplicated Parquet files.
//! Solves the "Small Files Problem" (DEBT-01) without downtime.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use uuid::Uuid;

use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};
use crate::features::storage::arrow_schema::candles_to_record_batch;

#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub partition_dir: PathBuf,
    pub files_compacted: usize,
    pub total_rows_before: usize,
    pub total_rows_after: usize,
    pub compacted_file: Option<PathBuf>,
}

pub struct LakeCompactor {
    compression: Compression,
    row_group_size: usize,
}

impl Default for LakeCompactor {
    fn default() -> Self {
        Self {
            compression: Compression::ZSTD(ZstdLevel::try_new(3).unwrap()),
            row_group_size: 65_536, // Larger row groups for compacted long-term files
        }
    }
}

impl LakeCompactor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compacts all micro-chunks in a specific partition directory.
    /// If fewer than `min_files` exist, compaction is skipped.
    pub fn compact_partition(
        &self,
        part_dir: &Path,
        min_files: usize,
    ) -> Result<CompactionResult, Box<dyn std::error::Error + Send + Sync>> {
        if !part_dir.exists() {
            return Ok(CompactionResult {
                partition_dir: part_dir.to_path_buf(),
                files_compacted: 0,
                total_rows_before: 0,
                total_rows_after: 0,
                compacted_file: None,
            });
        }

        // 1. Collect all chunk_*.parquet files (ignore already compacted_*)
        let mut chunk_paths = Vec::new();
        for entry in fs::read_dir(part_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if file_name.starts_with("chunk_") && file_name.ends_with(".parquet") {
                    chunk_paths.push(path);
                }
            }
        }

        if chunk_paths.len() < min_files {
            return Ok(CompactionResult {
                partition_dir: part_dir.to_path_buf(),
                files_compacted: 0,
                total_rows_before: 0,
                total_rows_after: 0,
                compacted_file: None,
            });
        }

        // 2. Read and deserialize candles from each micro-chunk
        let mut all_candles: Vec<AggregatorCandle> = Vec::new();
        for path in &chunk_paths {
            let file = File::open(path)?;
            let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
            let mut reader = builder.build()?;

            while let Some(batch_res) = reader.next() {
                let batch = batch_res?;
                let candles = Self::record_batch_to_candles(&batch)?;
                all_candles.extend(candles);
            }
        }

        let total_rows_before = all_candles.len();

        // 3. Deduplicate in-memory: settled records win over provisional
        let mut dedup_map = std::collections::HashMap::new();
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

        let mut deduped_candles: Vec<AggregatorCandle> = dedup_map.into_values().collect();
        // Sort chronologically
        deduped_candles.sort_by_key(|c| c.timestamp_utc);

        let total_rows_after = deduped_candles.len();

        // 4. Write compacted file
        let chunk_id = &Uuid::new_v4().to_string()[..8];
        let ts_str = Utc::now().format("%Y%m%dT%H%M%S");
        let compacted_file = part_dir.join(format!("compacted_{}_{}.parquet", ts_str, chunk_id));
        let tmp_file = part_dir.join(format!("compacted_{}_{}.tmp", ts_str, chunk_id));

        let refs: Vec<&AggregatorCandle> = deduped_candles.iter().collect();
        let batch = candles_to_record_batch(&refs)?;

        let props = WriterProperties::builder()
            .set_compression(self.compression)
            .set_dictionary_enabled(true)
            .set_max_row_group_size(self.row_group_size)
            .build();

        let file = File::create(&tmp_file)?;
        let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;
        writer.write(&batch)?;
        writer.close()?;

        // Atomic file commit
        fs::rename(&tmp_file, &compacted_file)?;

        // 5. Clean up old micro-chunks after successful compaction
        for path in &chunk_paths {
            let _ = fs::remove_file(path);
        }

        Ok(CompactionResult {
            partition_dir: part_dir.to_path_buf(),
            files_compacted: chunk_paths.len(),
            total_rows_before,
            total_rows_after,
            compacted_file: Some(compacted_file),
        })
    }

    /// Converts an Arrow RecordBatch into domain AggregatorCandle structs.
    fn record_batch_to_candles(
        batch: &arrow::record_batch::RecordBatch,
    ) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
        use arrow::array::*;
        use arrow::datatypes::{DataType, TimeUnit};

        let ts_col = batch.column(0);
        let ts_micros: Vec<i64> = match ts_col.data_type() {
            DataType::Timestamp(TimeUnit::Nanosecond, _) => {
                let arr = ts_col.as_any().downcast_ref::<TimestampNanosecondArray>().ok_or("Failed downcast ts nanos")?;
                (0..batch.num_rows()).map(|i| arr.value(i) / 1_000).collect()
            }
            DataType::Timestamp(TimeUnit::Microsecond, _) => {
                let arr = ts_col.as_any().downcast_ref::<TimestampMicrosecondArray>().ok_or("Failed downcast ts micros")?;
                (0..batch.num_rows()).map(|i| arr.value(i)).collect()
            }
            DataType::Timestamp(TimeUnit::Millisecond, _) => {
                let arr = ts_col.as_any().downcast_ref::<TimestampMillisecondArray>().ok_or("Failed downcast ts millis")?;
                (0..batch.num_rows()).map(|i| arr.value(i) * 1_000).collect()
            }
            dt => return Err(format!("Unexpected timestamp data type: {:?}", dt).into()),
        };

        let extract_str = |col_idx: usize, row_idx: usize| -> String {
            let col = batch.column(col_idx);
            if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
                return arr.value(row_idx).to_string();
            }
            if let Some(arr) = col.as_any().downcast_ref::<LargeStringArray>() {
                return arr.value(row_idx).to_string();
            }
            if let Some(dict) = col.as_any().downcast_ref::<DictionaryArray<arrow::datatypes::Int32Type>>() {
                if let Some(values) = dict.values().as_any().downcast_ref::<StringArray>() {
                    if let Some(key) = dict.key(row_idx) {
                        return values.value(key).to_string();
                    }
                }
            }
            String::new()
        };

        let open_array = batch.column(3).as_any().downcast_ref::<Float64Array>().ok_or("Invalid open col")?;
        let high_array = batch.column(4).as_any().downcast_ref::<Float64Array>().ok_or("Invalid high col")?;
        let low_array = batch.column(5).as_any().downcast_ref::<Float64Array>().ok_or("Invalid low col")?;
        let close_array = batch.column(6).as_any().downcast_ref::<Float64Array>().ok_or("Invalid close col")?;
        let vol_base_array = batch.column(7).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vol_base col")?;
        let vol_quote_array = batch.column(8).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vol_quote col")?;
        let taker_base_array = batch.column(9).as_any().downcast_ref::<Float64Array>().ok_or("Invalid taker_base col")?;
        let taker_quote_array = batch.column(10).as_any().downcast_ref::<Float64Array>().ok_or("Invalid taker_quote col")?;
        let is_settled_array = batch.column(14).as_any().downcast_ref::<BooleanArray>().ok_or("Invalid is_settled col")?;
        let sequence_id_array = batch.column(15).as_any().downcast_ref::<UInt64Array>().ok_or("Invalid seq col")?;
        let vwap_array = batch.column(16).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vwap col")?;
        let trade_count_array = batch.column(17).as_any().downcast_ref::<UInt32Array>().ok_or("Invalid trade_count col")?;

        let mut candles = Vec::with_capacity(batch.num_rows());
        for i in 0..batch.num_rows() {
            let ts = DateTime::from_timestamp_micros(ts_micros[i]).unwrap_or_else(Utc::now);
            let tf_str = extract_str(2, i);
            let timeframe = TimeFrame::from_str_opt(&tf_str).unwrap_or(TimeFrame::M1);

            let mode_str = extract_str(13, i);
            let ingestion_mode = match mode_str.as_str() {
                "rest_batch" => IngestionMode::RestBatch,
                "reconciliation" => IngestionMode::Reconciliation,
                _ => IngestionMode::LiveWs,
            };

            candles.push(AggregatorCandle {
                timestamp_utc: ts,
                canonical_symbol: extract_str(1, i),
                timeframe,
                open: open_array.value(i),
                high: high_array.value(i),
                low: low_array.value(i),
                close: close_array.value(i),
                volume_base: vol_base_array.value(i),
                volume_quote: vol_quote_array.value(i),
                taker_buy_volume_base: if taker_base_array.is_null(i) { None } else { Some(taker_base_array.value(i)) },
                taker_buy_volume_quote: if taker_quote_array.is_null(i) { None } else { Some(taker_quote_array.value(i)) },
                source: extract_str(11, i),
                raw_symbol: extract_str(12, i),
                ingestion_mode,
                is_settled: is_settled_array.value(i),
                sequence_id: if sequence_id_array.is_null(i) { None } else { Some(sequence_id_array.value(i)) },
                vwap: if vwap_array.is_null(i) { None } else { Some(vwap_array.value(i)) },
                trade_count: if trade_count_array.is_null(i) { None } else { Some(trade_count_array.value(i)) },
                price_type: extract_str(18, i),
                raw_metadata: if batch.num_columns() > 19 && !batch.column(19).is_null(i) {
                    let s = extract_str(19, i);
                    if s.is_empty() { None } else { Some(s) }
                } else {
                    None
                },
            });
        }

        Ok(candles)
    }
}
