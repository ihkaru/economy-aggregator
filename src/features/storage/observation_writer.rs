//! Bitemporal Append-Only Parquet Writer & Compactor helper for MetricObservations (Phase 310).

use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use chrono::{Datelike, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use uuid::Uuid;

use crate::core::observation_types::MetricObservation;
use crate::features::storage::observation_schema::{
    observations_to_record_batch, record_batch_to_observations,
};
use crate::features::storage::partition::PartitionResolver;

pub struct ObservationWriter {
    resolver: PartitionResolver,
    compression: Compression,
    row_group_size: usize,
}

impl ObservationWriter {
    pub fn new(resolver: PartitionResolver) -> Self {
        Self {
            resolver,
            compression: Compression::ZSTD(ZstdLevel::try_new(3).expect("valid zstd level")),
            row_group_size: 32_768,
        }
    }

    /// Validates provenance & finiteness, deduplicates by natural_key, sorts, and writes yearly Parquet chunks.
    pub fn write_observations(
        &self,
        observations: &[MetricObservation],
    ) -> std::io::Result<Vec<PathBuf>> {
        if observations.is_empty() {
            return Ok(Vec::new());
        }
        for obs in observations {
            obs.validate()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        }

        let deduped = self.deduplicate(observations);
        let mut by_part: HashMap<PathBuf, Vec<&MetricObservation>> = HashMap::new();
        for obs in &deduped {
            let dir = self.resolver.resolve_observation_partition(
                &obs.domain,
                &obs.series_id,
                obs.observation_ts_utc.year(),
            );
            by_part.entry(dir).or_default().push(obs);
        }

        let mut written = Vec::with_capacity(by_part.len());
        for (dir, mut group) in by_part {
            group.sort_by_key(|o| (o.observation_ts_utc, Reverse(o.known_at_utc)));
            fs::create_dir_all(&dir)?;
            written.push(self.write_chunk(&dir, &group)?);
        }
        Ok(written)
    }

    pub fn deduplicate<'a>(&self, rows: &'a [MetricObservation]) -> Vec<&'a MetricObservation> {
        let mut map: HashMap<String, &'a MetricObservation> = HashMap::with_capacity(rows.len());
        for r in rows {
            let key = r.natural_key();
            match map.get(&key) {
                Some(prev) if r.known_at_utc > prev.known_at_utc => {
                    map.insert(key, r);
                }
                None => {
                    map.insert(key, r);
                }
                _ => {}
            }
        }
        map.into_values().collect()
    }

    /// Compacts yearly observation partitions having at least `min_files` micro-chunks.
    pub fn compact_all(&self, min_files: usize) -> std::io::Result<usize> {
        let obs_root = self.resolver.base_lake_dir().join("observations");
        if !obs_root.exists() {
            return Ok(0);
        }
        let mut dirs = Vec::new();
        collect_leaf_dirs(&obs_root, &mut dirs)?;
        let mut compacted_count = 0usize;
        for dir in dirs {
            if self.compact_dir(&dir, min_files)? {
                compacted_count += 1;
            }
        }
        Ok(compacted_count)
    }

    fn compact_dir(&self, dir: &Path, min_files: usize) -> std::io::Result<bool> {
        let mut files: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.ends_with(".parquet") && (s.starts_with("chunk_") || s.starts_with("compacted_")))
                    .unwrap_or(false)
            })
            .collect();
        if files.len() < min_files {
            return Ok(false);
        }
        let mut rows = Vec::new();
        for p in &files {
            let f = File::open(p)?;
            let mut rdr = ParquetRecordBatchReaderBuilder::try_new(f)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                .build()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            while let Some(b) = rdr.next() {
                let batch = b.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                let parsed = record_batch_to_observations(&batch)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                rows.extend(parsed);
            }
        }
        let mut deduped = self.deduplicate(&rows);
        deduped.sort_by_key(|o| (o.observation_ts_utc, Reverse(o.known_at_utc)));
        let _out = self.write_named_chunk(dir, "compacted", &deduped)?;
        for old in files.drain(..) {
            let _ = fs::remove_file(old);
        }
        Ok(true)
    }

    fn write_chunk(&self, dir: &Path, group: &[&MetricObservation]) -> std::io::Result<PathBuf> {
        self.write_named_chunk(dir, "chunk", group)
    }

    fn write_named_chunk(
        &self,
        dir: &Path,
        prefix: &str,
        group: &[&MetricObservation],
    ) -> std::io::Result<PathBuf> {
        let id = &Uuid::new_v4().to_string()[..8];
        let ts = Utc::now().format("%Y%m%dT%H%M%S");
        let final_path = dir.join(format!("{}_{}_{}.parquet", prefix, ts, id));
        let tmp_path = dir.join(format!("{}_{}_{}.tmp", prefix, ts, id));

        let batch = observations_to_record_batch(group)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let props = WriterProperties::builder()
            .set_compression(self.compression)
            .set_dictionary_enabled(true)
            .set_max_row_group_size(self.row_group_size)
            .build();
        let file = File::create(&tmp_path)?;
        let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writer
            .write(&batch)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writer
            .close()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::rename(tmp_path, &final_path)?;
        Ok(final_path)
    }
}

fn collect_leaf_dirs(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut has_parquet = false;
    for entry in fs::read_dir(root)? {
        let p = entry?.path();
        if p.is_dir() {
            collect_leaf_dirs(&p, out)?;
        } else if p.extension().and_then(|e| e.to_str()) == Some("parquet") {
            has_parquet = true;
        }
    }
    if has_parquet {
        out.push(root.to_path_buf());
    }
    Ok(())
}
