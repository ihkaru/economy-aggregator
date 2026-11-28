# Milestone 20: Synchronized Liquidity & Capital Flow Atlas UI & Backend API

> **Status:** Implemented & Verified (Phases 362–367)  
> **Scope:** Build a high-density, anti-misinterpretation **6-Tank Liquidity & Capital Flow Atlas** directly inside the embedded Svelte Web Terminal (`https://market.dvlpid.my.id`), powered by a single-shot zero-lookahead backend endpoint (`GET /v1/analytics/liquidity-atlas`) with Left/Right historical date shifting (`-4W / -1W / Date Picker / +1W / +4W / LATEST`), synchronized crosshairs across all 6 tanks, explicit `data_layer` (*As-Is / Unit-Scaled / Upstream SA / Derived*) badges, honest `UNPRINTED / LAG` markers, and interactive `liquidity-tree-v1.1.0` diagnosis + transitions inspection.

---

## 1. The 6 Synchronized Capital Flow Tanks

| # | Tank ID | Primary Series (Histogram/Area) | Overlay / Mode Toggles | Anti-Misinterpretation Safeguards |
| :---: | :--- | :--- | :--- | :--- |
| **1** | `equities_spy` | `SPY` Exact Primary Unit Flow (`$B/wk`) | `SPY` Close Price + `SPY` Weekly ΔAUM (`$B`) | Visually separates mark-to-market price revaluation from primary AP share creations/redemptions |
| **2** | `crypto_btc` | US Spot BTC ETF Flow (`$B/wk`) | Mode A: `ΔStablecoin Mcap ($B/wk)` · Mode B: `Bybit BTC OI (Contracts)` + `8h Funding (bps)` | Contract-denominated OI + funding eliminates nominal USD price-revaluation bias |
| **3** | `gold_gld` | `GLD` Exact Primary Unit Flow (`$B/wk`) | `CFTC Gold Managed Money Net (contracts)` + `GLD` Close | Explicitly scoped to US financial gold ETF + COMEX futures (excludes Asian physical / central banks) |
| **4** | `cash_short_term` | `BIL` T-Bill Primary Flow (`$B/wk`) | `FRED DTB3 (%)` & `SOFR (%)` + `Retail MMF (WRMFNS $T)` + `ON RRP ($B)` | Displays short-term T-Bill yield right over BIL inflows so yield/carry vs precautionary holding stays visible |
| **5** | `credit_banks` | Fed H.8 Bank Deposits (`$T, SA`) | Mode A: `Daily HY OAS (%)` · Mode B: `H.8 Bank Credit ($T)` + `JNK Flow` · Mode C: `FINRA Margin Debt ($B)` | Truncates strictly at actual release date (`latest_date`) with explicit amber `LAG / UNPRINTED` indicator |
| **6** | `macro_fed_pipes` | Fed `WALCL − TGA` (`$T`, ex-RRP) | `US Treasury TGA ($B)` + `SPTL` Long Treasury Flow (`$B/wk`) | Uses default ex-RRP balance-sheet metric to avoid RRP-MMF accounting double-counting |

---

## 2. Implementation Phases (Phases 362–367)

| Phase | Deliverable | Target Modules | Verification Gate |
| :--- | :--- | :--- | :--- |
| **362** | Milestone 20 Roadmap & Atlas DTO Contracts | `docs/roadmap/milestone_20_*.md`, `src/core/atlas_types.rs` | Strongly-typed JSON schema with per-series `data_layer`, `latest_date`, `stale_lag_days`, and zero-lookahead bounds |
| **363** | Zero-Lookahead Single-Shot Atlas Loader | `src/runtime/atlas_loader.rs`, `src/runtime/atlas_tanks.rs` | Filters every observation and candle strictly `<= as_of_date`; builds all 6 tanks + `v1.1.0` tree in one pass |
| **364** | REST Route, Remote CLI & Integration Tests | `src/features/serving/atlas_routes.rs`, `rest.rs`, `cli_remote_analytics.rs`, `tests/test_m20_liquidity_atlas.rs` | `GET /v1/analytics/liquidity-atlas?as_of=&range=&window_weeks=` verified with historical date-shift zero-lookahead tests |
| **365** | Reusable Synchronized Tank Chart & Diagnostic Strip | `web/src/components/atlas/AtlasTankCard.svelte`, `AtlasDiagnosticStrip.svelte` | `lightweight-charts v5` multi-series charts, synced hover date readout, overlay mode switches, lineage popover |
| **366** | Liquidity Atlas View with Left/Right Date Controls | `web/src/components/views/LiquidityAtlasView.svelte`, `App.svelte`, `HeaderBar.svelte`, `AppLauncherModal.svelte` | `-4W / -1W / Date Picker / +1W / +4W / LATEST` + keyboard `ArrowLeft`/`ArrowRight`, responsive grid, `>=44px` mobile targets |
| **367** | Full Rust + Bun Build Gauntlet, `ADR-25`, Coolify Deploy & Live Verification | `docs/architecture/09_architecture_decisions.md`, `web/dist/` | `cargo test` + `bun run build` + `detect-ui-hygiene` + `vsa-boundary-checker` + `arc42-lint --strict` + live endpoint check |
