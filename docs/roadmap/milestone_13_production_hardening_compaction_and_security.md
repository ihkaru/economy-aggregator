# Milestone 13: Production Hardening, Auto-Compaction, Security Audit Remediation & Web Terminal Auth

**Phases:** 298 – 305  
**Domain:** Production Resilience, Lakehouse Auto-Compaction (`RISK-02`/`RISK-14`), Thread Separation (`spawn_blocking`), Security Hardening (`OWASP API Top 10` + `osv-scanner`), Built-In Web Terminal API Key Auth (`IF-08`).  
**Primary Goal:** Remediate empirical production findings from `https://market.dvlpid.my.id` and enforce 100% alignment with `arc42` constraints (`Section 2.1`, `8.1`, `8.6`, `ADR-02`, `ADR-16`).

---

## 1. Security Audit Findings (2026-11-27 Baseline)

| Finding ID | Severity | Area | Description | Remediation Phase |
| :--- | :--- | :--- | :--- | :--- |
| **SEC-01** | High | Dependencies (`Cargo.lock`) | `osv-scanner` detected 19 advisories across 11 crates (including `thrift` CVSS 7.5 via `parquet 52`, `url`, `zstd-safe`/`zstd-sys`). | Phase 298 (`cargo update` safe patches + track `parquet 53+` upgrade) |
| **SEC-02** | Medium | HTTP Layer (`rest.rs`) | Missing security response headers (`X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`, `Cache-Control: no-store` on `/v1/*`) and missing `DefaultBodyLimit(1MB)` DoS protection. | Phase 298 (`auth.rs` / `security_headers_middleware` + `DefaultBodyLimit`) |
| **SEC-03** | Medium | Input Validation (`rest.rs`, `subscription_routes.rs`) | `symbol`, `source`, `asset_class` inputs lacked explicit boundary rejection of path traversal sequences (`..`, `\`, `\0`) and excessive lengths (>128 chars). | Phase 298 (`validate_safe_identifier`) |
| **SEC-04** | Low | Availability / Panic Safety | `.read().unwrap()` / `.write().unwrap()` on `RwLock` in `orchestrator.rs` and `catalog_routes.rs` could panic on poisoned locks (violates `arc42` Section 8.1). | Phase 298 (`.unwrap_or_else(|e| e.into_inner())`) |
| **SEC-05** | Medium | Web UI Auth (`web/src/lib/api.ts`) | Built-in Svelte terminal did not attach `X-API-Key` header, causing `401 Unauthorized` when `AGGREGATOR_API_KEY` is enabled in production. | Phase 302 (`ApiKeyModal.svelte` + `api.ts` header injection) |

---

## 2. Granular Implementation Phases

### Phase 298: Security Hardening Middleware, Input Sanitization & Safe Lock Recovery
- **Files:** `src/features/serving/auth.rs`, `src/features/serving/rest.rs`, `src/features/serving/subscription_routes.rs`, `src/features/serving/catalog_routes.rs`, `src/features/ingestion/orchestrator.rs`
- **Deliverables:**
  - Add `security_headers_middleware` setting `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, `Permissions-Policy: geolocation=(), microphone=(), camera=()`, and `Cache-Control: no-store` on `/v1/*`.
  - Add `axum::extract::DefaultBodyLimit::max(1024 * 1024)` (1 MiB) to prevent memory exhaustion DoS.
  - Add `validate_safe_identifier()` rejecting `..`, `\`, `\0`, control chars, and length > 128 on subscription and candle/query boundaries.
  - Replace all `.read().unwrap()` / `.write().unwrap()` in serving & orchestrator with poison-safe `.unwrap_or_else(|e| e.into_inner())`.
  - Run `cargo update` to remediate fixable `osv-scanner` advisories in `Cargo.lock`.

### Phase 299: Autonomous Scheduled Compaction & Remote Compaction Endpoint
- **Files:** `src/features/ingestion/orchestrator.rs`, `src/features/serving/subscription_routes.rs`, `src/features/serving/rest.rs`, `src/core/cli_remote.rs`
- **Deliverables:**
  - Schedule `LakeCompactor::compact_all(&config.lake_dir, 2)` every 6 hours inside `IngestionOrchestrator` via `tokio::task::spawn_blocking`.
  - Expose `POST /v1/storage/compact` (supports optional `min_files` query/JSON param) with automatic serving cache invalidation.
  - Wire remote CLI `economy-aggregator compact [--min-files N]` to call `POST /v1/storage/compact`.

### Phase 300: ZSTD Compression Parity (`ADR-02`) for Prediction Markets
- **Files:** `src/features/ingestion/prediction_market/mod.rs`
- **Deliverables:**
  - Upgrade Parquet writer compression from `Compression::SNAPPY` to `Compression::ZSTD(ZstdLevel::try_new(3))` matching `ADR-02`.

### Phase 301: Tokio `spawn_blocking` Offloading (`arc42` Section 2.1 & 8.6)
- **Files:** `src/features/serving/rest.rs`, `src/features/serving/subscription_routes.rs`, `src/features/ingestion/orchestrator.rs`
- **Deliverables:**
  - Offload heavy synchronous Parquet read (`LakeReader::read_candles_with_limit`) and compaction (`LakeCompactor::compact_all`) to `tokio::task::spawn_blocking` so Tokio network worker threads are never blocked.

### Phase 302: Built-In Web Terminal UI API Key Support (`IF-08`)
- **Files:** `web/src/lib/api.ts`, `web/src/components/ApiKeyModal.svelte`, `web/src/App.svelte`
- **Deliverables:**
  - Inject `X-API-Key` header automatically from `localStorage` (`ea_api_key`) in `api.ts`.
  - Detect `401 Unauthorized` responses and prompt operator with an obsidian, `0px` border-radius, zero-emoji `ApiKeyModal.svelte` to enter/update the API key.

### Phase 303: Security & Compaction Integration Test Suite
- **Files:** `tests/test_auth_and_remote_cli.rs`
- **Deliverables:**
  - Verify security headers on `/v1/health` and `/v1/market/catalog`.
  - Verify path traversal rejection (`400 Bad Request`) on `POST /v1/subscriptions` and `GET /v1/market/candles`.
  - Verify `POST /v1/storage/compact` and remote CLI `compact` end-to-end against live authenticated Axum server.

### Phase 304: Arc42 ADR-17 & Provenance Synchronization
- **Files:** `docs/architecture/09_architecture_decisions.md`, `docs/architecture/01..12_*.md`, `docs/roadmap/ROADMAP.md`
- **Deliverables:**
  - Record **ADR-17** (Security Hardening, Auto-Compaction & Thread Separation).
  - Verify `arc42-lint` (12/12 FRESH) and `vsa-boundary-checker` (ALL SLICES COMPLIANT).

### Phase 305: Production Deployment & Live Compaction Verification
- **Deliverables:**
  - Push to `ihkaru/economy-aggregator` `main`, deploy to Coolify ThinkStation (`https://market.dvlpid.my.id`), verify security headers live, and run `economy-aggregator compact` on prod.
