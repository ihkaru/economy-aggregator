//! Catalog and Metadata REST route handlers for market instruments.

use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::features::serving::rest::AppState;
use crate::features::serving::types::*;
use crate::features::storage::{
    friendly_symbol_name, resolve_instrument_specs, resolve_instrument_vendor, LakeReader,
};

pub async fn list_instruments_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::features::storage::InstrumentMapping>> {
    let reg = state.registry.read().unwrap();
    Json(reg.list_all())
}

pub async fn market_catalog_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CatalogQuery>,
) -> Json<Vec<InstrumentCatalogItem>> {
    // 1. Check in-memory catalog cache (TTL: 60s) to eliminate fs::read_dir on every request
    let all_items = {
        let cached = if let Ok(guard) = state.catalog_cache.read() {
            guard.as_ref().and_then(|(t, items)| {
                if t.elapsed() < std::time::Duration::from_secs(60) {
                    Some(items.clone())
                } else {
                    None
                }
            })
        } else {
            None
        };

        if let Some(c) = cached {
            c
        } else {
            let fresh = scan_catalog_items(&state);
            if let Ok(mut guard) = state.catalog_cache.write() {
                *guard = Some((std::time::Instant::now(), fresh.clone()));
            }
            fresh
        }
    };

    // 2. Apply query filters
    let mut filtered = all_items;

    if let Some(q) = query.q.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        filtered.retain(|item| {
            item.symbol.to_lowercase().contains(&q)
                || item.canonical_symbol.to_lowercase().contains(&q)
                || item.name.to_lowercase().contains(&q)
        });
    }

    if let Some(ac) = query.asset_class.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        filtered.retain(|item| item.asset_class.to_lowercase() == ac);
    }

    if let Some(src) = query.source.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        filtered.retain(|item| item.source.to_lowercase() == src);
    }

    // 3. Apply pagination
    if let Some(offset) = query.offset {
        if offset < filtered.len() {
            filtered = filtered.split_off(offset);
        } else {
            filtered.clear();
        }
    }

    if let Some(limit) = query.limit {
        filtered.truncate(limit.min(5000));
    }

    Json(filtered)
}

fn scan_catalog_items(state: &AppState) -> Vec<InstrumentCatalogItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    let reg = state.registry.read().unwrap();
    for m in reg.list_all() {
        let src = m
            .vendor_symbols
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "lakehouse".to_string());
        seen.insert(m.canonical_symbol.clone());
        let (currency, precision, min_tick, supported_tf, _) =
            resolve_instrument_specs(&m.canonical_symbol, m.asset_class.as_str());
        items.push(InstrumentCatalogItem {
            symbol: m.canonical_symbol.clone(),
            canonical_symbol: m.canonical_symbol.clone(),
            name: m.name.clone(),
            asset_class: m.asset_class.as_str().to_string(),
            currency,
            price_precision: precision,
            min_tick,
            supported_timeframes: supported_tf,
            default_timeframe: "1d".to_string(),
            source: src,
            active: true,
        });
    }

    let market_dir = state.config.lake_dir.join("market");
    if let Ok(ac_entries) = fs::read_dir(market_dir) {
        for ac_entry in ac_entries.flatten() {
            let ac_path = ac_entry.path();
            if ac_path.is_dir() {
                let ac_name = ac_path.file_name().unwrap_or_default().to_string_lossy();
                if let Some(ac_stripped) = ac_name.strip_prefix("asset_class=") {
                    if let Ok(sym_entries) = fs::read_dir(&ac_path) {
                        for sym_entry in sym_entries.flatten() {
                            let sym_name = sym_entry.file_name().to_string_lossy().to_string();
                            if let Some(sym_stripped) = sym_name.strip_prefix("symbol=") {
                                let canonical = sym_stripped.replace('_', "/");
                                if seen.insert(canonical.clone()) {
                                    let friendly_name = friendly_symbol_name(sym_stripped);
                                    let (currency, precision, min_tick, supported_tf, _) =
                                        resolve_instrument_specs(&canonical, ac_stripped);
                                    let vendor = resolve_instrument_vendor(&canonical, ac_stripped);
                                    items.push(InstrumentCatalogItem {
                                        symbol: sym_stripped.to_string(),
                                        canonical_symbol: canonical,
                                        name: friendly_name.to_string(),
                                        asset_class: ac_stripped.to_string(),
                                        currency,
                                        price_precision: precision,
                                        min_tick,
                                        supported_timeframes: supported_tf,
                                        default_timeframe: "1d".to_string(),
                                        source: vendor.to_string(),
                                        active: true,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    items
}

pub async fn market_metadata_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MetadataQuery>,
) -> Result<Json<InstrumentMetadata>, (StatusCode, String)> {
    let canonical = query.symbol.replace('_', "/");
    let asset_class = query.asset_class.unwrap_or_else(|| "equities".to_string());
    let (currency, price_precision, min_tick, supported_tf, has_intraday) =
        resolve_instrument_specs(&canonical, &asset_class);

    let clean_symbol = query.symbol.to_uppercase().replace('/', "_");
    let bounds = LakeReader::read_metadata_bounds(
        &state.config.lake_dir,
        Some(&asset_class),
        &clean_symbol,
        "1d",
    ).unwrap_or((0, None, None));

    let friendly_name = friendly_symbol_name(&clean_symbol);

    Ok(Json(InstrumentMetadata {
        symbol: clean_symbol,
        canonical_symbol: canonical,
        name: friendly_name.to_string(),
        asset_class,
        currency,
        price_precision,
        min_tick,
        supported_timeframes: supported_tf,
        default_timeframe: "1d".to_string(),
        has_intraday,
        has_daily: bounds.0 > 0,
        has_weekly_and_monthly: bounds.0 > 0,
        total_bars: bounds.0,
        first_date: bounds.1,
        last_date: bounds.2,
    }))
}

pub async fn storage_stats_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StorageStatsResponse>, (StatusCode, String)> {
    let lake_dir = &state.config.lake_dir;
    let mut total_chunks = 0;
    let mut total_bytes = 0;

    fn visit_dirs(dir: &std::path::Path, chunks: &mut usize, bytes: &mut u64) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, chunks, bytes);
                } else if path.extension().and_then(|s| s.to_str()) == Some("parquet") {
                    *chunks += 1;
                    if let Ok(meta) = entry.metadata() {
                        *bytes += meta.len();
                    }
                }
            }
        }
    }

    visit_dirs(lake_dir, &mut total_chunks, &mut total_bytes);

    Ok(Json(StorageStatsResponse {
        lake_dir: lake_dir.display().to_string(),
        total_chunks_found: total_chunks,
        total_bytes,
    }))
}
