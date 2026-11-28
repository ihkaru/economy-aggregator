//! Event-Sourced Parquet Ledger & Atomic Consumer Checkpoint Store for Continuous Analytics (Phase 354).
//! Manages:
//!   - `<lake_dir>/diagnostics/checkpoints/continuous_analytics_state.json` (atomic sync + rename)
//!   - `<lake_dir>/diagnostics/transitions/year=YYYY/*.parquet` (ZSTD(3) append-only + bin-pack compaction)
//!   - `<lake_dir>/diagnostics/prospective_oos/year=YYYY/*.parquet` (ZSTD(3) append-only frozen OOS scorecards)

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, StringArray, TimestampNanosecondArray, UInt32Array};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::Utc;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use crate::core::continuous_types::{
    ContinuousAnalyticsCheckpoint, ProspectiveScorecardReport, RegimeTransitionEvent,
};

pub struct ContinuousAnalyticsStore {
    diag_dir: PathBuf,
}

impl ContinuousAnalyticsStore {
    pub fn new(lake_dir: &Path) -> Self {
        Self {
            diag_dir: lake_dir.join("diagnostics"),
        }
    }

    pub fn checkpoint_path(&self) -> PathBuf {
        self.diag_dir.join("checkpoints").join("continuous_analytics_state.json")
    }

    pub fn load_checkpoint(&self) -> ContinuousAnalyticsCheckpoint {
        let path = self.checkpoint_path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<ContinuousAnalyticsCheckpoint>(&s).ok())
            .unwrap_or_default()
    }

    /// Atomically persists consumer checkpoint (write .tmp -> sync_all -> rename).
    pub fn save_checkpoint(&self, cp: &ContinuousAnalyticsCheckpoint) -> Result<PathBuf, String> {
        let path = self.checkpoint_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("checkpoint mkdir failed: {}", e))?;
        }
        let tmp = path.with_extension("json.tmp");
        let payload = serde_json::to_string_pretty(cp).map_err(|e| e.to_string())?;
        {
            let mut f = File::create(&tmp).map_err(|e| format!("checkpoint tmp create: {}", e))?;
            f.write_all(payload.as_bytes()).map_err(|e| format!("checkpoint write: {}", e))?;
            f.sync_all().map_err(|e| format!("checkpoint fsync: {}", e))?;
        }
        fs::rename(&tmp, &path).map_err(|e| format!("checkpoint atomic rename: {}", e))?;
        Ok(path)
    }

    pub fn transition_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("schema_version", DataType::UInt32, false),
            Field::new("transition_id", DataType::Utf8, false),
            Field::new("detected_at_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
            Field::new("as_of_date", DataType::Utf8, false),
            Field::new("window_weeks", DataType::UInt32, false),
            Field::new("protocol_version", DataType::Utf8, false),
            Field::new("transition_kind", DataType::Utf8, false),
            Field::new("from_leaf_id", DataType::Utf8, true),
            Field::new("to_leaf_id", DataType::Utf8, false),
            Field::new("to_checksum_sha256", DataType::Utf8, false),
            Field::new("full_event_json", DataType::Utf8, false),
        ]))
    }

    pub fn scorecard_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("schema_version", DataType::UInt32, false),
            Field::new("scorecard_id", DataType::Utf8, false),
            Field::new("evaluated_at_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
            Field::new("as_of_date", DataType::Utf8, false),
            Field::new("protocol_version", DataType::Utf8, false),
            Field::new("latest_panel_week", DataType::Utf8, true),
            Field::new("oos_weeks_collected", DataType::UInt32, false),
            Field::new("overall_readout_status", DataType::Utf8, false),
            Field::new("scorecard_checksum_sha256", DataType::Utf8, false),
            Field::new("full_report_json", DataType::Utf8, false),
        ]))
    }

    pub fn persist_transition(&self, ev: &RegimeTransitionEvent) -> Result<PathBuf, String> {
        let year = ev.as_of_date.get(0..4).unwrap_or("unknown");
        let dir = self.diag_dir.join("transitions").join(format!("year={}", year));
        fs::create_dir_all(&dir).map_err(|e| format!("transitions mkdir: {}", e))?;
        let file_path = dir.join(format!("{}.parquet", ev.transition_id));
        write_transitions_parquet(&file_path, std::slice::from_ref(ev))?;
        Ok(file_path)
    }

    pub fn list_transitions(
        &self,
        window_weeks: Option<usize>,
        limit: usize,
    ) -> Result<Vec<RegimeTransitionEvent>, String> {
        let files = collect_parquet(&self.diag_dir.join("transitions"));
        let mut by_id: BTreeMap<String, RegimeTransitionEvent> = BTreeMap::new();
        for p in files {
            if let Ok(rows) = read_transitions_file(&p) {
                for ev in rows {
                    if window_weeks.is_none_or(|w| ev.window_weeks == w) {
                        by_id.insert(ev.transition_id.clone(), ev);
                    }
                }
            }
        }
        let mut out: Vec<RegimeTransitionEvent> = by_id.into_values().collect();
        out.sort_by(|a, b| b.detected_at_utc.cmp(&a.detected_at_utc));
        out.truncate(limit.clamp(1, 500));
        Ok(out)
    }

    /// Non-destructive bin-packing compaction for transitions `year=YYYY/` partitions.
    pub fn compact_transition_partitions(&self, min_files: usize) -> Result<usize, String> {
        let root = self.diag_dir.join("transitions");
        let Ok(entries) = fs::read_dir(&root) else { return Ok(0) };
        let threshold = min_files.max(2);
        let mut total_removed = 0usize;
        for entry in entries.flatten() {
            let year_dir = entry.path();
            if !year_dir.is_dir() {
                continue;
            }
            let files = collect_parquet(&year_dir);
            if files.len() < threshold {
                continue;
            }
            let mut by_id: BTreeMap<String, RegimeTransitionEvent> = BTreeMap::new();
            for p in &files {
                if let Ok(rows) = read_transitions_file(p) {
                    for ev in rows {
                        by_id.insert(ev.transition_id.clone(), ev);
                    }
                }
            }
            let mut all: Vec<RegimeTransitionEvent> = by_id.into_values().collect();
            all.sort_by_key(|e| e.detected_at_utc);
            if all.is_empty() {
                continue;
            }
            let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
            let dest = year_dir.join(format!("compacted_{}.parquet", stamp));
            write_transitions_parquet(&dest, &all)?;
            for old in files {
                if old != dest {
                    let _ = fs::remove_file(old);
                    total_removed += 1;
                }
            }
        }
        Ok(total_removed)
    }

    pub fn persist_scorecard(&self, rep: &ProspectiveScorecardReport) -> Result<PathBuf, String> {
        let year = rep.as_of_date.get(0..4).unwrap_or("unknown");
        let dir = self.diag_dir.join("prospective_oos").join(format!("year={}", year));
        fs::create_dir_all(&dir).map_err(|e| format!("prospective_oos mkdir: {}", e))?;
        let file_path = dir.join(format!("{}.parquet", rep.scorecard_id));
        let tmp_path = dir.join(format!(".{}.parquet.tmp", rep.scorecard_id));
        let json = serde_json::to_string(rep).map_err(|e| e.to_string())?;
        let ts_ns = rep.evaluated_at_utc.timestamp_nanos_opt().unwrap_or(0);

        let batch = RecordBatch::try_new(
            Self::scorecard_schema(),
            vec![
                Arc::new(UInt32Array::from(vec![rep.schema_version])),
                Arc::new(StringArray::from(vec![rep.scorecard_id.as_str()])),
                Arc::new(TimestampNanosecondArray::from(vec![ts_ns]).with_timezone("UTC")),
                Arc::new(StringArray::from(vec![rep.as_of_date.as_str()])),
                Arc::new(StringArray::from(vec![rep.protocol_version.as_str()])),
                Arc::new(StringArray::from(vec![rep.latest_panel_week.as_deref()])),
                Arc::new(UInt32Array::from(vec![rep.oos_weeks_collected as u32])),
                Arc::new(StringArray::from(vec![rep.overall_readout_status.as_str()])),
                Arc::new(StringArray::from(vec![rep.scorecard_checksum_sha256.as_str()])),
                Arc::new(StringArray::from(vec![json.as_str()])),
            ],
        )
        .map_err(|e| format!("scorecard Arrow batch error: {}", e))?;

        write_zstd_parquet(&tmp_path, &file_path, Self::scorecard_schema(), &batch)?;
        Ok(file_path)
    }

    pub fn list_scorecards(&self, limit: usize) -> Result<Vec<ProspectiveScorecardReport>, String> {
        let files = collect_parquet(&self.diag_dir.join("prospective_oos"));
        let mut out = Vec::new();
        for p in files {
            if let Ok(mut rows) = read_scorecards_file(&p) {
                out.append(&mut rows);
            }
        }
        out.sort_by(|a, b| b.evaluated_at_utc.cmp(&a.evaluated_at_utc));
        out.truncate(limit.clamp(1, 200));
        Ok(out)
    }
}

fn write_transitions_parquet(dest: &Path, events: &[RegimeTransitionEvent]) -> Result<(), String> {
    let tmp = dest.with_extension("parquet.tmp");
    let sv: Vec<u32> = events.iter().map(|e| e.schema_version).collect();
    let ids: Vec<&str> = events.iter().map(|e| e.transition_id.as_str()).collect();
    let ts: Vec<i64> = events.iter().map(|e| e.detected_at_utc.timestamp_nanos_opt().unwrap_or(0)).collect();
    let as_ofs: Vec<&str> = events.iter().map(|e| e.as_of_date.as_str()).collect();
    let wins: Vec<u32> = events.iter().map(|e| e.window_weeks as u32).collect();
    let vers: Vec<&str> = events.iter().map(|e| e.protocol_version.as_str()).collect();
    let kinds: Vec<&str> = events.iter().map(|e| e.transition_kind.as_str()).collect();
    let froms: Vec<Option<&str>> = events.iter().map(|e| e.from_leaf_id.as_deref()).collect();
    let tos: Vec<&str> = events.iter().map(|e| e.to_leaf_id.as_str()).collect();
    let shas: Vec<&str> = events.iter().map(|e| e.to_checksum_sha256.as_str()).collect();
    let jsons: Vec<String> = events
        .iter()
        .map(|e| serde_json::to_string(e).unwrap_or_default())
        .collect();
    let json_refs: Vec<&str> = jsons.iter().map(String::as_str).collect();

    let schema = ContinuousAnalyticsStore::transition_schema();
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt32Array::from(sv)),
            Arc::new(StringArray::from(ids)),
            Arc::new(TimestampNanosecondArray::from(ts).with_timezone("UTC")),
            Arc::new(StringArray::from(as_ofs)),
            Arc::new(UInt32Array::from(wins)),
            Arc::new(StringArray::from(vers)),
            Arc::new(StringArray::from(kinds)),
            Arc::new(StringArray::from(froms)),
            Arc::new(StringArray::from(tos)),
            Arc::new(StringArray::from(shas)),
            Arc::new(StringArray::from(json_refs)),
        ],
    )
    .map_err(|e| format!("transition Arrow batch error: {}", e))?;

    write_zstd_parquet(&tmp, dest, schema, &batch)
}

fn write_zstd_parquet(
    tmp: &Path,
    dest: &Path,
    schema: Arc<Schema>,
    batch: &RecordBatch,
) -> Result<(), String> {
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap_or_default()))
        .build();
    let file = File::create(tmp).map_err(|e| format!("create tmp parquet: {}", e))?;
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))
        .map_err(|e| format!("init parquet writer: {}", e))?;
    writer.write(batch).map_err(|e| format!("write parquet: {}", e))?;
    writer.close().map_err(|e| format!("close parquet: {}", e))?;
    fs::rename(tmp, dest).map_err(|e| format!("atomic rename parquet: {}", e))
}

fn collect_parquet(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.extend(collect_parquet(&p));
        } else if p.extension().and_then(|s| s.to_str()) == Some("parquet") {
            out.push(p);
        }
    }
    out
}

fn read_transitions_file(path: &Path) -> Result<Vec<RegimeTransitionEvent>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for batch_res in reader {
        let b = batch_res.map_err(|e| e.to_string())?;
        let jsons = b.column(10).as_any().downcast_ref::<StringArray>().ok_or("col 10")?;
        for i in 0..b.num_rows() {
            if let Ok(ev) = serde_json::from_str::<RegimeTransitionEvent>(jsons.value(i)) {
                out.push(ev);
            }
        }
    }
    Ok(out)
}

fn read_scorecards_file(path: &Path) -> Result<Vec<ProspectiveScorecardReport>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for batch_res in reader {
        let b = batch_res.map_err(|e| e.to_string())?;
        let jsons = b.column(9).as_any().downcast_ref::<StringArray>().ok_or("col 9")?;
        for i in 0..b.num_rows() {
            if let Ok(mut rep) = serde_json::from_str::<ProspectiveScorecardReport>(jsons.value(i)) {
                rep.persisted_path = Some(path.display().to_string());
                out.push(rep);
            }
        }
    }
    Ok(out)
}
