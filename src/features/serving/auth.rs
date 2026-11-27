//! API key authentication middleware for the REST serving layer.
//!
//! Enforces constant-time API key verification on `/v1/*` routes (excluding `/v1/health`)
//! whenever one or more API keys are configured via `AGGREGATOR_API_KEY`,
//! `AGGREGATOR_API_KEYS`, or `ECONOMY_AGGREGATOR_API_KEY`.

use std::sync::Arc;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::PrivateCookieJar;
use chrono::Utc;

use crate::features::serving::session::{SessionPayload, SESSION_COOKIE_NAME};
use crate::features::serving::AppState;

/// Extracts a client identifier for rate limiting (prefers Cloudflare/Proxy IP headers).
pub fn extract_client_key_label(headers: &HeaderMap) -> String {
    for hdr in ["cf-connecting-ip", "x-forwarded-for", "x-real-ip"] {
        if let Some(val) = headers.get(hdr).and_then(|v| v.to_str().ok()) {
            let first = val.split(',').next().unwrap_or("").trim();
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Validates external identifier inputs against path traversal and control-character injection.
pub fn validate_safe_identifier(value: &str, field: &str) -> Result<(), (StatusCode, String)> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Invalid '{}': must be between 1 and 128 characters", field),
        ));
    }
    if trimmed.contains("..")
        || trimmed.contains('\\')
        || trimmed.bytes().any(|b| b == 0 || b < 0x20 || b == 0x7f)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Invalid '{}': forbidden path traversal or control characters", field),
        ));
    }
    Ok(())
}

/// Attaches defense-in-depth HTTP security headers on all responses.
pub async fn security_headers_middleware(req: Request<Body>, next: Next) -> Response {
    let is_api = req.uri().path().starts_with("/v1/");
    let mut res = next.run(req).await;
    let headers = res.headers_mut();

    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );
    if is_api {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        );
    }
    res
}

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

/// Returns true if the request path requires authentication when auth is enabled.
fn is_protected_path(path: &str) -> bool {
    if !path.starts_with("/v1/") {
        return false;
    }
    !matches!(
        path,
        "/v1/health"
            | "/v1/auth/session"
            | "/v1/auth/login"
            | "/v1/auth/logout"
            | "/v1/auth/oidc/login"
            | "/v1/auth/oidc/callback"
    )
}

fn is_state_changing_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    )
}

/// Axum middleware enforcing API key OR AEAD-encrypted HttpOnly cookie session (+ CSRF) on `/v1/*`.
pub async fn api_key_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !state.auth_required() || !is_protected_path(req.uri().path()) {
        return next.run(req).await;
    }

    // 1. Machine-to-machine / CLI auth via Bearer or X-API-Key header
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

    // 2. Browser UI auth via AEAD-encrypted HttpOnly cookie (`ea_session`) + CSRF synchronizer token
    let jar = PrivateCookieJar::from_headers(req.headers(), state.session_key.clone());
    if let Some(cookie) = jar.get(SESSION_COOKIE_NAME) {
        if let Ok(session) = serde_json::from_str::<SessionPayload>(cookie.value()) {
            if !session.is_expired(Utc::now().timestamp()) {
                if is_state_changing_method(req.method()) {
                    let header_csrf = req
                        .headers()
                        .get("x-csrf-token")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if !constant_time_eq(session.csrf_token.as_bytes(), header_csrf.as_bytes()) {
                        let body = serde_json::json!({
                            "success": false,
                            "error": "Forbidden: missing or invalid X-CSRF-Token header",
                            "data": null
                        });
                        return (StatusCode::FORBIDDEN, Json(body)).into_response();
                    }
                }
                return next.run(req).await;
            }
        }
    }

    let body = serde_json::json!({
        "success": false,
        "error": "Unauthorized: sign in via UI session or provide 'Authorization: Bearer <key>' / 'X-API-Key'",
        "data": null
    });
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}
