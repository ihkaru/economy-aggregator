# Economy Analysis Platform

A high-performance, local financial and macroeconomic analysis workstation inspired by TradingView.

## Key Capabilities
- **Multi-Layout Workspace:** Multi-chart views (1x1, 1x2, 2x2, 3x3) with crosshair and timeframe synchronization.
- **Strict Sharp Visual Language:** Zero rounded corners (`border-radius: 0px` strictly enforced) for high-density, professional terminal aesthetics.
- **High-Performance Data Lakehouse:**
  - Cold Layer: Hive-partitioned Apache Parquet datasets.
  - Hot Buffer: Micro-batched live streaming ticks with in-memory buffer / SQLite WAL.
  - Compute Engine: Polars LazyFrame with zero-copy Apache Arrow integration.
- **Dual Ingestion:** Ingests live streaming WebSocket market feeds and batch historical macroeconomic & financial records.

## Project Structure (Vertical Slice Architecture)
- `src/core/`: Common domain models, types, and configuration.
- `src/features/ingestion/`: Live streaming and batch fetching pipelines (EIP: Pipes & Filters).
- `src/features/storage/`: Parquet partition writer, micro-batch buffer, and lake compactor.
- `src/features/analytics/`: Vectorized technical indicators (EMA, RSI, MACD, Bollinger Bands) and macro series aggregations via Polars.
- `src/features/api/`: Local REST and WebSocket server for UI interaction.

## Research
Comprehensive database benchmarking and architecture rationale are documented in:
[docs/research/database_implementation_research.md](docs/research/database_implementation_research.md)
