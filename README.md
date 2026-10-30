# Economy Aggregator

A high-performance, self-contained financial and macroeconomic data aggregator engine built in 100% Rust.

## Key Capabilities
- **Zero-GC & Minimal RAM Footprint:** Operates under 50 MB idle and under 150 MB under heavy streaming load (50,000 ticks/sec).
- **Columnar Lakehouse Storage:** Direct partitioning to Apache Parquet with ZSTD compression (Hive layout: `asset_class / symbol / timeframe / year / month`).
- **Hybrid Ingestion & Gap Healing:** Seamlessly ingests real-time WebSocket feeds and REST batch historical data with automated deduplication and sequence gap healing.
- **Enterprise arc42 Architecture:** Fully documented under `docs/architecture/` (01 to 12) with automated freshness provenance verification.

## Implementation Roadmap (225 Granular Phases)
The long-term engineering execution is organized into 8 milestones spanning 225 atomic phases:
- 🗺️ **[Master Implementation Roadmap](docs/roadmap/ROADMAP.md)**
  - 📦 **[M1: Core Engine & Storage](docs/roadmap/milestone_1_core_storage.md)** (Phases 001 - 025)
  - 🪙 **[M2: Crypto High-Frequency Engine](docs/roadmap/milestone_2_crypto.md)** (Phases 026 - 055)
  - 💱 **[M3: Forex & Commodities Tick Lake](docs/roadmap/milestone_3_forex_commodities.md)** (Phases 056 - 085)
  - 📈 **[M4: Global Equities & Corporate Actions](docs/roadmap/milestone_4_equities.md)** (Phases 086 - 115)
  - 🏛️ **[M5: Macroeconomic Calendar & Vintages](docs/roadmap/milestone_5_macro_calendar.md)** (Phases 116 - 145)
  - 🛡️ **[M6: Gap Healing & Lake Compaction](docs/roadmap/milestone_6_resilience_compactor.md)** (Phases 146 - 175)
  - ⚡ **[M7: Serving Layer & High-Speed Streaming](docs/roadmap/milestone_7_serving_streaming.md)** (Phases 176 - 200)
  - 📊 **[M8: Observability, Replay & Production Ops](docs/roadmap/milestone_8_operations_replay.md)** (Phases 201 - 225+)

## Architecture & Verification
- 🏛️ **[arc42 Architecture Documentation](docs/architecture/)**
- 📋 **[GEMINI.md Guidelines](GEMINI.md)**
