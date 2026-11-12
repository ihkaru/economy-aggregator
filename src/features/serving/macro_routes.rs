//! REST endpoints for Macroeconomic Calendar events and point-in-time statistics.

use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::features::ingestion::economic_calendar::EconomicCalendarFetcher;
use crate::features::serving::rest::AppState;
use crate::features::storage::LakeMacroReader;

#[derive(Debug, Deserialize)]
pub struct MacroCalendarQuery {
    pub country: Option<String>,
    pub importance: Option<String>,
    pub indicator: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub source: Option<String>,
    pub limit: Option<usize>,
    pub refresh: Option<bool>,
}

pub async fn query_macro_calendar_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MacroCalendarQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let country = query.country.as_deref().unwrap_or("US");
    let importance = query.importance.as_deref();
    let source = query.source.as_deref().unwrap_or("lake");
    let force_refresh = query.refresh.unwrap_or(false);
    let limit = query.limit.unwrap_or(100).min(2000);

    let from_d = query.from_date.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let to_d = query.to_date.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let cache_key = format!(
        "{}:{}:{}:{}:{}:{}",
        country,
        importance.unwrap_or("all"),
        query.indicator.as_deref().unwrap_or("all"),
        query.year.unwrap_or(0),
        query.month.unwrap_or(0),
        source
    );

    // 1. Check in-memory cache (TTL: 5 minutes) if no dynamic date range
    if !force_refresh && from_d.is_none() && to_d.is_none() {
        if let Ok(cache_guard) = state.macro_cache.read() {
            if let Some((inserted, events)) = cache_guard.get(&cache_key) {
                if inserted.elapsed() < Duration::from_secs(300) {
                    let sliced = events.iter().take(limit).cloned().collect::<Vec<_>>();
                    return Ok(Json(sliced));
                }
            }
        }
    }

    // 2. Lakehouse Mode: Read directly from partitioned Parquet files
    if source == "lake" || source == "hybrid" {
        let lake_events = LakeMacroReader::read_events(
            &state.config.lake_dir,
            Some(country),
            query.year,
            query.month,
            query.indicator.as_deref(),
            from_d,
            to_d,
            importance,
            Some(limit),
        ).unwrap_or_default();

        if !lake_events.is_empty() || source == "lake" {
            if from_d.is_none() && to_d.is_none() {
                if let Ok(mut cache_guard) = state.macro_cache.write() {
                    cache_guard.insert(cache_key, (Instant::now(), lake_events.clone()));
                }
            }
            return Ok(Json(lake_events));
        }
    }

    // 3. Live Provider Mode (or fallback when hybrid has no lake records)
    let fetcher = EconomicCalendarFetcher::default();
    match fetcher.fetch_rich_events(Some(country), importance).await {
        Ok(events) => {
            let _ = EconomicCalendarFetcher::persist_to_lake(&events, &state.resolver);

            if let Ok(mut cache_guard) = state.macro_cache.write() {
                cache_guard.insert(cache_key, (Instant::now(), events.clone()));
            }

            let sliced = events.into_iter().take(limit).collect::<Vec<_>>();
            Ok(Json(sliced))
        }
        Err(err) => {
            // Final fallback: try lakehouse reader if live fails
            let fallback_events = LakeMacroReader::read_events(
                &state.config.lake_dir,
                Some(country),
                query.year,
                query.month,
                query.indicator.as_deref(),
                from_d,
                to_d,
                importance,
                Some(limit),
            ).unwrap_or_default();

            if !fallback_events.is_empty() {
                return Ok(Json(fallback_events));
            }

            Err((
                StatusCode::BAD_GATEWAY,
                format!("Failed to fetch macroeconomic calendar: {}", err),
            ))
        }
    }
}
