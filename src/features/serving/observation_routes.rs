//! REST API Endpoints for Universal Bitemporal Observations, Catalog & Dynamic Wide Matrix (Phase 322).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde::Deserialize;

use crate::core::series_catalog::SeriesCatalog;
use crate::features::ingestion::liquidity_sync::sync_observations;
use crate::features::serving::auth::validate_safe_identifier;
use crate::features::serving::rest::AppState;
use crate::features::storage::{ObservationFilter, ObservationReader, PartitionResolver};

#[derive(Debug, Deserialize)]
pub struct ObservationQuery {
    pub series_id: Option<String>,
    pub domain: Option<String>,
    pub compartment: Option<String>,
    pub source: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub as_of_known_at: Option<String>,
    pub resolve_latest: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct MatrixQuery {
    pub series_ids: Option<String>,
    pub compartment: Option<String>,
    pub source: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub as_of_known_at: Option<String>,
    pub resample: Option<String>,
    pub format: Option<String>,
    pub forward_fill: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SyncQuery {
    pub target: Option<String>,
}

fn parse_utc_opt(raw: &Option<String>, end_of_day: bool) -> Result<Option<DateTime<Utc>>, (StatusCode, String)> {
    let Some(s) = raw.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(Some(dt.with_timezone(&Utc)));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let t = if end_of_day {
            d.and_hms_opt(23, 59, 59)
        } else {
            d.and_hms_opt(0, 0, 0)
        };
        return Ok(t.map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc)));
    }
    Err((StatusCode::BAD_REQUEST, format!("Invalid date/timestamp '{}'", s)))
}

pub async fn query_observations_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ObservationQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if let Some(ref sid) = q.series_id {
        validate_safe_identifier(sid, "series_id")?;
    }
    let filter = ObservationFilter {
        series_ids: q.series_id.map(|s| vec![s]).unwrap_or_default(),
        domain: q.domain,
        compartment: q.compartment,
        source: q.source,
        start_utc: parse_utc_opt(&q.start_date, false)?,
        end_utc: parse_utc_opt(&q.end_date, true)?,
        as_of_known_at: parse_utc_opt(&q.as_of_known_at, true)?,
        resolve_latest_revision: q.resolve_latest.unwrap_or(true),
        limit: Some(q.limit.unwrap_or(1000).min(10_000)),
    };

    let lake_dir = state.config.lake_dir.clone();
    let rows = tokio::task::spawn_blocking(move || {
        let reader = ObservationReader::new(PartitionResolver::new(lake_dir));
        reader.read_observations(&filter)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "count": rows.len(),
        "data": rows
    })))
}

pub async fn query_observations_catalog_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let cached = {
        let guard = state.obs_catalog_cache.read().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().and_then(|(ts, cov)| {
            if ts.elapsed() < std::time::Duration::from_secs(60) {
                Some(cov.clone())
            } else {
                None
            }
        })
    };
    let coverage = match cached {
        Some(c) => c,
        None => {
            let lake_dir = state.config.lake_dir.clone();
            let fresh = tokio::task::spawn_blocking(move || {
                ObservationReader::new(PartitionResolver::new(lake_dir)).summarize_coverage()
            })
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            if let Ok(mut guard) = state.obs_catalog_cache.write() {
                *guard = Some((std::time::Instant::now(), fresh.clone()));
            }
            fresh
        }
    };

    let catalog = SeriesCatalog::new_canonical();
    Ok(Json(serde_json::json!({
        "success": true,
        "defined_series": catalog.list(),
        "ingested_coverage": coverage
    })))
}

pub async fn query_observations_matrix_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<MatrixQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let series_ids: Vec<String> = q
        .series_ids
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();
    for sid in &series_ids {
        validate_safe_identifier(sid, "series_ids")?;
    }
    let filter = ObservationFilter {
        series_ids,
        compartment: q.compartment,
        source: q.source,
        start_utc: parse_utc_opt(&q.start_date, false)?,
        end_utc: parse_utc_opt(&q.end_date, true)?,
        as_of_known_at: parse_utc_opt(&q.as_of_known_at, true)?,
        resolve_latest_revision: true,
        ..Default::default()
    };
    let resample = q.resample.unwrap_or_else(|| "1d".to_string()).to_ascii_lowercase();
    let ffill = q.forward_fill.unwrap_or(false);
    let fmt = q.format.unwrap_or_else(|| "json".to_string()).to_ascii_lowercase();

    let lake_dir = state.config.lake_dir.clone();
    let rows = tokio::task::spawn_blocking(move || {
        ObservationReader::new(PartitionResolver::new(lake_dir)).read_observations(&filter)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut cols: BTreeSet<String> = BTreeSet::new();
    let mut sources_by_series: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut grid: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();

    for r in rows {
        let bucket = match resample.as_str() {
            "1w" | "w" => align_to_friday_str(r.observation_ts_utc.date_naive()),
            _ => r.observation_ts_utc.format("%Y-%m-%d").to_string(),
        };
        cols.insert(r.series_id.clone());
        sources_by_series
            .entry(r.series_id.clone())
            .or_default()
            .insert(r.provenance.source.clone());
        grid.entry(bucket).or_default().insert(r.series_id, r.value);
    }

    // Fail-fast Source Purity Guardrail: refuse to silently stitch multiple upstream sources into 1 column.
    for (sid, srcs) in &sources_by_series {
        if srcs.len() > 1 {
            let list: Vec<&str> = srcs.iter().map(String::as_str).collect();
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "Source-mixing guardrail triggered for series_id='{}' (sources={:?}). Pass ?source=<name> to pin one origin.",
                    sid, list
                ),
            ));
        }
    }

    let catalog = SeriesCatalog::new_canonical();
    let limitation_warnings: BTreeMap<String, String> = cols
        .iter()
        .filter_map(|sid| {
            catalog
                .get(sid)
                .and_then(|meta| meta.limitation_note.as_ref().map(|n| (sid.clone(), n.clone())))
        })
        .collect();
    let frequency_quality_flags: BTreeMap<String, String> = cols
        .iter()
        .filter_map(|sid| {
            let meta = catalog.get(sid)?;
            let freq = meta.frequency.as_str();
            if (freq == "1M" || freq == "1Q") && (resample == "1w" || resample == "w" || resample == "1d") && ffill {
                Some((sid.clone(), format!("{}_step_carry_on_{}", freq, resample)))
            } else {
                None
            }
        })
        .collect();

    if ffill {
        let mut last_seen: BTreeMap<String, f64> = BTreeMap::new();
        for (_date, row_map) in grid.iter_mut() {
            for c in &cols {
                if let Some(&v) = row_map.get(c) {
                    last_seen.insert(c.clone(), v);
                } else if let Some(&prev) = last_seen.get(c) {
                    row_map.insert(c.clone(), prev);
                }
            }
        }
    }

    let col_list: Vec<String> = cols.into_iter().collect();
    if fmt == "csv" {
        let mut csv_out = format!("date,{}\n", col_list.join(","));
        for (date, map) in &grid {
            let vals: Vec<String> = col_list
                .iter()
                .map(|c| map.get(c).map(|v| format!("{}", v)).unwrap_or_default())
                .collect();
            csv_out.push_str(&format!("{},{}\n", date, vals.join(",")));
        }
        return Ok((
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
            csv_out,
        )
            .into_response());
    }

    let matrix_rows: Vec<serde_json::Value> = grid
        .into_iter()
        .map(|(date, vals)| serde_json::json!({ "date": date, "values": vals }))
        .collect();
    Ok(Json(serde_json::json!({
        "success": true,
        "resample": resample,
        "forward_fill": ffill,
        "columns": col_list,
        "sources_by_series": sources_by_series,
        "limitation_warnings": limitation_warnings,
        "frequency_quality_flags": frequency_quality_flags,
        "row_count": matrix_rows.len(),
        "rows": matrix_rows
    }))
    .into_response())
}

pub async fn trigger_observations_sync_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SyncQuery>,
) -> impl IntoResponse {
    let target = q.target.unwrap_or_else(|| "all".to_string());
    let report = sync_observations(&state.config, &target).await;
    if let Ok(mut guard) = state.obs_catalog_cache.write() {
        *guard = None;
    }
    Json(serde_json::json!({
        "success": true,
        "report": report
    }))
}

fn align_to_friday_str(d: NaiveDate) -> String {
    let days_from_mon = d.weekday().num_days_from_monday() as i64;
    let fri_offset = 4 - days_from_mon;
    let friday = if fri_offset >= 0 {
        d + Duration::days(fri_offset)
    } else {
        d + Duration::days(7 + fri_offset)
    };
    friday.format("%Y-%m-%d").to_string()
}
