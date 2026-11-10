# Milestone 11: Autonomous Ingestion, Discovery & Gap Catch-Up Engine

**Scope:** Production-Ready Autonomous Data Ingestion, Discovery & Gap Catch-Up Engine for Containerized Environments (Coolify, Docker).  
**Runtime:** 100% Native Rust (Async Tokio, Axum, Rayon, Parquet ZSTD).  
**Status:** In Progress.

---

## 1. Executive Summary

Milestone 11 provides an autonomous, self-healing ingestion pipeline that:
1. Enables multi-source discovery (Search APIs) across global equities (Yahoo Finance), crypto (Binance), macroeconomic indicators (Biquote/SDMX), and prediction markets (Polymarket & Kalshi).
2. Manages subscriptions via an Agent-First CLI (`economy-aggregator search`, `subscribe`, `unsubscribe`, `subscriptions list`).
3. Automatically scans the Lakehouse high-watermark on container startup to detect missing historical ranges when the container was offline for days or months.
4. Executes deterministic historical catch-up via REST batch paginators, writes deduplicated Parquet chunks, and seamlessly transitions into continuous live WebSocket streaming and scheduled polling.
5. Fully containerized with DNS resilience (Cloudflare/Google resolvers) for deployment on Coolify and bare-metal nodes.

---

## 2. Granular Implementation Phases

### Phase 286: Multi-Source Search & Symbol Discovery Engine
- **Files:** `src/features/ingestion/search.rs`, `src/core/cli_search.rs`
- **Capabilities:**
  - `YahooSearchFetcher`: Queries `https://query1.finance.yahoo.com/v1/finance/search?q={query}`.
  - `BinanceSearchFetcher`: Queries Binance exchange symbols & active trading pairs.
  - `PolymarketSearchFetcher`: Queries `https://gamma-api.polymarket.com/public-search?q={query}`.
  - `KalshiSearchFetcher`: Queries Kalshi series and active market titles.
  - Unified search aggregator mapping external results to standardized `SearchResultItem` records.
  - CLI subcommand: `economy-aggregator search --query <query> [--source <source>] [--limit N] [--json]`.

### Phase 287: Persistent Subscription Configuration & CLI Manager
- **Files:** `src/core/subscription.rs`, `src/core/cli_subscription.rs`, `src/features/serving/subscription_routes.rs`
- **Capabilities:**
  - Schema `SubscriptionConfig` and `SubscriptionItem` serialized to `data/config/subscriptions.json`.
  - Atomic file write with backup and read/write lock for concurrent safety.
  - CLI subcommands:
    - `economy-aggregator subscribe --source <src> --symbol <raw> --canonical <canon> --asset <class> [--tf <tf>]`
    - `economy-aggregator unsubscribe --canonical <canon>`
    - `economy-aggregator subscriptions list [--json]`
  - REST endpoints: `GET /v1/subscriptions`, `POST /v1/subscriptions`, `DELETE /v1/subscriptions/:canonical`.

### Phase 288: High-Watermark Lakehouse Scanner & Gap Detection
- **Files:** `src/features/ingestion/watermark.rs`
- **Capabilities:**
  - Scans `data/lake/` partition hierarchies to determine the latest recorded timestamp (`high_watermark`) per instrument and timeframe.
  - Calculates missing intervals: `gap_duration = now - high_watermark`.
  - Identifies whether catch-up is required (e.g. missing > 1 daily bar or missing > 5 hourly bars).

### Phase 289: Historical Batch Catch-Up Engine
- **Files:** `src/features/ingestion/catch_up.rs`, `src/core/cli_catch_up.rs`
- **Capabilities:**
  - Historical range pagination for Yahoo Finance (`period1`, `period2`) and Binance (`startTime`, `endTime`).
  - Batching and writing into Parquet lakehouse via `LakeWriter` with natural-key deduplication.
  - CLI subcommand: `economy-aggregator catch-up [--canonical <sym>] [--dry-run] [--json]`.

### Phase 290: Container Autonomous Ingestion Orchestrator & Live Loop
- **Files:** `src/features/ingestion/orchestrator.rs`, `src/features/serving/ingestion_routes.rs`, `Dockerfile`, `docker-compose.coolify.yml`
- **Capabilities:**
  - Background orchestrator spawned on `economy-aggregator serve`.
  - Asynchronous startup sequence:
    1. Loads subscriptions.
    2. Runs high-watermark catch-up in background without blocking REST API startup.
    3. Launches WebSocket workers for crypto streams.
    4. Launches scheduled Tokio intervals for equities, macro, and prediction markets.
  - REST status endpoint: `GET /v1/ingestion/status`.
  - Coolify Docker setup with persistent volume mounting and Cloudflare DNS resolvers (`1.1.1.1`, `8.8.8.8`).

### Phase 291: Verification Gauntlet, Unit & Integration Tests
- **Files:** `tests/test_search_and_discovery.rs`, `tests/test_subscriptions.rs`, `tests/test_watermark_and_catch_up.rs`
- **Capabilities:**
  - End-to-end tests for discovery search, subscription lifecycle, gap detection, and batch catch-up.
  - Static code analysis (deterministic line limit check $\le 400$ lines).
