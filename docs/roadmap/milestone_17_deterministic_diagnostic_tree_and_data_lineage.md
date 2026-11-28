# Milestone 17: Deterministic Liquidity Diagnostic Tree, Persistent Backtrack Ledger & As-Is/Adjusted Data Lineage

> **Status:** Implemented & Verified (Phases 345–350)  
> **Scope:** Enforce explicit separation between raw *as-is* upstream data and *adjusted/derived* outputs across the entire Lakehouse, implement a deterministic epistemic diagnostic decision tree (`GET /v1/analytics/liquidity-tree`) for answering *"where is global capital moving or parking right now?"*, and persist immutable Arrow/Parquet diagnostic snapshots with full source-to-leaf backtrack lineage.

---

## 1. Empirical Audit: Raw As-Is vs Adjusted Data Across All Connectors

An exhaustive code-level audit of `src/features/ingestion/` and `src/features/storage/` established the exact processing status of every ingested series:

| Processing Layer (`DataProcessingLayer`) | Description | Connectors & Series | Raw Preservation Status |
| :--- | :--- | :--- | :--- |
| **`raw_as_is`** | Stored verbatim as published upstream (`value == raw_value`); zero scaling, zero seasonal adjustment, zero split modification. | `ssga_xlsx` (`SPY/GLD/BIL/JNK/SPTL` raw AUM & unadjusted shares), `defillama`, `coinmetrics_community`, `bgeometrics`, `github_ercindedeoglu`, `ofr_stfm`, `nyfed_api`, `treasury_tbill_csv`, `cftc_cot`, `occ_csv`, `stockanalysis`, `bybit_v5`, and dimensionless FRED rates (`DTB3`, `T10Y2Y`, `BAMLH0A0HYM2`, `SOFR`, `STLFSI4`). | **100% Preserved** (`value == raw_value`, plus SHA-256 `source_checksum`). |
| **`raw_unit_scaled`** | `raw_value` stores the untouched upstream figure; `value` applies only a constant SI unit multiplier (e.g. `$ Millions -> USD`). Zero statistical, seasonal, or split transformation. | `fred_api` NSA stocks (`WALCL`, `WTREGEN`, `RRPONTSYD`, `WRMFNS`, `WM2NS`, `ECBASSETSW`, `JPNASSETS`), `finra_xlsx` (`debit_millions * 1e6`), `treasury_fiscaldata` (`TGA * 1e6`), `dbnomics_fed`, `fed_h41_html`. | **100% Preserved** via `raw_value` column in Parquet. |
| **`upstream_seasonally_adjusted`** | Upstream publisher (Federal Reserve H.8) applies seasonal adjustment prior to publication (`raw_value` preserved untouched; `value` scaled to USD). | `fred_api` (`TOTBKCR`, `DPSACBW027SBOG`), `dbnomics_fed` (`BANK_CREDIT_USD`). | **Upstream-SA Preserved**; explicitly tagged so analysts never confuse SA vs NSA (`WRMFNS`/`WM2NS`). |
| **`upstream_split_adjusted`** | Upstream market candle feed retroactively adjusts historical `close` for stock splits (not dividends; dividends stay in Yahoo `adjclose`). | `yahoo_finance` (`1d` equity/ETF candles `SPY`, `GLD`, `BIL`, `JNK`, `SPTL`, `^MOVE`, `^GSPC`). | **Explicitly flagged** to prevent mixing split-adjusted Yahoo prices with unadjusted SSGA raw shares. |
| **`derived_adjusted`** | Computed on-demand or stored under isolated `domain="derived"` / `lake/diagnostics/`; **never mutates raw `lake/observations/`**. | `compute_exact_share_flows` (split & 1-day glitch guard flagged per row via `split_or_glitch_adjusted`), `compute_btc_etf_usd_flows`, `compute_fed_net_liquidity` (`ex_rrp` & `net_liquidity`), `WeeklyFlowPanel`, `SpilloverReport`, and `LiquidityDiagnosticTree`. | **Strictly isolated** from raw observation partitions. |

---

## 2. Anti-Fishing & Epistemic Governance Rules

To prevent recursive automated questioning from degenerating into *Garden of Forking Paths / Data Dredging*:
1. **Fixed Deterministic Protocol at Runtime:** Tree predicates, thresholds, and epistemic tiers (`Tier1_Accounting`, `Tier2_Replicated`, `Tier3_RegimeDescriptive`, `Tier4_HypothesisGauge`, `BlindSpot`) are frozen in versioned code (`protocol_version = "1.0.0"`). For identical `(as_of_date, window_weeks, lake_snapshot)`, traversal is 100% deterministic with verifiable SHA-256 `tree_checksum_sha256`.
2. **Staleness Awareness (`STALE_LAG`):** Monthly/lagged series (`FINRA Margin Debt`, `OFR MMF`, `BoJ`) automatically report `stale_lag_days` and emit `STALE_LAG` rather than silently pretending old observations describe today.
3. **Blind-Spot Escalation (Disciplined Growth):** Whenever observed tanks cannot explain a move, the tree terminates transparently at `BLIND_SPOT` and emits explicit `unanswered_questions` + missing-source recommendations — never fabricating post-hoc rationalizations.

---

## 3. Implementation Phases (Phases 345–350)

| Phase | Deliverable | Target Modules | Verification Gate |
| :--- | :--- | :--- | :--- |
| **345** | Roadmap & As-Is vs Adjusted Data Audit Spec | `docs/roadmap/milestone_17_*.md` | Committed & cross-referenced in `ADR-22` |
| **346** | `DataProcessingLayer` Lineage Metadata & `split_or_glitch_adjusted` Flag | `src/core/observation_types.rs`, `src/core/series_catalog.rs`, `src/features/analytics/liquidity_flows.rs`, `src/features/serving/observation_routes.rs` | Every catalog entry exposes `data_layer`, `raw_value_preserved`, `adjustment_note`; split flag verified in unit tests |
| **347** | Deterministic Liquidity Diagnostic Tree Engine (`v1.0.0`) | `src/core/diagnostic_types.rs`, `src/features/analytics/liquidity_tree.rs`, `src/features/analytics/liquidity_tree_branches.rs` | Deterministic leaf routing across Risk-Off/Risk-On/Blind-Spot/Stale-Lag + full source backtrack lineage + `tree_checksum_sha256` |
| **348** | Persistent Append-Only Parquet Diagnostic Ledger | `src/features/storage/diagnostic_ledger.rs`, `src/features/storage/mod.rs` | ZSTD(3) Parquet persistence under `lake/diagnostics/tree_runs/year=YYYY/`, list history, and exact `run_id` retrieval |
| **349** | REST API Endpoints & Remote CLI Commands | `src/features/serving/diagnostic_tree_routes.rs`, `rest.rs`, `src/core/cli_remote.rs`, `src/core/cli.rs` | `GET /v1/analytics/liquidity-tree`, `/history`, `/runs/:run_id` + CLI `diagnose`, `diagnose-history`, `diagnose-run` |
| **350** | Integration Tests, `ADR-22`, `arc42-lint`, Coolify Deploy & Live Diagnosis | `tests/test_m17_diagnostic_tree_and_lineage.rs`, `docs/architecture/*.md` | Full test suite + VSA + `<400L` + `arc42-lint --strict` + live production verification |
