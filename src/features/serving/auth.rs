//! API key authentication middleware for the REST serving layer.
//!
//! Enforces constant-time API key verification on `/v1/*` routes (excluding `/v1/health`)
//! whenever one or more API keys are configured via `AGGREGATOR_API_KEY`,
//! `AGGREGATOR_API_KEYS`, or `ECONOMY_AGGREGATOR_API_KEY`.

use std::sync::Arc;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::features::serving::AppState;

/// Constant-time byte slice equality check to prevent timing side-channels.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Loads configured API keys from environment variables.
/// Supports comma-separated lists for zero-downtime key rotation.
pub fn load_api_keys_from_env() -> Vec<String> {
    let mut keys = Vec::new();
    for var in [
        "ECONOMY_AGGREGATOR_API_KEY",
        "AGGREGATOR_API_KEY",
        "AGGREGATOR_API_KEYS",
    ] {
        if let Ok(raw) = std::env::var(var) {
            for part in raw.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() && !keys.iter().any(|k| k == trimmed) {
                    keys.push(trimmed.to_string());
                }
            }
        }
    }
    keys
}

/// Extracts a candidate API key from `Authorization: Bearer <key>` or `X-API-Key: <key>`.
fn extract_request_api_key(req: &Request<Body>) -> Option<&str> {
    if let Some(val) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(s) = val.to_str() {
            let trimmed = s.trim();
            if let Some(token) = trimmed
                .strip_prefix("Bearer ")
                .or_else(|| trimmed.strip_prefix("bearer "))
            {
                let t = token.trim();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }

    if let Some(val) = req.headers().get("x-api-key") {
        if let Ok(s) = val.to_str() {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }

    None
}

/// Returns true if the request path requires API key verification when keys are configured.
fn is_protected_path(path: &str) -> bool {
    path.starts_with("/v1/") && path != "/v1/health"
}

/// Axum middleware enforcing API key authentication on protected `/v1/*` endpoints.
pub async fn api_key_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if state.api_keys.is_empty() || !is_protected_path(req.uri().path()) {
        return next.run(req).await;
    }

    if let Some(provided) = extract_request_api_key(&req) {
        let provided_bytes = provided.as_bytes();
        let authorized = state
            .api_keys
            .iter()
            .any(|expected| constant_time_eq(expected.as_bytes(), provided_bytes));
        if authorized {
            return next.run(req).await;
        }
    }

    let body = serde_json::json!({
        "success": false,
        "error": "Unauthorized: valid API key required via 'Authorization: Bearer <key>' or 'X-API-Key' header",
        "data": null
    });
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}
