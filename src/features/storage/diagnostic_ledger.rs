//! Persistent Append-Only Arrow/Parquet ZSTD(3) Ledger for Deterministic Diagnostic Tree Runs (Phase 348).
//! Stored strictly under <lake_dir>/diagnostics/tree_runs/year=YYYY/ — isolated from raw lake/observations/.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, Int64Array, StringArray, TimestampNanosecondArray, UInt32Array};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::DateTime;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use crate::core::diagnostic_types::{DiagnosticTreeReport, DiagnosticTreeRunSummary};

pub struct DiagnosticTreeLedger {
    base_dir: PathBuf,
}

impl DiagnosticTreeLedger {
    pub fn new(lake_dir: &Path) -> Self {
        Self {
            base_dir: lake_dir.join("diagnostics").join("tree_runs"),
        }
    }

    pub fn schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("run_id", DataType::Utf8, false),
            Field::new("evaluated_at_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
            Field::new("as_of_date", DataType::Utf8, false),
            Field::new("window_weeks", DataType::UInt32, false),
            Field::new("protocol_version", DataType::Utf8, false),
            Field::new("root_regime", DataType::Utf8, false),
            Field::new("terminal_leaf_id", DataType::Utf8, false),
            Field::new("terminal_epistemic_tier", DataType::Utf8, false),
            Field::new("blind_spot_count", DataType::Int64, false),
            Field::new("stale_source_count", DataType::Int64, false),
            Field::new("tree_checksum_sha256", DataType::Utf8, false),
            Field::new("active_path_json", DataType::Utf8, false),
            Field::new("full_report_json", DataType::Utf8, false),
        ]))
    }

    /// Persists an immutable Parquet snapshot for a diagnostic tree evaluation run.
    pub fn persist_run(&self, report: &DiagnosticTreeReport) -> Result<PathBuf, String> {
        let year = report.as_of_date.get(0..4).unwrap_or("unknown");
        let dir = self.base_dir.join(format!("year={}", year));
        fs::create_dir_all(&dir).map_err(|e| format!("Failed creating diagnostic dir: {}", e))?;
        let file_path = dir.join(format!("{}.parquet", report.run_id));
        let tmp_path = dir.join(format!(".{}.parquet.tmp", report.run_id));

        let active_path_json =
            serde_json::to_string(&report.active_path).map_err(|e| e.to_string())?;
        let full_report_json = serde_json::to_string(report).map_err(|e| e.to_string())?;
        let stale_cnt = report.source_lineage.iter().filter(|s| s.is_stale).count() as i64;
        let ts_ns = report.evaluated_at_utc.timestamp_nanos_opt().unwrap_or(0);

        let batch = RecordBatch::try_new(
            Self::schema(),
            vec![
                Arc::new(StringArray::from(vec![report.run_id.as_str()])),
                Arc::new(TimestampNanosecondArray::from(vec![ts_ns]).with_timezone("UTC")),
                Arc::new(StringArray::from(vec![report.as_of_date.as_str()])),
                Arc::new(UInt32Array::from(vec![report.window_weeks as u32])),
                Arc::new(StringArray::from(vec![report.protocol_version.as_str()])),
                Arc::new(StringArray::from(vec![report.root_regime.as_str()])),
                Arc::new(StringArray::from(vec![report.terminal_leaf_id.as_str()])),
                Arc::new(StringArray::from(vec![report.terminal_epistemic_tier.as_str()])),
                Arc::new(Int64Array::from(vec![report.blind_spots.len() as i64])),
                Arc::new(Int64Array::from(vec![stale_cnt])),
                Arc::new(StringArray::from(vec![report.tree_checksum_sha256.as_str()])),
                Arc::new(StringArray::from(vec![active_path_json.as_str()])),
                Arc::new(StringArray::from(vec![full_report_json.as_str()])),
            ],
        )
        .map_err(|e| format!("Arrow batch error: {}", e))?;

        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap_or_default()))
            .build();
        let file = File::create(&tmp_path).map_err(|e| format!("Create tmp parquet failed: {}", e))?;
        let mut writer = ArrowWriter::try_new(file, Self::schema(), Some(props))
            .map_err(|e| format!("Parquet writer init failed: {}", e))?;
        writer.write(&batch).map_err(|e| format!("Parquet write failed: {}", e))?;
        writer.close().map_err(|e| format!("Parquet close failed: {}", e))?;
        fs::rename(&tmp_path, &file_path).map_err(|e| format!("Atomic rename failed: {}", e))?;
        Ok(file_path)
    }

    /// Lists persisted diagnostic tree runs sorted newest-first.
    pub fn list_runs(&self, limit: usize) -> Result<Vec<DiagnosticTreeRunSummary>, String> {
        let files = collect_parquet_files(&self.base_dir);
        let mut out = Vec::with_capacity(files.len());
        for path in files {
            if let Ok(mut rows) = read_summaries_from_file(&path) {
                out.append(&mut rows);
            }
        }
        out.sort_by(|a, b| b.evaluated_at_utc.cmp(&a.evaluated_at_utc));
        out.truncate(limit.clamp(1, 200));
        Ok(out)
    }

    /// Retrieves a full persisted diagnostic tree report (with backtrack lineage) by exact run_id.
    pub fn get_run(&self, run_id: &str) -> Result<Option<DiagnosticTreeReport>, String> {
        let files = collect_parquet_files(&self.base_dir);
        for path in files {
            if let Ok(Some(rep)) = find_report_in_file(&path, run_id) {
                return Ok(Some(rep));
            }
        }
        Ok(None)
    }
}

fn collect_parquet_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.extend(collect_parquet_files(&p));
        } else if p.extension().and_then(|s| s.to_str()) == Some("parquet") {
            out.push(p);
        }
    }
    out
}

fn read_summaries_from_file(path: &Path) -> Result<Vec<DiagnosticTreeRunSummary>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for batch_res in reader {
        let b = batch_res.map_err(|e| e.to_string())?;
        let run_ids = b.column(0).as_any().downcast_ref::<StringArray>().ok_or("col 0")?;
        let ts_arr = b.column(1).as_any().downcast_ref::<TimestampNanosecondArray>().ok_or("col 1")?;
        let as_ofs = b.column(2).as_any().downcast_ref::<StringArray>().ok_or("col 2")?;
        let wins = b.column(3).as_any().downcast_ref::<UInt32Array>().ok_or("col 3")?;
        let vers = b.column(4).as_any().downcast_ref::<StringArray>().ok_or("col 4")?;
        let regs = b.column(5).as_any().downcast_ref::<StringArray>().ok_or("col 5")?;
        let leaves = b.column(6).as_any().downcast_ref::<StringArray>().ok_or("col 6")?;
        let tiers = b.column(7).as_any().downcast_ref::<StringArray>().ok_or("col 7")?;
        let bs = b.column(8).as_any().downcast_ref::<Int64Array>().ok_or("col 8")?;
        let st = b.column(9).as_any().downcast_ref::<Int64Array>().ok_or("col 9")?;
        let shas = b.column(10).as_any().downcast_ref::<StringArray>().ok_or("col 10")?;
        for i in 0..b.num_rows() {
            out.push(DiagnosticTreeRunSummary {
                run_id: run_ids.value(i).to_string(),
                evaluated_at_utc: DateTime::from_timestamp_nanos(ts_arr.value(i)),
                as_of_date: as_ofs.value(i).to_string(),
                window_weeks: wins.value(i) as usize,
                protocol_version: vers.value(i).to_string(),
                root_regime: regs.value(i).to_string(),
                terminal_leaf_id: leaves.value(i).to_string(),
                terminal_epistemic_tier: tiers.value(i).to_string(),
                blind_spot_count: bs.value(i) as usize,
                stale_source_count: st.value(i) as usize,
                tree_checksum_sha256: shas.value(i).to_string(),
            });
        }
    }
    Ok(out)
}

fn find_report_in_file(path: &Path, target_run_id: &str) -> Result<Option<DiagnosticTreeReport>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    for batch_res in reader {
        let b = batch_res.map_err(|e| e.to_string())?;
        let run_ids = b.column(0).as_any().downcast_ref::<StringArray>().ok_or("col 0")?;
        let full_jsons = b.column(12).as_any().downcast_ref::<StringArray>().ok_or("col 12")?;
        for i in 0..b.num_rows() {
            if run_ids.value(i) == target_run_id {
                let mut rep: DiagnosticTreeReport =
                    serde_json::from_str(full_jsons.value(i)).map_err(|e| e.to_string())?;
                rep.persisted_path = Some(path.display().to_string());
                return Ok(Some(rep));
            }
        }
    }
    Ok(None)
}
