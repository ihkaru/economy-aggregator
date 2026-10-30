# Economy Aggregator

A high-performance, self-contained financial and macroeconomic data aggregator engine built in 100% Rust.

## Key Capabilities
- **Zero-GC & Minimal RAM Footprint:** Operates under 50 MB idle and under 150 MB under heavy streaming load (50,000 ticks/sec).
- **Columnar Lakehouse Storage:** Direct partitioning to Apache Parquet with ZSTD compression (Hive layout: `asset_class / symbol / timeframe / year / month`).
- **Hybrid Ingestion & Gap Healing:** Seamlessly ingests real-time WebSocket feeds and REST batch historical data with automated deduplication and sequence gap healing.
- **Enterprise arc42 Architecture:** Fully documented under `docs/architecture/` (01 to 12) with automated freshness provenance verification.

## Architecture & Verification
Refer to [GEMINI.md](GEMINI.md) and [docs/architecture/](docs/architecture/).
