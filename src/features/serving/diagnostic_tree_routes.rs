//! REST API Routes for Deterministic Liquidity Diagnostic Tree, Event-Sourced Transitions,
//! Continuous Analytics Supervisor & Frozen-Protocol Prospective OOS Scorecard (Phases 349 & 356).

use std::sync::Arc;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;

use crate::features::analytics::{
    detect_regime_transition, evaluate_liquidity_tree, evaluate_prospective_scorecard,
};
use crate::features::serving::auth::validate_safe_identifier;
use crate::features::serving::rest::AppState;
use crate::features::storage::{ContinuousAnalyticsStore, DiagnosticTreeLedger};
use crate::runtime::{build_prospective_inputs, build_tree_inputs};

#[derive(Debug, Deserialize, Default)]
pub struct DiagnosticTreeQuery {
    pub as_of: Option<String>,
    pub window_weeks: Option<usize>,
    pub persist: Option<bool>,
    pub include_spillover: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
    pub window_weeks: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ContinuousRunQuery {
    pub as_of: Option<String>,
    pub force: Option<bool>,
    pub include_spillover: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ProspectiveQuery {
    pub as_of: Option<String>,
    pub persist: Option<bool>,
    pub include_spillover: Option<bool>,
}

fn parse_as_of(raw: Option<&str>) -> Result<NaiveDate, (StatusCode, String)> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(Utc::now().date_naive()),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| (StatusCode::BAD_REQUEST, format!("Invalid as_of '{}', expected YYYY-MM-DD", s))),
    }
}

pub async fn query_liquidity_tree_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DiagnosticTreeQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let as_of = parse_as_of(q.as_of.as_deref())?;
    let window_weeks = q.window_weeks.unwrap_or(4).clamp(1, 52);
    let persist = q.persist.unwrap_or(false);
    let include_spillover = q.include_spillover.unwrap_or(true);
    let lake_dir = state.config.lake_dir.clone();

    let (report, transition_id) = tokio::task::spawn_blocking(move || {
        let inputs = build_tree_inputs(&lake_dir, as_of, window_weeks, include_spillover);
        let mut rep = evaluate_liquidity_tree(inputs);
        let mut tid = None;
        if persist {
            let ledger = DiagnosticTreeLedger::new(&lake_dir);
            let prev = ledger.latest_run_for_window(window_weeks).ok().flatten();
            let path = ledger.persist_run(&rep)?;
            rep.persisted_path = Some(path.display().to_string());
            if let Some(ev) = detect_regime_transition(prev.as_ref(), &rep) {
                let store = ContinuousAnalyticsStore::new(&lake_dir);
                let _ = store.persist_transition(&ev);
                tid = Some(ev.transition_id);
            }
        }
        Ok::<_, String>((rep, tid))
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "transition_id": transition_id,
        "diagnostic_tree": report
    })))
}

pub async fn list_liquidity_tree_history_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let limit = q.limit.unwrap_or(20).clamp(1, 200);
    let lake_dir = state.config.lake_dir.clone();
    let runs = tokio::task::spawn_blocking(move || DiagnosticTreeLedger::new(&lake_dir).list_runs(limit))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({ "success": true, "count": runs.len(), "runs": runs })))
}

pub async fn get_liquidity_tree_run_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    validate_safe_identifier(&run_id, "run_id")?;
    let lake_dir = state.config.lake_dir.clone();
    let found = tokio::task::spawn_blocking(move || DiagnosticTreeLedger::new(&lake_dir).get_run(&run_id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    match found {
        Some(rep) => Ok(Json(serde_json::json!({ "success": true, "diagnostic_tree": rep }))),
        None => Err((StatusCode::NOT_FOUND, "Diagnostic tree run_id not found".to_string())),
    }
}

pub async fn list_liquidity_tree_transitions_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let limit = q.limit.unwrap_or(30).clamp(1, 500);
    let win = q.window_weeks;
    let lake_dir = state.config.lake_dir.clone();
    let events = tokio::task::spawn_blocking(move || {
        ContinuousAnalyticsStore::new(&lake_dir).list_transitions(win, limit)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "count": events.len(),
        "transitions": events
    })))
}

pub async fn query_prospective_scorecard_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ProspectiveQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let as_of = parse_as_of(q.as_of.as_deref())?;
    let persist = q.persist.unwrap_or(false);
    let include_spillover = q.include_spillover.unwrap_or(true);
    let lake_dir = state.config.lake_dir.clone();

    let report = tokio::task::spawn_blocking(move || {
        let inputs = build_prospective_inputs(&lake_dir, as_of, include_spillover);
        let mut rep = evaluate_prospective_scorecard(&inputs);
        if persist {
            let store = ContinuousAnalyticsStore::new(&lake_dir);
            let p = store.persist_scorecard(&rep)?;
            rep.persisted_path = Some(p.display().to_string());
        }
        Ok::<_, String>(rep)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({ "success": true, "prospective_scorecard": report })))
}

pub async fn query_continuous_status_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let status = state.continuous_supervisor.status();
    Ok(Json(serde_json::json!({ "success": true, "continuous_analytics": status })))
}

pub async fn trigger_continuous_run_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ContinuousRunQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let as_of = match q.as_of.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(s) => Some(
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| (StatusCode::BAD_REQUEST, format!("Invalid as_of '{}', expected YYYY-MM-DD", s)))?,
        ),
    };
    let force = q.force.unwrap_or(false);
    let include_spillover = q.include_spillover.unwrap_or(true);

    let outcome = state
        .continuous_supervisor
        .run_cycle(as_of, force, include_spillover)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({ "success": true, "outcome": outcome })))
}
