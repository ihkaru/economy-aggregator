# GEMINI.md - Architecture & Project Guidelines: Economy Analysis Platform

## 1. Project Overview & Vision
`economy-analysis` is a high-performance local financial and economic analysis platform modeled after TradingView.
Key characteristics:
- **Multi-layout workspace:** Synchronized multi-chart panels (1x1, 1x2, 2x2, 3x3) with independent or linked timeframes, indicators, and crosshairs.
- **Strict Sharp Visual Language:** 0 border-radius (`border-radius: 0px` strictly enforced). Sharp 90-degree corners, high-density terminal dark mode.
- **Data Engine:** Powered natively by **Polars** with a **Hybrid Columnar Lakehouse** (Hive-Partitioned Parquet cold store + low-latency micro-batch hot buffer) and optional DuckDB zero-copy interop.
- **Dual Ingestion:** Ingests external live streaming data (WebSocket) and historical batch data (REST APIs) into local storage for sub-10ms analytics.

---

## 2. Architecture: Vertical Slice Architecture (VSA) + EIP + SOLID

The codebase is organized by business feature capabilities (Vertical Slices), rather than horizontal technical tiers.

```
src/
├── core/                         # Shared foundational contracts, config, domain primitives
│   ├── config.py                 # Environment and engine configuration
│   └── types.py                  # Domain models (Candle, Tick, Indicator, TimeFrame)
├── features/
│   ├── ingestion/                # Ingestion Slice: Live WebSockets & Batch Fetchers
│   │   ├── command_fetch_batch.py
│   │   ├── stream_live_ticks.py
│   │   └── normalizer_pipeline.py# EIP: Pipes & Filters for schema validation
│   ├── storage/                  # Storage Slice: Hot WAL buffer & Parquet Lake
│   │   ├── parquet_lake_writer.py
│   │   ├── micro_batch_buffer.py # EIP: Idempotent receiver / aggregator
│   │   └── lake_compactor.py     # Background small-file compaction
│   ├── analytics/                # Analytics Slice: Polars-native computation engine
│   │   ├── series_scanner.py     # LazyFrame partition scanner & predicate pushdown
│   │   ├── technical_indicators.py # Vectorized EMA, RSI, MACD, Bollinger Bands
│   │   └── macro_aggregator.py   # Resampling & cross-sectional correlations
│   └── api/                      # Presentation Slice: REST & WebSocket API for UI
│       ├── router.py
│       └── live_stream_channel.py# EIP: Publish/Subscribe for connected UI clients
└── tests/
    ├── test_ingestion.py
    ├── test_storage.py
    └── test_analytics.py
```

### Architectural Invariants:
1. **Single Responsibility Principle (SRP):**
   - Maximum function length: 50 lines.
   - Maximum class/component length: 180 lines.
   - Maximum source file length: 400 lines.
2. **Vertical Slice Isolation:**
   - Slices must not tightly couple to internal implementations of sibling slices.
   - Cross-slice communication occurs via domain contracts or event routing.
3. **Zero JVM / Low RAM First:**
   - All components must run within standard workstation memory budgets.
   - Streaming iterators and Polars `LazyFrame` must be prioritized over buffering large DataFrames in memory.
4. **Strict Sharp UI Rules:**
   - No rounded corners on any UI elements (`border-radius: 0` globally).
   - High information density, monospaced numeric formatting for price/volume.

---

## 3. Verification Gauntlet & Tooling

Run the following commands to verify code quality, SRP adherence, and test coverage:

```bash
# 1. Deterministic SRP and length linting
python3 -m static_srp_analyzer src/ --strict

# 2. Parallel test execution
pytest -n auto tests/

# 3. Type checking
mypy src/
```

---

## 4. Licensing & Dependencies
- All third-party libraries must carry permissive enterprise licenses: **MIT, Apache-2.0, or BSD**.
- Copyleft licenses (GPL, AGPL) are strictly forbidden.
