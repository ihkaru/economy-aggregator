# Research Report: Optimal Local Database & Storage Architecture for Polars

**Date:** October 2026  
**Scope:** High-performance local economic and financial time-series analytics (TradingView-style platform)  
**Target Capabilities:** Live streaming tick/candle ingestion, historical batch ingestion, zero-copy querying with Polars.

---

## 1. Executive Summary

For an economic and financial analysis platform operating locally on desktop/workstation environments, the optimal architecture paired with **Polars** is a **Hybrid Columnar Lakehouse Architecture**:
1. **Analytical Storage (Cold Layer):** Hive-Partitioned **Apache Parquet** compressed with `ZSTD` and dictionary encoding.
2. **Live Ingestion (Hot Buffer Layer):** In-memory ring buffer with timed micro-batching (5–10s flush) or **SQLite WAL** append-only journal.
3. **Query & Computational Engine:** **Polars LazyFrame** (`pl.scan_parquet`) as primary computational engine, interoperating with **DuckDB** via zero-copy **Apache Arrow** pointers for ad-hoc analytical SQL and ASOF joins.

This combination achieves:
- **Zero JVM / Zero Daemon Overhead:** Pure native binaries (Rust & C++).
- **Sub-10ms Query Latencies:** Utilizing column projection and partition pruning.
- **$O(1)$ Memory Streaming:** Polars streaming engine prevents out-of-memory errors on large multi-year datasets.
- **Permissive Enterprise Licensing:** MIT (Polars, DuckDB), Apache-2.0 (Arrow, Parquet).

---

## 2. Benchmark & Comparative Analysis

| Dimension | DuckDB + Parquet (Recommended) | LanceDB / Lance | QuestDB / TimescaleDB | SQLite Alone |
| :--- | :--- | :--- | :--- | :--- |
| **Polars Integration** | Native Zero-Copy (Arrow) | Native Zero-Copy (Arrow) | ADBC / ConnectorX | ConnectorX / Row scan |
| **Storage Format** | Open Parquet / `.duckdb` | Open Lance Columnar | Proprietary / PG Heap | B-Tree Row-based |
| **Execution Model** | Vectorized Columnar | Vectorized Columnar | Vectorized Columnar | Row-by-Row |
| **Live Ingestion** | Micro-batch (5-15s flush) | Native Append (MVCC) | High-throughput socket | Fast single-row insert |
| **Memory Footprint** | Extremely low (~20–50MB idle)| Low (~30–60MB idle) | Heavy (JVM/PG Daemon) | Minimal (~5MB idle) |
| **Licensing** | MIT / Apache-2.0 | Apache-2.0 | Apache-2.0 / BSL | Public Domain |
| **Ecosystem Portability**| Universal (Python, R, Go) | Growing | SQL Clients | Universal |

---

## 3. Storage Hierarchy Design

```
projects/economy-analysis/
├── data/
│   ├── hot_buffer/               <-- Temporary WAL / In-flight micro-batches
│   │   └── live_stream.wal
│   └── lake/                     <-- Partitioned Analytical Lake
│       ├── macro/                <-- FRED, World Bank, Central Bank rates
│       │   └── indicator=GDP/
│       │       └── freq=quarterly/
│       │           └── data.parquet
│       └── market/               <-- Equities, FX, Crypto OHLCV & Ticks
│           └── asset_class=equities/
│               └── symbol=AAPL/
│                   └── timeframe=1m/
│                       └── year=2026/
│                           └── month=10/
│                               └── chunk_20261030.parquet
```

### 3.1 Partitioning Strategy
- **Partition keys:** `asset_class / symbol / timeframe / year / month`
- **Why this works:** When the TradingView-style frontend requests candles for symbol `AAPL` on timeframe `1m` for the past 7 days, Polars scans *only* `market/asset_class=equities/symbol=AAPL/timeframe=1m/year=2026/month=10/*.parquet`, pruning away 99.9% of the dataset instantly.

### 3.2 Compression & Row Group Tuning
- **Compression:** `ZSTD` (Level 3 offers optimal compression ratio vs decompression speed).
- **Row Group Size:** `32,768` to `65,536` rows for intraday time-series (enables granular predicate pushdown and multi-threaded parallel scanning).

---

## 4. Live vs Batch Ingestion Architecture

### 4.1 Batch Ingestion Pipeline (Historical)
- Sources: Yahoo Finance, FRED, Stooq, SEC EDGAR, exchange archives.
- Process:
  1. Fetch historical chunks via asynchronous HTTP/REST.
  2. Normalize into standardized Polars DataFrame (`timestamp: Datetime[UTC]`, `open: Float64`, `high: Float64`, `low: Float64`, `close: Float64`, `volume: Float64`).
  3. Deduplicate and sort by `timestamp` ascending ($O(N \log N)$).
  4. Write atomic partitioned Parquet using `pl.DataFrame.write_parquet(..., compression="zstd")`.

### 4.2 Live Ingestion Pipeline (Streaming)
- Sources: WebSocket feeds (Binance, Alpaca, IEX, Finnhub, Polygon).
- Architecture (Pipes & Filters with Micro-Batching):
  1. WebSocket listener pushes raw tick/candle events into an in-memory ring buffer (thread-safe queue).
  2. Every $T$ seconds (e.g. 5 seconds) or when count $\ge M$ (e.g. 10,000 items), the buffer triggers an async flush.
  3. Flushed chunk is converted directly to an Apache Arrow Table / Polars DataFrame and written as a timestamped micro-parquet file into the day's partition.
  4. Periodic Compactor (Daily or Hourly): Merges small micro-parquet files into a single consolidated, sorted Parquet file to eliminate the "small files problem".

---

## 5. UI Architecture Vision: Sharp Pro Terminal (TradingView Style)

- **Layout Engine:** Multi-pane grid (Golden-Layout / Mosaic grid / custom CSS Grid splitters) supporting 1x1, 1x2, 2x2, 3x3 layouts with crosshair synchronization, timeframe sync, and independent chart panels.
- **Design Language:** Strictly sharp (`border-radius: 0px` on all elements, buttons, cards, dialogs, tabs). High-contrast dark palette (monokai / charcoal / terminal green & orange).
- **Chart Rendering:** WebGL / Canvas-based charting engine (such as Lightweight Charts or custom canvas renderer) capable of rendering 100,000+ data points at 60 FPS.
