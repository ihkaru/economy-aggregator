//! Lake Compactor: Merges high-frequency micro-parquet chunks into unified, deduplicated Parquet files.
//! Solves the "Small Files Problem" (DEBT-01) without downtime.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use chrono::Utc;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use uuid::Uuid;

use crate::core::types::AggregatorCandle;
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
                let candles = crate::features::storage::arrow_schema::record_batch_to_candles(&batch)?;
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

    /// Recursively scans and compacts all partitions under a root lake directory.
    pub fn compact_all(
        &self,
        root_dir: &Path,
        min_files: usize,
    ) -> Result<Vec<CompactionResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();
        let mut dirs = Vec::new();
        Self::collect_partition_dirs(root_dir, &mut dirs);

        for dir in dirs {
            let res = self.compact_partition(&dir, min_files)?;
            if res.files_compacted > 0 {
                results.push(res);
            }
        }
        Ok(results)
    }

    fn collect_partition_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            let mut has_chunks = false;
            let mut subdirs = Vec::new();
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    subdirs.push(p);
                } else if let Some(name) = p.file_name().and_then(|f| f.to_str()) {
                    if name.starts_with("chunk_") && name.ends_with(".parquet") {
                        has_chunks = true;
                    }
                }
            }
            if has_chunks {
                out.push(dir.to_path_buf());
            }
            for sub in subdirs {
                Self::collect_partition_dirs(&sub, out);
            }
        }
    }
}
