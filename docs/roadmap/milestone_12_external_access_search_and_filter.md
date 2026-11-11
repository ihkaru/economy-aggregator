# Milestone 12: External System Access, Search & High-Precision Lakehouse Filter Parity

**Phases:** 292 – 297  
**Domain:** External Serving Layer (REST API), High-Performance In-Memory Search, Historical Lakehouse Parquet Readers (Macro & Prediction Markets), Bitemporal Range Queries, Storage Partition Explorer.  
**Primary Goal:** Transform `economy-aggregator`'s Serving & Query Layer to be 100% proper, robust, and accessible for external systems (*downstream analytics, automated trading agents, BI dashboards, and external microservices*) across ALL persisted data domains.

---

## 1. Executive Summary

While `economy-aggregator` possesses an enterprise-grade Apache Parquet Lakehouse engine with Hive Partitioning, external REST API access previously had gaps in historical retrieval, catalog search, and fine-grained range filtering:
1. **Catalog Search & Filtering:** `GET /v1/market/catalog` returned the entire instrument array without server-side filtering (`q` query string or `asset_class`), forcing consumers to download and filter client-side.
2. **Candles Range Filtering:** `GET /v1/market/candles` supported `to`, `year`, and `month`, but lacked an explicit `from` timestamp parameter for arbitrary window slicing.
3. **Macroeconomic Lakehouse Archival Reader:** `GET /v1/macro/calendar` was primarily a live proxy with fallback to a single hardcoded partition (`2026/11`), preventing external systems from querying historical economic releases (2024–2025) or filtering by specific indicator (`CPI`, `GDP`).
4. **Prediction Markets Historical Time-Series:** `GET /v1/prediction-markets/markets/:id/candles` generated a single instant candle from live orderbooks and did not read historical candle time-series stored in the Parquet lakehouse.
5. **Storage Partition Discovery:** `GET /v1/storage/partitions` lacked substring keyword search on partition identifiers.

Milestone 12 addresses all of these gaps, achieving 100% read, search, and filter parity across all persistence tiers.

---

## 2. Granular Implementation Phases

### Phase 292: Market Catalog Local Search & Asset Class Filtering
- **Files:** `src/features/serving/catalog_routes.rs`, `src/features/serving/types.rs`
- **Capabilities:**
  - Introduce `CatalogQuery` DTO:
    ```rust
    pub struct CatalogQuery {
        pub q: Option<String>,
        pub asset_class: Option<String>,
        pub source: Option<String>,
        pub limit: Option<usize>,
        pub offset: Option<usize>,
    }
    ```
  - Implement fast in-memory case-insensitive substring and prefix matching against `symbol`, `canonical_symbol`, and friendly `name`.
  - Filter by asset class (`equities`, `crypto`, `forex`, `commodities`, `macro`).
  - Add pagination support (`limit`, `offset`) with deterministic count headers.

### Phase 293: Flexible Temporal Range Filtering on Market Candles
- **Files:** `src/features/serving/rest.rs`, `src/features/storage/reader.rs`, `src/features/serving/types.rs`
- **Capabilities:**
  - Add `from: Option<i64>` (Unix timestamp) and `start_date: Option<String>`, `end_date: Option<String>` (ISO-8601) to `CandleQuery`.
  - Update `LakeReader::read_candles_with_limit` to accept both start and end timestamp cutoffs (`from_timestamp..=to_timestamp`).
  - Normalize multi-venue symbols in the cache key to replace both `:` and `/` with `_` (e.g. `COINBASE:BTCUSD` -> `COINBASE_BTCUSD`).
  - Partition pruning optimization: if `from` and `to` are within a narrow range, skip older and newer partition directories entirely.

### Phase 294: Macroeconomic Lakehouse Archival Reader (`LakeMacroReader`) & REST Parity
- **Files:** `src/features/storage/macro_reader.rs`, `src/features/serving/macro_routes.rs`
- **Capabilities:**
  - Implement `LakeMacroReader` to scan and deserialize historical Parquet batches from `data/lake/macro/calendar/country={country}/year={year}/month={month}/*.parquet`.
  - Support partition pruning by `country`, `year`, and `month`.
  - Filter events by `indicator_code` (e.g. `cpi_m_m`, `gdp_q_q`, `unemployment_rate`), `importance` (`high`, `medium`, `low`), and date range (`from_date` to `to_date`).
  - Enhance `GET /v1/macro/calendar`:
    - `source=lake|live|hybrid` (default: `"lake"` for sub-5ms offline-first performance).
    - Expose multi-year historical macro archives directly to external systems.

### Phase 295: Prediction Market Historical Candle Reader (`LakePredictionReader`) & Contracts Search
- **Files:** `src/features/storage/prediction_reader.rs`, `src/features/serving/prediction_routes.rs`
- **Capabilities:**
  - Implement `LakePredictionReader` to read historical probability candles from `data/lake/prediction_markets/candles/venue={venue}/market={market_id}/timeframe={tf}/year={year}/month={month}/*.parquet`.
  - Enhance `GET /v1/prediction-markets/markets/:id/candles` to serve complete historical time-series curves from disk Parquet files, optionally stitched with current live orderbook snapshots.
  - Enhance `GET /v1/prediction-markets/events`:
    - Add `q: Option<String>` for substring search on contract titles/questions.
    - Add `status: Option<String>` (`open`, `closed`, `all`).
    - Fall back to local persisted contract parquet files when upstream APIs are offline.

### Phase 296: Storage Partition Explorer Keyword Filtering & Metric Telemetry
- **Files:** `src/features/serving/analytics_routes.rs`
- **Capabilities:**
  - Enhance `GET /v1/storage/partitions`:
    - `domain: Option<String>` (`market`, `macro`, `prediction_markets`, `ALL`).
    - `q: Option<String>` (case-insensitive substring filter on identifier/symbol, e.g. `BTC`, `BBCA`, `cpi`).
    - `timeframe: Option<String>`.
    - `year: Option<i32>`, `month: Option<u32>`.
    - `limit: Option<usize>`.
  - Enable external monitoring systems to audit the existence and size of specific partitions instantaneously.

### Phase 297: Comprehensive Verification Gauntlet & Zero-Mock Integration Tests
- **Files:** `tests/test_external_access_and_filter.rs`, `tests/test_sanity.rs`
- **Capabilities:**
  - Automated integration tests verifying:
    1. Filtered catalog search (`/v1/market/catalog?q=BTC&asset_class=crypto`).
    2. Date-range candle query (`/v1/market/candles?from=...&to=...`).
    3. Historical macro lake query (`/v1/macro/calendar?source=lake&indicator=cpi&year=2026`).
    4. Prediction candle time-series query (`/v1/prediction-markets/markets/:id/candles`).
    5. Filtered partition query (`/v1/storage/partitions?q=COINBASE`).
  - Governance compliance: enforce strict $\le 350$ lines for serving/features, $\le 400$ lines overall per file.
  - Zero-mock validation: 100% real Parquet files and valid Arrow schemas.
