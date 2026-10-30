# Milestone 4: Global Equities & Corporate Actions

**Phases:** 086 – 115  
**Domain:** Alpaca IEX Stream, Yahoo Finance, Corporate Actions, Stock Splits, Dividends  
**Primary Goal:** Accurate equities tracking with corporate action adjustments.

---

### Phase Catalog

- [ ] **PHASE-086: Alpaca Markets WebSocket Client**  
  Establish authenticated connection to Alpaca `wss://stream.data.alpaca.markets/v2/iex`.
- [ ] **PHASE-087: Alpaca Real-Time Minute Bars Stream Parser**  
  Subscribe to `bars` stream (`{"action": "subscribe", "bars": ["AAPL", "TSLA", "NVDA"]}`).
- [ ] **PHASE-088: Alpaca Real-Time Trades Stream Parser**  
  Subscribe to `trades` stream for live transaction tick analysis.
- [ ] **PHASE-089: Alpaca Historical Bars REST Paginator**  
  Fetch multi-year historical stock bars via `GET /v2/stocks/bars` with page tokens.
- [ ] **PHASE-090: Yahoo Finance Direct REST Scraper Client**  
  Query Yahoo Finance `/v8/finance/chart/{symbol}` endpoint without external Python libraries.
- [ ] **PHASE-091: Yahoo Finance User-Agent & Cookie Crumb Resolver**  
  Implement automated crumb extraction to avoid 401/403 HTTP blocks on Yahoo Finance.
- [ ] **PHASE-092: Global Equity Symbology Registry**  
  Support US Equities (`AAPL`), Indonesian Equities (`BBCA.JK`), Tokyo (`7203.T`), London (`SHEL.L`).
- [ ] **PHASE-093: Stock Split Detection & Multiplier Calculation**  
  Extract split events (e.g. 1:10 split ratio) and calculate historical price adjustment factors.
- [ ] **PHASE-094: Cash Dividend Event Ingestion**  
  Store dividend payout records in `data/lake/meta/dividends/`.
- [ ] **PHASE-095: Adjusted Close (`adj_close`) Column Integration**  
  Store both raw `close` and split/dividend-adjusted `adj_close` in `AggregatorCandle`.
- [ ] **PHASE-096: Backward Split-Adjustment Algorithm**  
  Calculate cumulative backward adjustment factor for seamless historical backtesting charts.
- [ ] **PHASE-097: NYSE & NASDAQ Market Hours State Machine**  
  Track market state: Pre-market (04:00-09:30), Regular (09:30-16:00), Post-market (16:00-20:00 EST).
- [ ] **PHASE-098: Market Holiday Calendar Integration**  
  Store NYSE and international exchange holiday dates to suppress false gap alerts.
- [ ] **PHASE-099: Market Timezone Normalization to UTC**  
  Convert local Eastern Time (ET) and Western Indonesia Time (WIB) to strict nanosecond UTC.
- [ ] **PHASE-100: Finnhub Stock Quote REST Client**  
  Implement Finnhub `/quote` endpoint poller as secondary real-time validation source.
- [ ] **PHASE-101: Stock Trading Halt Detection**  
  Detect circuit breaker trading halts (LULD - Limit Up / Limit Down) and log halt state.
- [ ] **PHASE-102: Share Volume vs Dollar Turnover Tracking**  
  Record `volume_base` (shares) and `volume_quote` (total dollar value traded).
- [ ] **PHASE-103: Top 500 US Equities (S&P 500) Bulk Ingestion**  
  Execute parallel historical backfill of S&P 500 components partitioned into Parquet.
- [ ] **PHASE-104: Ticker Delisting & Symbol Change Handler**  
  Track historical ticker migrations (e.g. `FB` -> `META`) via the symbology master table.
- [ ] **PHASE-105: ETF Asset Class Classification**  
  Categorize ETFs (SPY, QQQ, VOO) distinctly with underlying benchmark index metadata.
- [ ] **PHASE-106: Market Capitalization Metadata Extractor**  
  Extract shares outstanding and market cap tiers (Large, Mid, Small Cap).
- [ ] **PHASE-107: Sector & Industry Taxonomy Tagging**  
  Tag stocks with GICS (Global Industry Classification Standard) sectors.
- [ ] **PHASE-108: Multi-Exchange Price Discrepancy Detection**  
  Compare Alpaca (IEX feed) prices with Yahoo (Composite SIP feed) to flag spread variance.
- [ ] **PHASE-109: End-of-Day (EOD) Official Close Reconciliation**  
  Overwrite provisional 15:59:00 live bars with official 16:00:00 exchange closing auctions.
- [ ] **PHASE-110: Indonesian Market (IDX) Adapter**  
  Ingest top 45 Indonesian stocks (LQ45 index) from Yahoo Finance with WIB timezone offsets.
- [ ] **PHASE-111: Overnight Gap Analysis Calculator**  
  Compute opening gap percentage: $Gap\% = \frac{Open_{today} - Close_{yesterday}}{Close_{yesterday}} \times 100$.
- [ ] **PHASE-112: Zero-Volume Bar Policy**  
  Handle illiquid stocks: retain last traded close or omit empty bars based on configuration.
- [ ] **PHASE-113: Equities Lakehouse Partition Verification**  
  Confirm 10 years of daily AAPL data scans in $< 5$ ms.
- [ ] **PHASE-114: Corporate Action Re-indexing Tool**  
  Provide CLI tool to re-calculate `adj_close` on historical Parquet files when new split occurs.
- [ ] **PHASE-115: Milestone 4 Integration Verification**  
  Verify real-time Alpaca WebSocket ingestion combined with corporate action adjustments.
