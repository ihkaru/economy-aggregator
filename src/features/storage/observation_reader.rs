//! Bitemporal Point-in-Time Parquet Reader & Coverage Summarizer for MetricObservations (Phase 310).

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Datelike, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};

use crate::core::observation_types::{sanitize_partition_component, MetricObservation};
use crate::features::storage::observation_schema::record_batch_to_observations;
use crate::features::storage::partition::PartitionResolver;

#[derive(Debug, Clone, Default)]
pub struct ObservationFilter {
    pub series_ids: Vec<String>,
    pub domain: Option<String>,
    pub compartment: Option<String>,
    pub source: Option<String>,
    pub start_utc: Option<DateTime<Utc>>,
    pub end_utc: Option<DateTime<Utc>>,
    pub as_of_known_at: Option<DateTime<Utc>>,
    pub resolve_latest_revision: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesCoverageSummary {
    pub series_id: String,
    pub domain: String,
    pub compartment: Option<String>,
    pub entity_id: String,
    pub metric_name: String,
    pub frequency: String,
    pub unit: String,
    pub row_count: usize,
    pub min_ts_utc: DateTime<Utc>,
    pub max_ts_utc: DateTime<Utc>,
    pub latest_known_at_utc: DateTime<Utc>,
    pub sources: Vec<String>,
}

pub struct ObservationReader {
    resolver: PartitionResolver,
}

impl ObservationReader {
    pub fn new(resolver: PartitionResolver) -> Self {
        Self { resolver }
    }

    pub fn read_observations(&self, filter: &ObservationFilter) -> Vec<MetricObservation> {
        let obs_root = self.resolver.base_lake_dir().join("observations");
        if !obs_root.exists() {
            return Vec::new();
        }
        let paths = self.discover_candidate_files(&obs_root, filter);
        let mut rows = self.load_filtered_files(&paths, filter);

        if filter.resolve_latest_revision || filter.as_of_known_at.is_some() {
            rows = resolve_bitemporal_latest(rows);
        } else {
            rows.sort_by_key(|r| (r.observation_ts_utc, r.known_at_utc));
        }

        if let Some(lim) = filter.limit {
            if rows.len() > lim {
                let start = rows.len() - lim;
                rows = rows.split_off(start);
            }
        }
        rows
    }

    pub fn summarize_coverage(&self) -> Vec<SeriesCoverageSummary> {
        let rows = self.read_observations(&ObservationFilter {
            resolve_latest_revision: true,
            ..Default::default()
        });
        let mut grouped: HashMap<String, Vec<MetricObservation>> = HashMap::new();
        for r in rows {
            grouped.entry(r.series_id.clone()).or_default().push(r);
        }

        let mut summaries = Vec::with_capacity(grouped.len());
        for (series_id, list) in grouped {
            let first = &list[0];
            let mut sources = BTreeSet::new();
            let mut min_ts = first.observation_ts_utc;
            let mut max_ts = first.observation_ts_utc;
            let mut latest_known = first.known_at_utc;
            for r in &list {
                sources.insert(r.provenance.source.clone());
                if r.observation_ts_utc < min_ts {
                    min_ts = r.observation_ts_utc;
                }
                if r.observation_ts_utc > max_ts {
                    max_ts = r.observation_ts_utc;
                }
                if r.known_at_utc > latest_known {
                    latest_known = r.known_at_utc;
                }
            }
            summaries.push(SeriesCoverageSummary {
                series_id,
                domain: first.domain.clone(),
                compartment: first.compartment.clone(),
                entity_id: first.entity_id.clone(),
                metric_name: first.metric_name.clone(),
                frequency: first.frequency.as_str().to_string(),
                unit: first.unit.clone(),
                row_count: list.len(),
                min_ts_utc: min_ts,
                max_ts_utc: max_ts,
                latest_known_at_utc: latest_known,
                sources: sources.into_iter().collect(),
            });
        }
        summaries.sort_by(|a, b| a.series_id.cmp(&b.series_id));
        summaries
    }

    fn discover_candidate_files(&self, root: &Path, filter: &ObservationFilter) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let target_domains: Vec<PathBuf> = if let Some(ref d) = filter.domain {
            let p = root.join(format!("domain={}", sanitize_partition_component(d).to_lowercase()));
            if p.exists() { vec![p] } else { Vec::new() }
        } else {
            fs::read_dir(root)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        };

        let wanted_series: BTreeSet<String> = filter
            .series_ids
            .iter()
            .map(|s| format!("series_id={}", sanitize_partition_component(s)))
            .collect();

        for dom_dir in target_domains {
            let Ok(series_dirs) = fs::read_dir(&dom_dir) else { continue };
            for entry in series_dirs.filter_map(Result::ok) {
                let sp = entry.path();
                if !sp.is_dir() {
                    continue;
                }
                if !wanted_series.is_empty() {
                    let fname = sp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !wanted_series.contains(fname) {
                        continue;
                    }
                }
                collect_year_parquets(&sp, filter.start_utc, filter.end_utc, &mut out);
            }
        }
        out
    }

    fn load_filtered_files(&self, paths: &[PathBuf], filter: &ObservationFilter) -> Vec<MetricObservation> {
        let mut out = Vec::new();
        for p in paths {
            let Ok(file) = File::open(p) else { continue };
            let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) else { continue };
            let Ok(mut reader) = builder.build() else { continue };
            while let Some(Ok(batch)) = reader.next() {
                if let Ok(rows) = record_batch_to_observations(&batch) {
                    for r in rows {
                        if matches_filter(&r, filter) {
                            out.push(r);
                        }
                    }
                }
            }
        }
        out
    }
}

fn collect_year_parquets(
    series_dir: &Path,
    start_utc: Option<DateTime<Utc>>,
    end_utc: Option<DateTime<Utc>>,
    out: &mut Vec<PathBuf>,
) {
    let min_y = start_utc.map(|d| d.year()).unwrap_or(i32::MIN);
    let max_y = end_utc.map(|d| d.year()).unwrap_or(i32::MAX);
    let Ok(year_dirs) = fs::read_dir(series_dir) else { return };
    for entry in year_dirs.filter_map(Result::ok) {
        let yp = entry.path();
        if !yp.is_dir() {
            continue;
        }
        if let Some(y) = yp
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.strip_prefix("year="))
            .and_then(|s| s.parse::<i32>().ok())
        {
            if y < min_y || y > max_y {
                continue;
            }
        }
        if let Ok(files) = fs::read_dir(&yp) {
            for f in files.filter_map(Result::ok) {
                let fp = f.path();
                if fp.extension().and_then(|e| e.to_str()) == Some("parquet") {
                    out.push(fp);
                }
            }
        }
    }
}

fn matches_filter(r: &MetricObservation, f: &ObservationFilter) -> bool {
    if !f.series_ids.is_empty() && !f.series_ids.iter().any(|s| s == &r.series_id) {
        return false;
    }
    if let Some(ref c) = f.compartment {
        if r.compartment.as_deref() != Some(c.as_str()) {
            return false;
        }
    }
    if let Some(ref src) = f.source {
        if !r.provenance.source.eq_ignore_ascii_case(src) {
            return false;
        }
    }
    if let Some(s) = f.start_utc {
        if r.observation_ts_utc < s {
            return false;
        }
    }
    if let Some(e) = f.end_utc {
        if r.observation_ts_utc > e {
            return false;
        }
    }
    if let Some(as_of) = f.as_of_known_at {
        if r.known_at_utc > as_of {
            return false;
        }
    }
    true
}

fn resolve_bitemporal_latest(rows: Vec<MetricObservation>) -> Vec<MetricObservation> {
    // Key strictly by (series_id, source, timestamp) so distinct sources never overwrite each other.
    let mut best: HashMap<(String, String, i64), MetricObservation> = HashMap::new();
    for r in rows {
        let key = (
            r.series_id.clone(),
            r.provenance.source.clone(),
            r.observation_ts_utc.timestamp_nanos_opt().unwrap_or(0),
        );
        match best.get(&key) {
            Some(prev) => {
                let newer = (r.revision_seq, r.known_at_utc) > (prev.revision_seq, prev.known_at_utc);
                if newer {
                    best.insert(key, r);
                }
            }
            None => {
                best.insert(key, r);
            }
        }
    }
    let mut resolved: Vec<MetricObservation> = best.into_values().collect();
    resolved.sort_by_key(|r| (r.series_id.clone(), r.observation_ts_utc));
    resolved
}
