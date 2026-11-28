//! Persistent Append-Only Arrow/Parquet ZSTD(3) Ledger for Deterministic Diagnostic Tree Runs (Phase 348).
//! Stored strictly under <lake_dir>/diagnostics/tree_runs/year=YYYY/ — isolated from raw lake/observations/.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, Int64Array, StringArray, TimestampNanosecondArray, UInt32Array};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
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

    /// Returns the newest persisted full DiagnosticTreeReport matching `window_weeks`.
    pub fn latest_run_for_window(
        &self,
        window_weeks: usize,
    ) -> Result<Option<DiagnosticTreeReport>, String> {
        let files = collect_parquet_files(&self.base_dir);
        let mut summaries = Vec::new();
        for path in files {
            if let Ok(rows) = read_summaries_from_file(&path) {
                for r in rows {
                    if r.window_weeks == window_weeks {
                        summaries.push(r);
                    }
                }
            }
        }
        summaries.sort_by(|a, b| b.evaluated_at_utc.cmp(&a.evaluated_at_utc));
        match summaries.first() {
            Some(s) => self.get_run(&s.run_id),
            None => Ok(None),
        }
    }

    /// Non-destructive bin-packing compaction per `year=YYYY/` partition (dedups by run_id, zero data loss).
    pub fn compact_year_partitions(&self, min_files: usize) -> Result<usize, String> {
        let Ok(entries) = fs::read_dir(&self.base_dir) else { return Ok(0) };
        let threshold = min_files.max(2);
        let mut removed = 0usize;
        for entry in entries.flatten() {
            let year_dir = entry.path();
            if !year_dir.is_dir() {
                continue;
            }
            let files = collect_parquet_files(&year_dir);
            if files.len() < threshold {
                continue;
            }
            let mut by_id: BTreeMap<String, DiagnosticTreeReport> = BTreeMap::new();
            for p in &files {
                if let Ok(reps) = read_all_reports_from_file(p) {
                    for rep in reps {
                        by_id.insert(rep.run_id.clone(), rep);
                    }
                }
            }
            let mut reports: Vec<DiagnosticTreeReport> = by_id.into_values().collect();
            reports.sort_by_key(|r| r.evaluated_at_utc);
            if reports.is_empty() {
                continue;
            }
            let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
            let dest = year_dir.join(format!("compacted_{}.parquet", stamp));
            write_reports_batch(&dest, &reports)?;
            for old in files {
                if old != dest {
                    let _ = fs::remove_file(old);
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

fn write_reports_batch(dest: &Path, reports: &[DiagnosticTreeReport]) -> Result<(), String> {
    let tmp = dest.with_extension("parquet.tmp");
    let run_ids: Vec<&str> = reports.iter().map(|r| r.run_id.as_str()).collect();
    let ts: Vec<i64> = reports.iter().map(|r| r.evaluated_at_utc.timestamp_nanos_opt().unwrap_or(0)).collect();
    let as_ofs: Vec<&str> = reports.iter().map(|r| r.as_of_date.as_str()).collect();
    let wins: Vec<u32> = reports.iter().map(|r| r.window_weeks as u32).collect();
    let vers: Vec<&str> = reports.iter().map(|r| r.protocol_version.as_str()).collect();
    let regs: Vec<&str> = reports.iter().map(|r| r.root_regime.as_str()).collect();
    let leaves: Vec<&str> = reports.iter().map(|r| r.terminal_leaf_id.as_str()).collect();
    let tiers: Vec<&str> = reports.iter().map(|r| r.terminal_epistemic_tier.as_str()).collect();
    let bs: Vec<i64> = reports.iter().map(|r| r.blind_spots.len() as i64).collect();
    let st: Vec<i64> = reports.iter().map(|r| r.source_lineage.iter().filter(|s| s.is_stale).count() as i64).collect();
    let shas: Vec<&str> = reports.iter().map(|r| r.tree_checksum_sha256.as_str()).collect();
    let paths_json: Vec<String> = reports.iter().map(|r| serde_json::to_string(&r.active_path).unwrap_or_default()).collect();
    let full_json: Vec<String> = reports.iter().map(|r| serde_json::to_string(r).unwrap_or_default()).collect();
    let path_refs: Vec<&str> = paths_json.iter().map(String::as_str).collect();
    let full_refs: Vec<&str> = full_json.iter().map(String::as_str).collect();

    let batch = RecordBatch::try_new(
        DiagnosticTreeLedger::schema(),
        vec![
            Arc::new(StringArray::from(run_ids)),
            Arc::new(TimestampNanosecondArray::from(ts).with_timezone("UTC")),
            Arc::new(StringArray::from(as_ofs)),
            Arc::new(UInt32Array::from(wins)),
            Arc::new(StringArray::from(vers)),
            Arc::new(StringArray::from(regs)),
            Arc::new(StringArray::from(leaves)),
            Arc::new(StringArray::from(tiers)),
            Arc::new(Int64Array::from(bs)),
            Arc::new(Int64Array::from(st)),
            Arc::new(StringArray::from(shas)),
            Arc::new(StringArray::from(path_refs)),
            Arc::new(StringArray::from(full_refs)),
        ],
    )
    .map_err(|e| format!("Arrow batch error: {}", e))?;

    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap_or_default()))
        .build();
    let file = File::create(&tmp).map_err(|e| format!("Create tmp parquet failed: {}", e))?;
    let mut writer = ArrowWriter::try_new(file, DiagnosticTreeLedger::schema(), Some(props))
        .map_err(|e| format!("Parquet writer init failed: {}", e))?;
    writer.write(&batch).map_err(|e| format!("Parquet write failed: {}", e))?;
    writer.close().map_err(|e| format!("Parquet close failed: {}", e))?;
    fs::rename(&tmp, dest).map_err(|e| format!("Atomic rename failed: {}", e))
}

fn read_all_reports_from_file(path: &Path) -> Result<Vec<DiagnosticTreeReport>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for batch_res in reader {
        let b = batch_res.map_err(|e| e.to_string())?;
        let full_jsons = b.column(12).as_any().downcast_ref::<StringArray>().ok_or("col 12")?;
        for i in 0..b.num_rows() {
            if let Ok(rep) = serde_json::from_str::<DiagnosticTreeReport>(full_jsons.value(i)) {
                out.push(rep);
            }
        }
    }
    Ok(out)
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
