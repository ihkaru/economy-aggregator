//! REST API serving endpoints for market candles, instruments catalog, and health.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::Key;
use chrono::NaiveDate;
use tower_http::services::{ServeDir, ServeFile};

use crate::core::config::LakehouseConfig;
use crate::core::macro_types::RichEconomicEvent;
use crate::core::prediction_types::PredictionMarketContract;
use crate::core::subscription::SubscriptionManager;
use crate::core::types::{AggregatorCandle, AssetClass, TimeFrame};
use crate::features::ingestion::orchestrator::IngestionOrchestrator;
use crate::features::serving::analytics_routes::*;
use crate::features::serving::auth_routes::{
    auth_login_handler, auth_logout_handler, auth_session_handler, oidc_callback_handler,
    oidc_login_handler,
};
use crate::features::serving::catalog_routes::*;
use crate::features::serving::liquidity_analytics_routes::{
    query_liquidity_panel_handler, query_liquidity_spillover_handler,
};
use crate::features::serving::macro_routes::query_macro_calendar_handler;
use crate::features::serving::observation_routes::{
    query_observations_catalog_handler, query_observations_handler,
    query_observations_matrix_handler, trigger_observations_sync_handler,
};
use crate::features::serving::prediction_routes::{
    query_prediction_candles_handler, query_prediction_contracts_handler,
};
use crate::features::serving::session::{derive_session_key, LoginRateLimiter, OidcConfig, UiAdminAccount};
use crate::features::serving::subscription_routes::*;
use crate::features::serving::types::*;
use crate::features::storage::{
    CandleResampler, InstrumentRegistry, LakeReader, PartitionResolver, SeriesCoverageSummary,
};

pub struct AppState {
    pub config: LakehouseConfig,
    pub resolver: PartitionResolver,
    pub registry: RwLock<InstrumentRegistry>,
    pub sub_manager: Arc<SubscriptionManager>,
    pub orchestrator: Arc<IngestionOrchestrator>,
    pub start_time: Instant,
    pub candle_cache: RwLock<HashMap<String, (Instant, Vec<AggregatorCandle>)>>,
    pub macro_cache: RwLock<HashMap<String, (Instant, Vec<RichEconomicEvent>)>>,
    pub prediction_contracts_cache: RwLock<HashMap<String, (Instant, Vec<PredictionMarketContract>)>>,
    pub catalog_cache: RwLock<Option<(Instant, Vec<InstrumentCatalogItem>)>>,
    pub partitions_cache: RwLock<Option<(Instant, Vec<PartitionItem>)>>,
    pub obs_catalog_cache: RwLock<Option<(Instant, Vec<SeriesCoverageSummary>)>>,
    pub api_keys: Vec<String>,
    pub ui_admin: Option<UiAdminAccount>,
    pub session_key: Key,
    pub oidc_config: Option<OidcConfig>,
    pub login_limiter: LoginRateLimiter,
}

impl AppState {
    pub fn new(
        config: LakehouseConfig,
        resolver: PartitionResolver,
        registry: InstrumentRegistry,
        sub_manager: Arc<SubscriptionManager>,
        orchestrator: Arc<IngestionOrchestrator>,
    ) -> Self {
        Self {
            config,
            resolver,
            registry: RwLock::new(registry),
            sub_manager,
            orchestrator,
            start_time: Instant::now(),
            candle_cache: RwLock::new(HashMap::new()),
            macro_cache: RwLock::new(HashMap::new()),
            prediction_contracts_cache: RwLock::new(HashMap::new()),
            catalog_cache: RwLock::new(None),
            partitions_cache: RwLock::new(None),
            obs_catalog_cache: RwLock::new(None),
            api_keys: Vec::new(),
            ui_admin: None,
            session_key: Key::generate(),
            oidc_config: None,
            login_limiter: LoginRateLimiter::default(),
        }
    }

    pub fn with_api_keys(mut self, api_keys: Vec<String>) -> Self {
        let fallback_secret = self
            .ui_admin
            .as_ref()
            .and_then(|a| a.password_hash.as_deref().or(a.plain_password.as_deref()));
        self.session_key = derive_session_key(None, &api_keys, fallback_secret);
        self.api_keys = api_keys;
        self
    }

    pub fn with_ui_auth(
        mut self,
        session_secret: Option<String>,
        ui_admin: Option<UiAdminAccount>,
        oidc_config: Option<OidcConfig>,
    ) -> Self {
        self.ui_admin = ui_admin;
        let fallback_secret = self
            .ui_admin
            .as_ref()
            .and_then(|a| a.password_hash.as_deref().or(a.plain_password.as_deref()));
        self.session_key = derive_session_key(
            session_secret.as_deref(),
            &self.api_keys,
            fallback_secret,
        );
        self.oidc_config = oidc_config;
        self
    }

    pub fn auth_required(&self) -> bool {
        !self.api_keys.is_empty() || self.ui_admin.is_some() || self.oidc_config.is_some()
    }
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let api_router = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/health", get(health_handler))
        .route("/v1/auth/session", get(auth_session_handler))
        .route("/v1/auth/login", post(auth_login_handler))
        .route("/v1/auth/logout", post(auth_logout_handler))
        .route("/v1/auth/oidc/login", get(oidc_login_handler))
        .route("/v1/auth/oidc/callback", get(oidc_callback_handler))
        .route("/v1/instruments", get(list_instruments_handler))
        .route("/v1/market/catalog", get(market_catalog_handler))
        .route("/v1/market/metadata", get(market_metadata_handler))
        .route("/v1/market/candles", get(query_candles_handler))
        .route("/v1/storage/stats", get(storage_stats_handler))
        .route("/v1/storage/partitions", get(storage_partitions_handler))
        .route("/v1/storage/compact", post(trigger_compact_handler))
        .route("/v1/macro/calendar", get(query_macro_calendar_handler))
        .route("/v1/observations", get(query_observations_handler))
        .route("/v1/observations/catalog", get(query_observations_catalog_handler))
        .route("/v1/observations/matrix", get(query_observations_matrix_handler))
        .route("/v1/observations/sync", post(trigger_observations_sync_handler))
        .route("/v1/prediction-markets/events", get(query_prediction_contracts_handler))
        .route("/v1/prediction-markets/markets/:id/candles", get(query_prediction_candles_handler))
        .route("/v1/analytics/divergence", get(query_divergence_handler))
        .route("/v1/analytics/liquidity-panel", get(query_liquidity_panel_handler))
        .route("/v1/analytics/liquidity-spillover", get(query_liquidity_spillover_handler))
        .route("/v1/integrity/audit", get(integrity_audit_handler))
        .route("/v1/subscriptions", get(list_subscriptions_handler).post(add_subscription_handler))
        .route("/v1/subscriptions/:symbol", delete(delete_subscription_handler))
        .route("/v1/ingestion/status", get(ingestion_status_handler))
        .route("/v1/ingestion/catch-up", post(trigger_catch_up_handler))
        .route("/v1/discovery/search", get(search_symbols_handler))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::features::serving::auth::api_key_auth_middleware,
        ))
        .layer(middleware::from_fn(
            crate::features::serving::auth::security_headers_middleware,
        ))
        .layer(tower_http::compression::CompressionLayer::new())
        .with_state(state);

    let web_dist_str = std::env::var("WEB_DIST_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "web/dist".to_string());
    let web_dist = Path::new(&web_dist_str);
    if web_dist.exists() {
        let serve_dir = ServeDir::new(web_dist)
            .not_found_service(ServeFile::new(web_dist.join("index.html")));
        api_router.fallback_service(serve_dir)
    } else {
        api_router
    }
}

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    Json(HealthResponse {
        status: "ok",
        service: "economy-aggregator",
        version: "0.1.0",
        uptime_secs: uptime,
    })
}

pub async fn query_candles_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CandleQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    crate::features::serving::auth::validate_safe_identifier(&query.symbol, "symbol")?;
    let asset_class = match query.asset_class.to_lowercase().as_str() {
        "crypto" => AssetClass::Crypto,
        "equities" => AssetClass::Equities,
        "forex" => AssetClass::Forex,
        "commodities" => AssetClass::Commodities,
        "macro" => AssetClass::Macro,
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid asset_class".to_string())),
    };
    let timeframe = match TimeFrame::from_str_opt(&query.timeframe) {
        Some(tf) => tf,
        None => return Err((StatusCode::BAD_REQUEST, "Invalid timeframe".to_string())),
    };

    let explicit_start = query.start_date.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let explicit_end = query.end_date.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let (start_date, end_date) = match (explicit_start, explicit_end) {
        (Some(s), Some(e)) => (Some(s), Some(e)),
        (Some(s), None) => (Some(s), None),
        (None, Some(e)) => (None, Some(e)),
        (None, None) => match (query.year, query.month) {
            (Some(y), Some(m)) => {
                let s = NaiveDate::from_ymd_opt(y, m, 1);
                let next_m = if m == 12 { 1 } else { m + 1 };
                let next_y = if m == 12 { y + 1 } else { y };
                let e = NaiveDate::from_ymd_opt(next_y, next_m, 1).and_then(|d| d.pred_opt());
                (s, e)
            }
            (Some(y), None) => {
                let s = NaiveDate::from_ymd_opt(y, 1, 1);
                let e = NaiveDate::from_ymd_opt(y, 12, 31);
                (s, e)
            }
            _ => (None, None),
        },
    };

    let src_tag = query.source.as_deref().unwrap_or("all");
    let clean_symbol = query.symbol.to_uppercase().replace('/', "_").replace(':', "_");
    let cache_key = format!("{}:{}:{}:{}", asset_class.as_str(), clean_symbol, timeframe.as_str(), src_tag);

    // Strict Memory Guardrail: limit cap <= 5000 (prevents OOM on runaway allocations)
    let capped_limit = Some(query.limit.unwrap_or(300).min(5000));

    // Check fast in-memory cache if no arbitrary historical date filter and no range cutoffs
    if start_date.is_none() && end_date.is_none() && query.to.is_none() && query.from.is_none() {
        if let Ok(cache) = state.candle_cache.read() {
            if let Some((inserted_at, cached_candles)) = cache.get(&cache_key) {
                if inserted_at.elapsed() < Duration::from_secs(60) {
                    let mut res = cached_candles.clone();
                    let lim = capped_limit.unwrap_or(300);
                    if res.len() > lim {
                        let start_idx = res.len() - lim;
                        res = res.split_off(start_idx);
                    }
                    return Ok(Json(res));
                }
            }
        }
    }

    let lake_dir = state.config.lake_dir.clone();
    let ac_str = asset_class.as_str().to_string();
    let sym_owned = query.symbol.clone();
    let tf_str = timeframe.as_str().to_string();
    let from_ts = query.from;
    let to_ts = query.to;

    let mut candles = tokio::task::spawn_blocking(move || {
        LakeReader::read_candles_with_limit_and_range(
            &lake_dir,
            Some(&ac_str),
            &sym_owned,
            &tf_str,
            start_date,
            end_date,
            capped_limit,
            from_ts,
            to_ts,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Dynamic Resampling: If candles are empty for weekly or monthly, resample from 1d daily lake
    if candles.is_empty() && (timeframe == TimeFrame::W1 || timeframe == TimeFrame::MN1) {
        let daily_limit = capped_limit.map(|l| if timeframe == TimeFrame::MN1 { (l * 31).min(5000) } else { (l * 7).min(5000) });
        if let Ok(daily_candles) = LakeReader::read_candles_with_limit_and_range(
            &state.config.lake_dir,
            Some(asset_class.as_str()),
            &query.symbol,
            "1d",
            start_date,
            end_date,
            daily_limit,
            query.from,
            query.to,
        ) {
            candles = CandleResampler::resample(&daily_candles, timeframe);
        }
    }

    if let Some(ref src_filter) = query.source {
        let target_src = src_filter.trim();
        if !target_src.is_empty() {
            candles.retain(|c| c.source.eq_ignore_ascii_case(target_src));
        }
    } else {
        let mut distinct_sources: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for c in &candles {
            distinct_sources.insert(c.source.as_str());
        }
        if distinct_sources.len() > 1 {
            let list: Vec<&str> = distinct_sources.into_iter().collect();
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "Source-mixing guardrail: symbol '{}' spans multiple sources {:?}. Pass ?source=<vendor> to prevent mixing.",
                    query.symbol, list
                ),
            ));
        }
    }

    // Cache resolved candles in memory if full initial view
    if start_date.is_none() && end_date.is_none() && query.to.is_none() && query.from.is_none() && !candles.is_empty() {
        if let Ok(mut cache) = state.candle_cache.write() {
            cache.insert(cache_key, (Instant::now(), candles.clone()));
        }
    }

    // Apply limit: keep the most recent candles for charting
    if let Some(lim) = capped_limit {
        if candles.len() > lim {
            let start_idx = candles.len() - lim;
            candles = candles.split_off(start_idx);
        }
    }

    Ok(Json(candles))
}
