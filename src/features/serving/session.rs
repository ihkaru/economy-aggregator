//! Production browser session management using AEAD-encrypted HttpOnly cookies
//! (`axum-extra` PrivateCookieJar), CSRF synchronizer tokens, and per-IP rate limiting.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use axum::http::{header, HeaderMap};
use axum_extra::extract::cookie::{Cookie, Key, SameSite};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SESSION_COOKIE_NAME: &str = "ea_session";
pub const OIDC_TX_COOKIE_NAME: &str = "ea_oidc_tx";
pub const SESSION_TTL_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPayload {
    pub sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub auth_method: String,
    pub csrf_token: String,
    pub issued_at: i64,
    pub expires_at: i64,
}

impl SessionPayload {
    pub fn is_expired(&self, now_ts: i64) -> bool {
        self.expires_at <= now_ts
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcTxState {
    pub state: String,
    pub nonce: String,
    pub code_verifier: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub provider_name: String,
}

impl OidcConfig {
    pub fn from_env() -> Option<Self> {
        let issuer = std::env::var("OIDC_ISSUER")
            .ok()
            .filter(|s| !s.trim().is_empty())?;
        let client_id = std::env::var("OIDC_CLIENT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())?;
        let redirect_uri = std::env::var("OIDC_REDIRECT_URI")
            .ok()
            .filter(|s| !s.trim().is_empty())?;
        let client_secret = std::env::var("OIDC_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let provider_name = std::env::var("OIDC_PROVIDER_NAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "rIDM SSO".to_string());

        Some(Self {
            issuer: issuer.trim_end_matches('/').to_string(),
            client_id: client_id.trim().to_string(),
            client_secret: client_secret.map(|s| s.trim().to_string()),
            redirect_uri: redirect_uri.trim().to_string(),
            provider_name,
        })
    }
}

/// Generates a 32-byte cryptographically random URL-safe token.
pub fn generate_random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Computes an RFC 7636 PKCE S256 code_challenge from a code_verifier.
pub fn pkce_s256_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Derives a persistent 64-byte AEAD master key for `PrivateCookieJar`.
/// Uses `AGGREGATOR_SESSION_SECRET` if configured, or domain-separated SHA-512-style
/// expansion of the primary API key / UI password so sessions survive container restarts.
pub fn derive_session_key(
    explicit_secret: Option<&str>,
    api_keys: &[String],
    ui_password: Option<&str>,
) -> Key {
    let seed = explicit_secret
        .filter(|s| !s.trim().is_empty())
        .or_else(|| ui_password.filter(|s| !s.trim().is_empty()))
        .or_else(|| api_keys.first().map(|s| s.as_str()));

    if let Some(material) = seed {
        let mut h1 = Sha256::new();
        h1.update(b"economy-aggregator:cookie-key:part1:");
        h1.update(material.as_bytes());
        let d1 = h1.finalize();

        let mut h2 = Sha256::new();
        h2.update(b"economy-aggregator:cookie-key:part2:");
        h2.update(material.as_bytes());
        let d2 = h2.finalize();

        let mut key_bytes = [0u8; 64];
        key_bytes[..32].copy_from_slice(&d1);
        key_bytes[32..].copy_from_slice(&d2);
        Key::from(&key_bytes)
    } else {
        Key::generate()
    }
}

pub fn load_session_secret_from_env() -> Option<String> {
    std::env::var("AGGREGATOR_SESSION_SECRET")
        .or_else(|_| std::env::var("SESSION_SECRET"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

pub fn load_ui_password_from_env() -> Option<String> {
    std::env::var("AGGREGATOR_UI_PASSWORD")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Detects TLS termination (direct HTTPS or reverse proxy `X-Forwarded-Proto: https`).
pub fn is_https_request(headers: &HeaderMap) -> bool {
    if let Some(proto) = headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()) {
        if proto.split(',').any(|p| p.trim().eq_ignore_ascii_case("https")) {
            return true;
        }
    }
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if origin.starts_with("https://") {
            return true;
        }
    }
    false
}

/// Builds a hardened `HttpOnly`, `SameSite=Lax` cookie for the encrypted session jar.
pub fn build_session_cookie(encoded: String, secure: bool, max_age_secs: i64) -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE_NAME, encoded);
    cookie.set_http_only(true);
    cookie.set_secure(secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(max_age_secs));
    cookie
}

pub fn build_clear_session_cookie(secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE_NAME, "");
    cookie.set_http_only(true);
    cookie.set_secure(secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}

/// Sliding-window per-client login failure rate limiter to prevent brute-force attacks.
pub struct LoginRateLimiter {
    attempts: RwLock<HashMap<String, Vec<Instant>>>,
    max_attempts: usize,
    window: Duration,
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self {
            attempts: RwLock::new(HashMap::new()),
            max_attempts: 10,
            window: Duration::from_secs(300),
        }
    }
}

impl LoginRateLimiter {
    pub fn check_allowed(&self, client_id: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut guard = self.attempts.write().unwrap_or_else(|e| e.into_inner());
        let entry = guard.entry(client_id.to_string()).or_default();
        entry.retain(|ts| now.duration_since(*ts) < self.window);
        if entry.len() >= self.max_attempts {
            let oldest = entry.first().copied().unwrap_or(now);
            let retry = self.window.saturating_sub(now.duration_since(oldest)).as_secs().max(1);
            return Err(retry);
        }
        Ok(())
    }

    pub fn record_failure(&self, client_id: &str) {
        let now = Instant::now();
        let mut guard = self.attempts.write().unwrap_or_else(|e| e.into_inner());
        let entry = guard.entry(client_id.to_string()).or_default();
        entry.retain(|ts| now.duration_since(*ts) < self.window);
        entry.push(now);
    }

    pub fn clear(&self, client_id: &str) {
        let mut guard = self.attempts.write().unwrap_or_else(|e| e.into_inner());
        guard.remove(client_id);
    }
}
