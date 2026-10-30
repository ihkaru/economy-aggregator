# Milestone 9: One-Config Bootstrap, Deep Historical Backfill (2020–Present), & Agent-First CLI

**Phases:** 226 – 260  
**Domain:** Declarative Configuration, Secret Injection, Historical Backfill (Crypto, Stocks, Commodities, Macro), Agent-Ready CLI Engine  
**Primary Goal:** Enable any developer or AI agent to install, configure, backfill 6+ years of daily historical data, and inspect the engine via a deterministic CLI in a single step.

---

### Phase Catalog

#### A. One-Config-Away Bootstrap & Secret Hierarchy (Phases 226 – 232)
- [ ] **PHASE-226: Hierarchical Configuration Engine (`economy-aggregator.toml`)**  
  Implement 3-tier config loader: CLI flags $\to$ Environment Variables / Infisical $\to$ `economy-aggregator.toml` default file.
- [ ] **PHASE-227: Infisical Universal Auth Secret Resolver**  
  Integrate zero-leak secret injection via Infisical machine credentials (`INFISICAL_CLIENT_ID`, `INFISICAL_CLIENT_SECRET`) for vendor API keys.
- [ ] **PHASE-228: `economy-aggregator config init` Command**  
  Generate production-ready, fully commented template `economy-aggregator.toml` with default 2020 backfill parameters.
- [ ] **PHASE-229: `economy-aggregator config validate` Command**  
  Validate syntax, directory write permissions, and network connectivity to configured vendor endpoints with `--json` output.
- [ ] **PHASE-230: `economy-aggregator config show` Command**  
  Display active configuration with sensitive tokens/passwords automatically masked (e.g. `sk_***`).
- [ ] **PHASE-231: Declarative Start-Year Range Selector**  
  Support `start_year = 2020` or `start_year = "all"` in config to automatically calculate earliest timestamp boundaries per provider.
- [ ] **PHASE-232: Environment Variable Fallback Loader (`.env` & System ENV)**  
  Load local `.env` if present without committing secrets to git, falling back to system environment variables.

---

#### B. Top Asset Universe Definition (Phases 233 – 238)
- [ ] **PHASE-233: Top 20 Cryptocurrency Universe Catalog**  
  Register top 20 crypto assets (`BTC`, `ETH`, `BNB`, `SOL`, `XRP`, `DOGE`, `ADA`, `AVAX`, `TRX`, `LINK`, `SUI`, `SHIB`, `NEAR`, `DOT`, `LTC`, `BCH`, `UNI`, `PEPE`, `ICP`, `APT`) with `USDT` and `USD` base pairs.
- [ ] **PHASE-234: Top 25 Technology & Cloud Stocks Catalog**  
  Register mega-cap tech symbols (`AAPL`, `MSFT`, `NVDA`, `GOOGL`, `AMZN`, `META`, `TSM`, `AVGO`, `ORCL`, `CRM`, `AMD`, `ADBE`, `QCOM`, `CSCO`, `INTC`, `IBM`, `TXN`, `AMAT`, `NOW`, `INTU`, `MU`, `ACN`, `PANW`, `LRCX`, `ADI`).
- [ ] **PHASE-235: Top 25 Financial & Banking Stocks Catalog**  
  Register mega-cap financial institutions (`JPM`, `V`, `MA`, `BAC`, `WFC`, `MS`, `GS`, `BLK`, `AXP`, `C`, `SPGI`, `BX`, `MMC`, `CB`, `PGR`, `USB`, `PNC`, `SCHW`, `CME`, `ICE`, `AON`, `TRV`, `MCO`, `AIG`, `ALL`).
- [ ] **PHASE-236: Top 25 Healthcare & Biotech Stocks Catalog**  
  Register top pharmaceutical & health leaders (`LLY`, `NVO`, `UNH`, `JNJ`, `ABBV`, `MRK`, `TMO`, `ABT`, `AZN`, `DHR`, `PFE`, `ISRG`, `BMY`, `AMGN`, `SYK`, `ELV`, `GILD`, `CVS`, `MDT`, `CI`, `REGN`, `VRTX`, `ZTS`, `BDX`, `BSX`).
- [ ] **PHASE-237: Top 25 Energy, Consumer & Industrial Stocks Catalog**  
  Register top energy and defensive staples (`XOM`, `CVX`, `SHEL`, `TTE`, `COP`, `BP`, `EOG`, `SLB`, `WMT`, `COST`, `PG`, `KO`, `PEP`, `HD`, `MCD`, `TM`, `NKE`, `CAT`, `GE`, `RTX`, `BA`, `HON`, `UNP`, `LMT`, `DE`).
- [ ] **PHASE-238: Top 10 Major Commodities Catalog**  
  Register precious metals, energy, and ags (`XAU/USD` Gold, `XAG/USD` Silver, `XPT/USD` Platinum, `XPD/USD` Palladium, `WTI/USD` Crude Oil, `BRENT/USD` Brent, `NG/USD` Natural Gas, `COPPER/USD` Copper, `CORN/USD` Corn, `WHEAT/USD` Wheat).

---

#### C. Deep Historical Daily Backfill Engine (2020–Present) (Phases 239 – 248)
- [ ] **PHASE-239: Crypto 1D Daily Backfill Paginator (2020–Present)**  
  Implement automated chunked REST paginator fetching Binance/Bybit 1D bars from `2020-01-01T00:00:00Z` to current date.
- [ ] **PHASE-240: Equities 1D Daily Backfill Fetcher with Split Adjustments**  
  Ingest daily stock bars from 2020 with corporate action handling (`adj_close` dividend and split factoring).
- [ ] **PHASE-241: Commodities 1D Daily Historical Ingestion**  
  Fetch spot and futures continuous historical daily commodity prices from 2020.
- [ ] **PHASE-242: Historical Economic Calendar Multi-Year Backfill**  
  Fetch and categorize major macro events (CPI, PCE, Non-Farm Payrolls, Central Bank Rate Decisions) from 2020 to present.
- [ ] **PHASE-243: Backfill Rate-Limit Budgeting & Token-Bucket Throttler**  
  Enforce non-blocking token bucket pausing requests before hitting exchange IP rate limits (e.g. Binance 1200 weight/min).
- [ ] **PHASE-244: Checkpointed Backfill Progress State (`backfill_progress.json`)**  
  Save per-symbol progress watermark timestamps so interrupted backfills resume without re-downloading existing years.
- [ ] **PHASE-245: Multi-Asset Parallel Backfill Coordinator**  
  Run asynchronous multi-worker backfill tasks with bounded concurrency (`tokio::sync::Semaphore`).
- [ ] **PHASE-246: Historical Micro-Batch to Parquet Streaming Flush**  
  Direct backfill streams into `LakeWriter` in 10,000-candle batches to preserve $\le 50$ MB RAM budget during deep backfill.
- [ ] **PHASE-247: Post-Backfill Automatic Partition Compaction**  
  Trigger `LakeCompactor` upon backfill completion to merge historical batches into clean monthly Parquet files.
- [ ] **PHASE-248: Backfill Integrity & Gap Verification Auditor**  
  Scan generated historical partitions for missing trading days (excluding exchange weekends/holidays) and report completeness percentage.

---

#### D. Agent-First CLI Architecture (Phases 249 – 260)
- [ ] **PHASE-249: Universal JSON Envelope Standard**  
  Format all CLI command outputs into machine-readable envelope: `{"success": true, "error": null, "data": {...}, "next_steps": [...]}`.
- [ ] **PHASE-250: Typed Deterministic Exit Codes**  
  Enforce strict exit codes: `0` (Success), `1` (Bad Configuration/Args), `2` (Network/Rate-Limit Error), `3` (Storage/I/O Error), `4` (Integrity Failure).
- [ ] **PHASE-251: Zero-Interactivity & Non-Blocking Execution**  
  Implement `--yes`, `--force`, and `--non-interactive` flags; eliminate stdin prompts when piped to AI agent subprocesses.
- [ ] **PHASE-252: `economy-aggregator health` CLI Subcommand**  
  Return system health, uptime, disk lake size, RAM consumption, and active ingestion statuses.
- [ ] **PHASE-253: `economy-aggregator backfill start` CLI Subcommand**  
  Trigger backfill with `--asset [crypto|stocks|commodities|macro|all]`, `--start-year [YYYY|all]`, and `--dry-run`.
- [ ] **PHASE-254: `economy-aggregator backfill status` CLI Subcommand**  
  Report real-time progress percentages, estimated time remaining (ETA), and completed date ranges.
- [ ] **PHASE-255: `economy-aggregator query candles` CLI Subcommand**  
  Query and dump historical Parquet candles directly to stdout in JSON or NDJSON format with sub-10ms response time.
- [ ] **PHASE-256: `economy-aggregator query events` CLI Subcommand**  
  Query macroeconomic calendar releases by country, importance, and date range.
- [ ] **PHASE-257: `economy-aggregator lake stats` CLI Subcommand**  
  Inspect partition hierarchy, total rows, compression ratio, and storage savings.
- [ ] **PHASE-258: `economy-aggregator lake compact` CLI Subcommand**  
  Manually or periodically trigger lake compaction with `--partition` filter and `--min-files` threshold.
- [ ] **PHASE-259: AI Agent Skill Package (`SKILL.md`)**  
  Expose a self-describing skill manifest teaching LLM agents when and how to invoke each CLI command.
- [ ] **PHASE-260: Milestone 9 Integration & Gauntlet Test Suite**  
  Verify end-to-end headless CLI execution, mock agent invocation, and 2020 backfill simulation in automated tests.
