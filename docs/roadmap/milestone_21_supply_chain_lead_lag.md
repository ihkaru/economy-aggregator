# Milestone 21: Persistent Supply-Chain Graph & Cross-Stock Lead-Lag Analyzer

## 1. Objective
Build a persistent, relational **Supply-Chain Graph Registry** (`target -> suppliers[]`) coupled with a **Zero-Lookahead Cross-Stock Lead-Lag Analyzer** across **20 major Tech / Semiconductor / Cloud companies**, surfacing the **Top 5 empirically validated targets** in a dedicated UI (`#supply-chain`).

## 2. Empirical Benchmark Results (20 Tech Targets, Discovery <=2022 vs OOS 2023-2026)
All 20 tech targets were benchmarked against 24 global supply-chain peers (`4063.T`, `8035.T`, `6857.T`, `ASML`, `AMAT`, `LRCX`, `KLAC`, `000660.KS`, `005930.KS`, `3037.TW`, `2317.TW`, `ASX`, `TER`, `SNPS`, `CDNS`, `CAMT`, `TSM`, `NVDA`, `MU`, `AVGO`, `MRVL`, `ANET`, `TXN`, `CSCO`) across lags `1..63d` and horizons `5d` & `21d`:

| Rank | Target | Survivors (`|r_disc|>=0.10 & |r_oos|>=0.12`) | Best Lag Driver | `r_disc` -> `r_oos` | Newey-West `t` |
|---:|---|---:|---|---|---:|
| **1** | **`ADI`** | 62 | `h=21d TXN_r21 lag=10d` | `-0.238 -> -0.269` | **`-2.67`** |
| **2** | **`NXPI`** | 55 | `h=21d spread_NXPI_vs_TSM_z63 lag=10d` | `-0.229 -> -0.232` | **`-2.64`** |
| **3** | **`INTC`** | 44 | `h=21d 4063.T_z63 lag=42d` | `+0.220 -> +0.217` | `+1.83` |
| **4** | **`AAPL`** | 27 | `h=21d spread_AAPL_vs_4063.T_z63 lag=42d` | `-0.201 -> -0.277` | **`-2.76`** |
| **5** | **`ORCL`** | 71 | `h=21d LRCX_r21 lag=1d` | `-0.199 -> -0.229` | `-1.92` |
| 6 | `MU` | 12 | `h=21d TSM_z63 lag=63d` | `+0.191 -> +0.295` | `+3.07` |
| 7 | `NVDA` | 30 | `h=21d ASX_r21 lag=1d` | `-0.191 -> -0.197` | `-2.29` |
| 8 | `SMCI` | 14 | `h=21d TER_z63 lag=42d` | `-0.188 -> -0.225` | `-2.08` |
| 9 | `MSFT` | 50 | `h=21d 4063.T_r21 lag=1d` | `-0.191 -> -0.182` | `-1.89` |
| 10 | `CRM` | 54 | `h=21d LRCX_r21 lag=1d` | `-0.181 -> -0.210` | `-1.73` |
| 11 | `ANET` | 45 | `h=21d ASX_r21 lag=2d` | `-0.179 -> -0.243` | `-2.78` |
| 12 | `TSM` | 8 | `h=21d spread_TSM_vs_4063.T_z63 lag=42d` | `-0.183 -> -0.179` | `-1.90` |
| 13 | `QCOM` | 31 | `h=21d spread_QCOM_vs_4063.T_z63 lag=21d` | `-0.177 -> -0.252` | `-2.12` |
| 14 | `AVGO` | 27 | `h=21d spread_AVGO_vs_2317.TW_z63 lag=63d` | `+0.175 -> +0.257` | `+3.00` |
| 15 | `AMZN` | 23 | `h=21d spread_AMZN_vs_MRVL_z63 lag=63d` | `+0.175 -> +0.196` | `+2.38` |
| 16 | `GOOGL` | 31 | `h=21d 4063.T_z63 lag=42d` | `+0.216 -> +0.174` | `+1.54` |
| 17 | `MRVL` | 15 | `h=21d spread_MRVL_vs_NVDA_z63 lag=63d` | `-0.255 -> -0.169` | `-1.58` |
| 18 | `AMD` | 20 | `h=21d TXN_r21 lag=63d` | `+0.193 -> +0.169` | `+1.65` |
| 19 | `DELL` | 17 | `h=21d spread_DELL_vs_ASML_z63 lag=42d` | `-0.163 -> -0.227` | `-2.51` |
| 20 | `META` | 10 | `h=21d ANET_r21 lag=21d` | `-0.131 -> -0.165` | `-2.20` |

## 3. Architecture & Implementation Plan
1. **Persistent Supply-Chain Registry (`src/core/supply_chain.rs`):**
   - Atomic JSON persistence (`supply_chain_graph.json` inside `lake_dir` volume) seeded with all 20 tech targets and their curated supplier relationships.
   - `GET /v1/supply-chain` lists the graph; `POST /v1/supply-chain/edges` adds/updates a `target -> supplier` relationship and **automatically subscribes** both symbols into `SubscriptionManager`; `DELETE /v1/supply-chain/edges` removes an edge.
2. **Core Subscription Defaults (`src/core/subscription.rs`):**
   - Persist all 20 tech targets + supply-chain & SMH macro/vol/rate tickers in `generate_core_defaults()`.
3. **Pure Analytics Engine (`src/features/analytics/supply_chain_analysis.rs`):**
   - Evaluates any `target` against its registered suppliers (or full universe) across lags `[1, 2, 5, 10, 21, 42, 63]` for `h=21d`: computes Discovery (`<=2022`) vs OOS (`>2022`) correlations, Newey-West HAC t-stat, `SURVIVED` vs `FLIPPED` status, and a causal Ridge forecast + `BUY / HOLD / SELL` signal using only consistent survivors.
4. **Top-5 UI View (`web/src/components/views/SupplyChainView.svelte`, `#supply-chain`):**
   - Quick-select tabs for the **Top 5 Tech Targets** (`ADI`, `NXPI`, `INTC`, `AAPL`, `ORCL`) plus dropdown for all 20 tech targets (`MU`, `NVDA`, `TSM`, etc.).
   - Interactive `Target -> Supplier` editor (add/remove supplier edges with automatic subscription).
   - Lead-Lag Stability Matrix (`SURVIVED` vs `FLIPPED` + Newey-West `t-stat`) and causal `BUY / HOLD / SELL` chart.
