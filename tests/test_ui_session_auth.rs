//! Integration tests for production Web UI authentication:
//! AEAD-encrypted HttpOnly cookie session (`axum-extra` PrivateCookieJar),
//! CSRF synchronizer token enforcement, brute-force rate limiting, and logout.

use std::sync::Arc;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::subscription::SubscriptionManager;
use economy_aggregator::features::ingestion::orchestrator::IngestionOrchestrator;
use economy_aggregator::features::serving::session::UiAdminAccount;
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{InstrumentRegistry, PartitionResolver};

fn build_auth_app(tmp: &std::path::Path) -> axum::Router {
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::Argon2;
    use rand::rngs::OsRng;

    let mut config = LakehouseConfig::default();
    config.lake_dir = tmp.join("lake");
    config.hot_buffer_dir = tmp.join("hot_buffer");
    config.base_dir = tmp.to_path_buf();
    config.ensure_directories().unwrap();

    let salt = SaltString::generate(&mut OsRng);
    let argon_hash = Argon2::default()
        .hash_password(b"operator-passphrase-2026", &salt)
        .unwrap()
        .to_string();

    let resolver = PartitionResolver::new(config.lake_dir.clone());
    let sub_manager = Arc::new(SubscriptionManager::new(
        config.base_dir.join("config").join("subscriptions.json"),
    ));
    let orchestrator = Arc::new(IngestionOrchestrator::new(
        config.clone(),
        sub_manager.clone(),
    ));
    let state = Arc::new(
        AppState::new(
            config,
            resolver,
            InstrumentRegistry::new(),
            sub_manager,
            orchestrator,
        )
        .with_api_keys(vec!["ea_prod_master_key_2026".to_string()])
        .with_ui_auth(
            Some("persistent_session_master_secret_64_bytes_minimum_entropy".to_string()),
            Some(UiAdminAccount {
                username: "admin".to_string(),
                password_hash: Some(argon_hash),
                plain_password: None,
            }),
            None,
        ),
    );
    create_router(state)
}

#[tokio::test]
async fn test_ui_httponly_cookie_session_csrf_and_logout_lifecycle() {
    let tmp = tempdir().unwrap();
    let app = build_auth_app(tmp.path());

    // 1. Unauthenticated /v1/auth/session reports auth_required=true, authenticated=false
    let sess_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/auth/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sess_res.status(), StatusCode::OK);
    let sess_json: Value =
        serde_json::from_slice(&sess_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(sess_json["auth_required"], true);
    assert_eq!(sess_json["authenticated"], false);
    assert!(sess_json["csrf_token"].is_null());

    // 1b. Machine API key CANNOT be used as UI password when standalone UiAdminAccount is configured
    let api_key_reject = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"ea_prod_master_key_2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(api_key_reject.status(), StatusCode::UNAUTHORIZED);

    // 2. Login via POST /v1/auth/login with valid Argon2id admin account issues HttpOnly cookie
    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"operator-passphrase-2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);
    let set_cookie = login_res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.contains("ea_session="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));
    let cookie_pair = set_cookie.split(';').next().unwrap().to_string();

    let login_json: Value =
        serde_json::from_slice(&login_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let csrf_token = login_json["csrf_token"].as_str().unwrap().to_string();
    assert_eq!(login_json["authenticated"], true);

    // 3. GET /v1/market/catalog succeeds using ONLY HttpOnly cookie (no X-API-Key header)
    let cat_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/market/catalog")
                .header(header::COOKIE, &cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cat_res.status(), StatusCode::OK);

    // 4. State-changing POST without X-CSRF-Token is blocked with 403 Forbidden
    let csrf_fail = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/storage/compact")
                .header(header::COOKIE, &cookie_pair)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"min_files":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(csrf_fail.status(), StatusCode::FORBIDDEN);

    // 5. State-changing POST WITH matching X-CSRF-Token succeeds 200 OK
    let csrf_ok = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/storage/compact")
                .header(header::COOKIE, &cookie_pair)
                .header("x-csrf-token", &csrf_token)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"min_files":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(csrf_ok.status(), StatusCode::OK);

    // 6. Logout clears session cookie
    let logout_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/logout")
                .header(header::COOKIE, &cookie_pair)
                .header("x-csrf-token", &csrf_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout_res.status(), StatusCode::OK);
    let clear_cookie = logout_res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(clear_cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn test_login_rate_limiter_blocks_bruteforce() {
    let tmp = tempdir().unwrap();
    let app = build_auth_app(tmp.path());

    for _ in 0..10 {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("x-forwarded-for", "203.0.113.42")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"username":"admin","password":"wrong-password"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("x-forwarded-for", "203.0.113.42")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"operator-passphrase-2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.headers().contains_key(header::RETRY_AFTER));
}

#[test]
fn test_decode_base64_argon2id_hash_for_coolify_dotenv_safety() {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use economy_aggregator::features::serving::session::{decode_password_hash_value, hash_password_phc};

    let raw = hash_password_phc("TestPass-2026!").unwrap();
    assert_eq!(decode_password_hash_value(&raw).as_deref(), Some(raw.as_str()));

    let b64 = STANDARD.encode(raw.as_bytes());
    assert_eq!(decode_password_hash_value(&b64).as_deref(), Some(raw.as_str()));
    assert_eq!(
        decode_password_hash_value(&format!("b64:{}", b64)).as_deref(),
        Some(raw.as_str())
    );
}
