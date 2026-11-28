# Milestone 19: Epistemic Neutrality Upgrade (`liquidity-tree-v1.1.0`), Price-Neutral Crypto Leverage & Additional Sensor Wiring

> **Status:** Implemented & Verified (Phases 358–361)  
> **Scope:** Purge flawed `v1.0.0` derived diagnostic snapshots from production `lake/diagnostics/` (leaving raw `lake/observations/` 100% untouched), eliminate unobserved motive wording and nominal USD OI revaluation bias in `liquidity-tree-v1.1.0`, wire already-ingested lake sensors (`FRED_HY_OAS_PCT`, `FRED_SOFR_PCT`, `FRED_DTB3_PCT`, `FRED_STLFSI4_IDX`, `cot:GOLD:managed_money_net`, `deriv:BYBIT_BTC:open_interest_contracts`), and add keyless 8-hour BTC perpetual funding rate connectors (`binance_futures` & `bybit_v5`).

---

## Key Epistemic Fixes Enforced in `liquidity-tree-v1.1.0`

1. **Zero Unobserved Motive Language:** Removed *"dip-buying"*, *"profit-taking"*, *"defensive cash rotation"*, and single-window *"regime"* assertions across node summaries and terminal leaves; replaced with pure accounting and co-movement phrasing.
2. **Removed Pseudo-OOS Flag (`dash_for_cash_gld_out_and_bil_in`):** Eliminated from `Q2` so an unvalidated ~51%-FPR rule cannot masquerade as a regime marker.
3. **Price-Neutral Derivatives Leverage (`Q3`):** Switched crypto deleveraging predicate from price-revaluation-affected `bgeometrics_oi_usd` to contract-denominated `deriv:BYBIT_BTC:open_interest_contracts` alongside keyless 8-hour perpetual funding rates (`deriv:BINANCE_BTC:funding_rate_8h`, `deriv:BYBIT_BTC:funding_rate_8h`, stored `raw_as_is`).
4. **Yield-Carry Context (`Q2` / `Q5`):** Attached `FRED_DTB3_PCT` and `FRED_SOFR_PCT` so `BIL` inflows under ~4% short-term yields explicitly acknowledge carry/yield allocation alongside precautionary cash holding.
5. **Honest Lag & Credit Coverage (`Q3` / `Q6`):** Wired daily `FRED_HY_OAS_PCT` and weekly `FRED_STLFSI4_IDX` plus an explicit `h8_covers_latest_market_date` check that emits a `blind_spots` entry whenever Fed H.8 has not yet printed for the newest equity move.
6. **Accurate Epistemic Tier Assignment:** `Tier2Replicated` is strictly restricted to fresh FINRA margin contraction or fresh WRMFNS inflows — never applied when triggered only by crypto derivatives or accounting flows.
