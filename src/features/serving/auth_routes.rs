//! Browser UI authentication endpoints (`/v1/auth/session`, `/v1/auth/login`,
//! `/v1/auth/logout`, and `/v1/auth/oidc/*` PKCE SSO).

use std::sync::Arc;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};
use chrono::Utc;
use serde::Deserialize;

use crate::features::serving::auth::{constant_time_eq, extract_client_key_label};
use crate::features::serving::rest::AppState;
use crate::features::serving::session::{
    build_clear_session_cookie, build_session_cookie, generate_random_token, is_https_request,
    pkce_s256_challenge, OidcTxState, SessionPayload, OIDC_TX_COOKIE_NAME, SESSION_COOKIE_NAME,
    SESSION_TTL_SECS,
};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    #[serde(default)]
    pub username: String,
    #[serde(default, alias = "credential")]
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

fn read_valid_session(jar: &PrivateCookieJar) -> Option<SessionPayload> {
    let cookie = jar.get(SESSION_COOKIE_NAME)?;
    let payload: SessionPayload = serde_json::from_str(cookie.value()).ok()?;
    if payload.is_expired(Utc::now().timestamp()) {
        return None;
    }
    Some(payload)
}

/// `GET /v1/auth/session` — returns non-secret browser session metadata & in-memory CSRF token.
pub async fn auth_session_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let jar = PrivateCookieJar::from_headers(&headers, state.session_key.clone());
    let auth_required = state.auth_required();

    if let Some(session) = read_valid_session(&jar) {
        return Json(serde_json::json!({
            "authenticated": true,
            "auth_required": auth_required,
            "csrf_token": session.csrf_token,
            "expires_at": session.expires_at,
            "user": {
                "sub": session.sub,
                "name": session.name,
                "auth_method": session.auth_method
            }
        }));
    }

    Json(serde_json::json!({
        "authenticated": !auth_required,
        "auth_required": auth_required,
        "csrf_token": null,
        "expires_at": null,
        "user": null
    }))
}

/// `POST /v1/auth/login` — authenticates the standalone UI admin account (Argon2id + username)
/// and exchanges it for an AEAD-encrypted `HttpOnly` cookie session (`ea_session`).
/// Strictly separate from machine-to-machine `AGGREGATOR_API_KEY` when `UiAdminAccount` is configured.
pub async fn auth_login_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Response {
    let jar = PrivateCookieJar::from_headers(&headers, state.session_key.clone());
    let client_ip = extract_client_key_label(&headers);
    if let Err(retry_secs) = state.login_limiter.check_allowed(&client_ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, retry_secs.to_string())],
            Json(serde_json::json!({
                "success": false,
                "error": format!("Too many failed login attempts. Try again in {}s.", retry_secs)
            })),
        )
            .into_response();
    }

    let password = req.password.trim();
    if password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": "Password cannot be empty" })),
        )
            .into_response();
    }

    let (authorized, sub, display_name) = if let Some(ref admin) = state.ui_admin {
        let user_input = if req.username.trim().is_empty() {
            "admin"
        } else {
            req.username.trim()
        };
        let ok = admin.verify(user_input, password);
        (ok, admin.username.clone(), Some(admin.username.clone()))
    } else if !state.auth_required() {
        (true, "operator".to_string(), Some("Operator".to_string()))
    } else {
        let ok = state
            .api_keys
            .iter()
            .any(|k| constant_time_eq(k.as_bytes(), password.as_bytes()));
        (ok, "operator".to_string(), Some("Operator".to_string()))
    };

    if !authorized {
        state.login_limiter.record_failure(&client_ip);
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "success": false, "error": "Invalid username or password" })),
        )
            .into_response();
    }

    state.login_limiter.clear(&client_ip);
    issue_session_response(jar, &headers, sub, display_name, "local_admin")
}

/// `POST /v1/auth/logout` — clears the `HttpOnly` session cookie.
pub async fn auth_logout_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let jar = PrivateCookieJar::from_headers(&headers, state.session_key.clone());
    if let Some(session) = read_valid_session(&jar) {
        let header_csrf = headers.get("x-csrf-token").and_then(|v| v.to_str().ok()).unwrap_or("");
        if !constant_time_eq(session.csrf_token.as_bytes(), header_csrf.as_bytes()) {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "success": false, "error": "Invalid CSRF token" })),
            )
                .into_response();
        }
    }
    let secure = is_https_request(&headers);
    let updated = jar.remove(build_clear_session_cookie(secure));
    (updated, Json(serde_json::json!({ "success": true, "authenticated": false }))).into_response()
}

/// `GET /v1/auth/oidc/login` — starts Authorization Code + PKCE S256 flow against rIDM / OIDC IdP.
pub async fn oidc_login_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    let jar = PrivateCookieJar::from_headers(&headers, state.session_key.clone());
    let cfg = match &state.oidc_config {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "success": false, "error": "OIDC SSO is not configured" })),
            )
                .into_response();
        }
    };

    let discovery_url = format!("{}/.well-known/openid-configuration", cfg.issuer);
    let auth_endpoint = match reqwest::Client::new().get(&discovery_url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(v) => v.get("authorization_endpoint").and_then(|s| s.as_str()).map(|s| s.to_string()),
            Err(_) => None,
        },
        _ => None,
    }
    .unwrap_or_else(|| format!("{}/protocol/openid-connect/auth", cfg.issuer));

    let state_tok = generate_random_token();
    let nonce = generate_random_token();
    let verifier = generate_random_token();
    let challenge = pkce_s256_challenge(&verifier);

    let tx = OidcTxState {
        state: state_tok.clone(),
        nonce: nonce.clone(),
        code_verifier: verifier,
        expires_at: Utc::now().timestamp() + 300,
    };
    let tx_json = match serde_json::to_string(&tx) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let mut tx_cookie = Cookie::new(OIDC_TX_COOKIE_NAME, tx_json);
    tx_cookie.set_http_only(true);
    tx_cookie.set_secure(is_https_request(&headers));
    tx_cookie.set_same_site(SameSite::Lax);
    tx_cookie.set_path("/");
    tx_cookie.set_max_age(time::Duration::seconds(300));

    let auth_url = reqwest::Url::parse_with_params(
        &auth_endpoint,
        &[
            ("response_type", "code"),
            ("client_id", cfg.client_id.as_str()),
            ("redirect_uri", cfg.redirect_uri.as_str()),
            ("scope", "openid profile email"),
            ("state", state_tok.as_str()),
            ("nonce", nonce.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
        ],
    )
    .map(|u| u.to_string())
    .unwrap_or_else(|_| "/".to_string());

    (jar.add(tx_cookie), Redirect::temporary(&auth_url)).into_response()
}

/// `GET /v1/auth/oidc/callback` — validates PKCE/state, exchanges code, fetches userinfo, sets `ea_session`.
pub async fn oidc_callback_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<OidcCallbackQuery>,
) -> Response {
    let jar = PrivateCookieJar::from_headers(&headers, state.session_key.clone());
    let cfg = match &state.oidc_config {
        Some(c) => c,
        None => return (StatusCode::NOT_FOUND, "OIDC not configured").into_response(),
    };
    if q.error.is_some() {
        return Redirect::temporary("/?auth_error=oidc_denied").into_response();
    }
    let (Some(code), Some(cb_state)) = (q.code, q.state) else {
        return (StatusCode::BAD_REQUEST, "Missing OIDC code or state").into_response();
    };
    let tx: OidcTxState = match jar
        .get(OIDC_TX_COOKIE_NAME)
        .and_then(|c| serde_json::from_str(c.value()).ok())
    {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Missing OIDC transaction cookie").into_response(),
    };
    if tx.expires_at < Utc::now().timestamp() || !constant_time_eq(tx.state.as_bytes(), cb_state.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "Invalid or expired OIDC state").into_response();
    }

    let http = reqwest::Client::new();
    let disc: serde_json::Value = match http
        .get(format!("{}/.well-known/openid-configuration", cfg.issuer))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
        _ => serde_json::Value::Null,
    };

    let token_ep = disc
        .get("token_endpoint")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/protocol/openid-connect/token", cfg.issuer));
    let userinfo_ep = disc
        .get("userinfo_endpoint")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/protocol/openid-connect/userinfo", cfg.issuer));

    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", cfg.redirect_uri.as_str()),
        ("client_id", cfg.client_id.as_str()),
        ("code_verifier", tx.code_verifier.as_str()),
    ];
    if let Some(ref secret) = cfg.client_secret {
        form.push(("client_secret", secret.as_str()));
    }

    let token_res: serde_json::Value = match http.post(&token_ep).form(&form).send().await {
        Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
        _ => return (StatusCode::BAD_GATEWAY, "OIDC token exchange failed").into_response(),
    };
    let Some(access_token) = token_res.get("access_token").and_then(|v| v.as_str()) else {
        return (StatusCode::BAD_GATEWAY, "OIDC access_token missing").into_response();
    };
    let userinfo: serde_json::Value = match http.get(&userinfo_ep).bearer_auth(access_token).send().await {
        Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
        _ => return (StatusCode::BAD_GATEWAY, "OIDC userinfo fetch failed").into_response(),
    };

    let sub = userinfo.get("sub").and_then(|v| v.as_str()).unwrap_or("oidc-user").to_string();
    let name = userinfo
        .get("name")
        .or_else(|| userinfo.get("preferred_username"))
        .or_else(|| userinfo.get("email"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let now = Utc::now().timestamp();
    let session = SessionPayload {
        sub,
        name,
        auth_method: "oidc".to_string(),
        csrf_token: generate_random_token(),
        issued_at: now,
        expires_at: now + SESSION_TTL_SECS,
    };
    let encoded = serde_json::to_string(&session).unwrap_or_default();
    let secure = is_https_request(&headers);
    let updated = jar
        .remove(Cookie::from(OIDC_TX_COOKIE_NAME))
        .add(build_session_cookie(encoded, secure, SESSION_TTL_SECS));
    (updated, Redirect::temporary("/")).into_response()
}

fn issue_session_response(
    jar: PrivateCookieJar,
    headers: &HeaderMap,
    sub: String,
    name: Option<String>,
    method: &str,
) -> Response {
    let now = Utc::now().timestamp();
    let session = SessionPayload {
        sub: sub.clone(),
        name: name.clone(),
        auth_method: method.to_string(),
        csrf_token: generate_random_token(),
        issued_at: now,
        expires_at: now + SESSION_TTL_SECS,
    };
    let encoded = match serde_json::to_string(&session) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let secure = is_https_request(headers);
    let updated = jar.add(build_session_cookie(encoded, secure, SESSION_TTL_SECS));
    (
        updated,
        Json(serde_json::json!({
            "success": true,
            "authenticated": true,
            "csrf_token": session.csrf_token,
            "expires_at": session.expires_at,
            "user": { "sub": sub, "name": name, "auth_method": method }
        })),
    )
        .into_response()
}
