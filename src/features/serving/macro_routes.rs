//! REST endpoints for Macroeconomic Calendar events and point-in-time statistics.

use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::features::ingestion::economic_calendar::EconomicCalendarFetcher;
use crate::features::serving::rest::AppState;
use crate::features::storage::macro_arrow_schema::record_batch_to_macro_events;

#[derive(Debug, Deserialize)]
pub struct MacroCalendarQuery {
    pub country: Option<String>,
    pub importance: Option<String>,
    pub limit: Option<usize>,
    pub refresh: Option<bool>,
}

pub async fn query_macro_calendar_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MacroCalendarQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let country = query.country.as_deref().unwrap_or("US");
    let importance = query.importance.as_deref();
    let cache_key = format!("{}:{}", country, importance.unwrap_or("all"));
    let force_refresh = query.refresh.unwrap_or(false);

    // 1. Check in-memory cache (TTL: 5 minutes)
    if !force_refresh {
        let cache_guard = state.macro_cache.read().map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Macro cache lock error: {}", e))
        })?;

        if let Some((inserted, events)) = cache_guard.get(&cache_key) {
            if inserted.elapsed() < Duration::from_secs(300) {
                let limit = query.limit.unwrap_or(events.len());
                let sliced = events.iter().take(limit).cloned().collect::<Vec<_>>();
                return Ok(Json(sliced));
            }
        }
    }

    // 2. Fetch live from economic calendar provider
    let fetcher = EconomicCalendarFetcher::default();
    match fetcher.fetch_rich_events(Some(country), importance).await {
        Ok(events) => {
            // Persist to Parquet lakehouse asynchronously/inline
            let _ = EconomicCalendarFetcher::persist_to_lake(&events, &state.resolver);

            // Update in-memory cache
            if let Ok(mut cache_guard) = state.macro_cache.write() {
                cache_guard.insert(cache_key, (Instant::now(), events.clone()));
            }

            let limit = query.limit.unwrap_or(events.len());
            let sliced = events.into_iter().take(limit).collect::<Vec<_>>();
            Ok(Json(sliced))
        }
        Err(err) => {
            // Fallback: Read from local Parquet lakehouse if network fails
            let country_dir = state.resolver.resolve_macro_partition(country, 2026, 11);
            if country_dir.exists() {
                if let Ok(entries) = fs::read_dir(&country_dir) {
                    let mut fallback_events = Vec::new();
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("parquet") {
                            if let Ok(file) = fs::File::open(&path) {
                                if let Ok(reader) = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file) {
                                    if let Ok(batch_reader) = reader.build() {
                                        for batch in batch_reader.flatten() {
                                            if let Ok(mut evts) = record_batch_to_macro_events(&batch) {
                                                fallback_events.append(&mut evts);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !fallback_events.is_empty() {
                        let limit = query.limit.unwrap_or(fallback_events.len());
                        let sliced = fallback_events.into_iter().take(limit).collect::<Vec<_>>();
                        return Ok(Json(sliced));
                    }
                }
            }

            Err((
                StatusCode::BAD_GATEWAY,
                format!("Failed to fetch macroeconomic calendar: {}", err),
            ))
        }
    }
}
