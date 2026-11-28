# Milestone 18: Best-Practice Continuous Analytics Engine, Event-Sourced Parquet Persistence & Prospective OOS Scorecard

> **Status:** Implemented & Verified (Phases 351–357)  
> **Scope:** Upgrade the Lakehouse from on-demand analytical queries to a **100% autonomous, event-sourced, idempotent Continuous Analytics Engine** with checksum-deduplicated Parquet persistence, non-destructive bin-packing compaction, atomic consumer checkpoints, automated regime-transition & blind-spot diffing, and a frozen-protocol Prospective OOS Scorecard (`v1.1.0`, anchored `2026-11-28`).

---

## 1. Best-Practice 4-Layer Persistence Architecture (`lake/diagnostics/`)

All analytical outputs remain **strictly isolated** from raw `lake/observations/` and `lake/candles/` (zero mutation of raw upstream observations):

```text
lake/diagnostics/
├── tree_runs/year=YYYY/                 # Layer 1: Immutable Diagnostic Tree Snapshots (Parquet ZSTD(3))
│   ├── tree_<YYYYMMDD>_<uuid8>.parquet  #   Atomic .tmp -> rename; content-addressed via tree_checksum_sha256
│   └── compacted_<YYYY>_<ts>.parquet    #   Non-destructive bin-packed consolidation (dedup by run_id, zero data loss)
├── transitions/year=YYYY/               # Layer 2: Event-Sourced Regime Shift & Blind-Spot Resolution Log (Parquet ZSTD(3))
│   └── tr_<YYYYMMDD>_<uuid8>.parquet    #   Promoted typed columns (window_weeks, transition_kind, from_leaf, to_leaf) + JSON diff
├── prospective_oos/year=YYYY/           # Layer 3: Weekly Prospective OOS Scorecard Snapshots (Parquet ZSTD(3))
│   └── oos_<week_friday>_<uuid8>.parquet#   Frozen protocol_v1.1.0 (anchor SHA-256 07949ada...), anti-peeking guard
└── checkpoints/
    └── continuous_analytics_state.json  # Layer 4: Atomic CQRS Consumer Checkpoint (tmp -> sync_all -> rename)
                                         #   Tracks per-window last_run_id, last_checksum, last_leaf, data_watermark_hash,
                                         #   last_oos_week_scored, and last_compaction_utc across container restarts.
```

### Key Architectural Invariants
1. **Schema Flexibility + Columnar Speed:** Frequently filtered dimensions (`schema_version`, `window_weeks`, `protocol_version`, `as_of_date`, `from_leaf_id`, `to_leaf_id`, `transition_kind`, `tree_checksum_sha256`) live as **top-level typed Arrow columns**, while deep evolving payloads (`nodes`, `source_lineage`, `metric_diffs`, `rule_rows`) live in versioned JSON columns — allowing protocol upgrades (`v1.0.0 -> v1.1.0+`) without breaking backward reads.
2. **Idempotent Checksum Deduplication (Zero File Sprawl):** The background supervisor persists a new tree snapshot **only when** `as_of_date` rolls over OR `tree_checksum_sha256` changes due to fresh upstream observations.
3. **Non-Destructive Bin-Packing Compaction:** Consolidates fragmented single-run Parquet files within `year=YYYY/` once partition file count reaches threshold (`>= 8`), atomically writing a single ZSTD(3) file and removing obsolete fragments only after verified close + rename.
4. **Anti-Peeking Prospective Governance:** `ProspectiveScorecard` embeds the frozen `prospective_rules_2026_11_28.json` parameters (`burn_in_end = 2026-11-27`, `oos_start = 2026-12-04`, `primary_readout_week = 104 / 2028-11-24`, `secondary_readout_week = 260 / 2031-11-21`). Confirmatory PASS/FAIL verdicts remain strictly locked (`ACCUMULATING_OOS_NO_PEEKING`) until fixed calendar readout; `SEC_5` quantiles are strictly expanding (`< t`, zero lookahead).
5. **Clean VSA Composition Root (`src/runtime/`):** Shared input loading (`tree_loader.rs`) and background supervision (`continuous_supervisor.rs`) live in `src/runtime/`, keeping `features/analytics/` pure compute (zero I/O) and `features/storage/` pure persistence (zero analytics coupling).

---

## 2. Implementation Phases (Phases 351–357)

| Phase | Deliverable | Target Modules | Verification Gate |
| :--- | :--- | :--- | :--- |
| **351** | Milestone 18 Roadmap & Core Event/Checkpoint Contracts | `docs/roadmap/milestone_18_*.md`, `src/core/continuous_types.rs` | Serializable, schema-versioned contracts (`schema_version = 1`) |
| **352** | Pure Regime Transition & Blind-Spot Diff Engine | `src/features/analytics/regime_transitions.rs` | Deterministic detection of `LeafChanged`, `RootRegimeChanged`, `BlindSpotOpened/Resolved`, `StaleSourceFreshened` + numeric metric diffs |
| **353** | Pure Prospective OOS Scorecard (`v1.1.0` Frozen Rules) | `src/features/analytics/prospective_scorecard.rs` | Exact M=2 Primary + M=5 Secondary tracking, strictly expanding Q80/Q20, anti-peeking status lock |
| **354** | Event-Sourced Parquet Store, Atomic Checkpoint & Compactor | `src/features/storage/continuous_ledger.rs`, `src/features/storage/diagnostic_ledger.rs` | Atomic checkpoint roundtrip, ZSTD(3) transitions & OOS Parquet roundtrip, non-destructive partition compaction |
| **355** | Composition Root: Shared Tree Loader & `ContinuousAnalyticsSupervisor` | `src/runtime/mod.rs`, `src/runtime/tree_loader.rs`, `src/runtime/continuous_supervisor.rs`, `src/main.rs` | Event/watermark-aware continuous loop (boot pass + periodic check + 24h compaction), zero duplicate runs on unchanged data |
| **356** | REST API Endpoints & Remote CLI Commands | `src/features/serving/diagnostic_tree_routes.rs`, `src/features/serving/continuous_routes.rs`, `rest.rs`, `src/core/cli_remote_analytics.rs`, `cli.rs` | `/v1/analytics/continuous/status`, `POST /v1/analytics/continuous/run`, `/v1/analytics/liquidity-tree/transitions`, `/v1/analytics/prospective-scorecard` + CLI commands |
| **357** | Integration Tests, `ADR-23`, `arc42-lint`, Coolify Deploy & Live Verification | `tests/test_m18_continuous_analytics.rs`, `docs/architecture/*.md` | Full test suite + VSA + `<400L` + `arc42-lint --strict` + live production verification |
