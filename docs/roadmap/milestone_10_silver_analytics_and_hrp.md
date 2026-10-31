# Milestone 10: Native Silver Data Pipeline, Pure Rust Quantitative Allocation (HRP) & Lakehouse Catalog

**Phases:** 261 – 285  
**Domain:** Silver Panel Transformation, Pure Rust Machine Learning Allocation (Hierarchical Risk Parity), Threshold Rebalancing, Lakehouse Catalog, Convex Derivatives Modeling  
**Primary Goal:** Eliminate external Python runtime dependencies, resolve the small-files I/O traversal bottleneck, and provide a compile-time safe quantitative modeling engine in 100% native Rust.

---

### Phase Catalog

#### A. Native Silver Data Pipeline Engine (`economy-aggregator build-silver`) (Phases 261 – 266)
- [ ] **PHASE-261: Declarative Silver Dataset Specification & Target Schema**  
  Define strongly-typed configuration for consolidated Silver tables: spine calendar dates, asset universe symbols, currency normalization target, and Hampel cleaning window.
- [ ] **PHASE-262: Automated Multi-Exchange Calendar Harmonization Pipeline**  
  Integrate `TradingCalendarAligner` with forward-fill determinism to merge disparate exchange dates (NYSE, IDX, TSE, Crypto 24/7) into a continuous aligned panel.
- [ ] **PHASE-263: Real-Time Multi-Currency Normalization to Base Currency**  
  Automatically convert all foreign instruments (USD, EUR, SGD) into investor base currency (IDR) using daily FX time series (`FxNormalizer`).
- [ ] **PHASE-264: Consolidated Silver Parquet Table Generation**  
  Write unified multi-column columnar Parquet table (`data/lake/silver/panel_daily_<base_currency>.parquet`) with Snappy/ZSTD compression.
- [ ] **PHASE-265: Fast Point-in-Time Slice Reader from Silver Table**  
  Enable sub-5ms range queries across multi-decade panels without traversing thousands of raw Bronze partition folders.
- [ ] **PHASE-266: `economy-aggregator build-silver` CLI Command**  
  Provide agent-ready CLI command with `--base-currency <IDR|USD>`, `--start-year <YYYY>`, progress telemetry, and deterministic JSON envelope output.

---

#### B. Pure Rust Machine Learning & Portfolio Allocation Engine (Phases 267 – 275)
- [ ] **PHASE-267: Vectorized Pearson Correlation & Distance Matrix in Rust**  
  Compute correlation $\rho_{i,j}$ and correlation distance $d_{i,j} = \sqrt{0.5(1 - \rho_{i,j})}$ directly using vectorized SIMD-friendly Rust loops.
- [ ] **PHASE-268: Hierarchical Tree Clustering (Single Linkage Dendrogram) in Rust**  
  Implement agglomerative hierarchical clustering to build the hierarchical asset tree without external SciPy C-extensions.
- [ ] **PHASE-269: Quasi-Diagonalization of Covariance Matrix**  
  Reorder covariance rows and columns based on tree leaf clusters to place correlated assets adjacent to each other.
- [ ] **PHASE-270: Recursive Bisection Allocator (López de Prado HRP)**  
  Implement top-down inverse-variance recursive bisection, producing robust risk-parity weights resilient to singular/ill-conditioned covariance matrices.
- [ ] **PHASE-271: Pure Rust Threshold Rebalancing Engine**  
  Implement rebalancing simulation supporting tolerance bands ($\pm 5\%$, $\pm 10\%$, $\pm 25\%$) and periodic schedules (monthly, quarterly, annual).
- [ ] **PHASE-272: Native Backtest Command: `economy-aggregator analyze --strategy hrp`**  
  Integrate HRP allocator and rebalancer directly into the Rust CLI, outputting multi-decade equity curves and trade counts.
- [ ] **PHASE-273: Calibrated Dragon & Cockroach Tail-Risk Engine in Rust**  
  Implement institutional tail-risk model calibrated to CBOE Eurekahedge Long Volatility specs with theta decay and volatility crush.
- [ ] **PHASE-274: Quantitative Performance Metrics Suite**  
  Calculate CAGR, Annualized Volatility, Maximum Drawdown, Sharpe Ratio, Sortino Ratio, Calmar Ratio, and Portfolio Turnover in native Rust.
- [ ] **PHASE-275: Agent-Ready Portfolio Optimization & Rebalance Alerts**  
  Output actionable JSON recommendations (`{"rebalance_needed": true, "trades": [{"symbol": "BBCA", "action": "BUY", "delta_weight": 0.04}]}`).

---

#### C. Lakehouse Metadata Manifest & Autonomous Compaction Daemon (Phases 276 – 280)
- [ ] **PHASE-276: Symbology & Partition Manifest Catalog (`data/lake/meta/catalog.json`)**  
  Maintain a lightweight catalog tracking active symbols, date ranges, total row counts, and file lists to avoid recursive directory scans.
- [ ] **PHASE-277: Fast Incremental Catalog Updater on Micro-Batch Writes**  
  Update partition boundaries in the manifest atomically upon each micro-batch write.
- [ ] **PHASE-278: Autonomous Background Compactor Worker**  
  Trigger asynchronous compaction automatically when a partition directory accumulates more than 5 micro-chunks.
- [ ] **PHASE-279: Zero-Downtime Atomic Compaction Staging**  
  Stage compacted Parquet files as `.tmp` and commit via atomic `rename`, guaranteeing continuous read access for analytics.
- [ ] **PHASE-280: Lakehouse Health & Storage Diagnostics Command (`economy-aggregator status`)**  
  Provide partition fragmentation stats, average file sizes, and disk footprint metrics in the CLI status output.

---

#### D. Domain Type Abstraction for Derivatives & Non-Linear Payoffs (Phases 281 – 285)
- [ ] **PHASE-281: Typed Domain Enum for Delta vs Convex/Derivative Instruments**  
  Introduce `InstrumentType::Option`, `InstrumentType::TailRisk`, and `InstrumentConvexity` traits in `src/core/types.rs`.
- [ ] **PHASE-282: Option Greeks & Theta Burn Parameter Contracts**  
  Require daily theta decay rate, delta bounds, and implied volatility parameters for non-linear instruments.
- [ ] **PHASE-283: Bounded Delta Saturation & Volatility Crush Invariant Enforcement**  
  Enforce invariant preventing autonomous daily compounding on convex options without delta capping ($\le 1.0$) and post-rebound crush.
- [ ] **PHASE-284: Cash-Injection Tail Protection Model (Universa / Artemis Structure)**  
  Model tail risk as a fixed annual protection budget (1%–2%) that injects rebalancing liquidity during severe drawdowns.
- [ ] **PHASE-285: Integration Test Suite: Invariant Proof against Compounding Glitch**  
  Write automated hermetic tests proving that the engine rejects uncalibrated exponential compounding on option instruments.

---

#### E. Temporal Symbology & Corporate Actions Auto-Ingestion (Phases 286 – 290)
- [x] **PHASE-286: Permanent Identifiers (ISIN, OpenFIGI, PermID) & Temporal Aliases (`TickerAlias`)**  
  Extended `InstrumentMapping` in `symbology.rs` with persistent financial IDs and validity dates for historical rebrands.
- [x] **PHASE-287: Point-in-Time Symbology Resolver (`InstrumentRegistry::resolve_at`)**  
  Implemented temporal resolver mapping past ticker symbols (e.g. `FB` $\to$ `META`) based on query dates, preventing ticker drift.
- [x] **PHASE-288: Volume-Aware Stock Split Backward Adjustment (`adjust_ohlcv_backward`)**  
  Extended `CorporateActionAdjuster` to scale historical price ($P \times \text{factor}$) and volume ($V / \text{split\_factor}$), preserving turnover continuity.
- [x] **PHASE-289: Automated Yahoo Finance Corporate Action Events Extractor**  
  Implemented `YahooFinanceFetcher::fetch_daily_bars_with_events()` parsing historical splits and dividends directly from exchange feeds.
- [ ] **PHASE-290: Parquet Corporate Actions Metadata Table (`data/lake/meta/corporate_actions/`)**  
  Write persistent Parquet tables for declared corporate actions to feed the automated Silver pipeline.
