# Milestone 5: Macroeconomic Calendar & Vintages

**Phases:** 116 – 145  
**Domain:** FRED API, FinanceCalendar, ForexFactory, SDMX, Vintages, Economic Indicators  
**Primary Goal:** Capture global macroeconomic indicators, release calendars, and data revisions.

---

### Phase Catalog

- [ ] **PHASE-116: Federal Reserve (FRED) REST Client**  
  Implement authenticated client for FRED API (`api.stlouisfed.org/fred/series/observations`).
- [ ] **PHASE-117: FRED Key Indicator Seed Catalog**  
  Register core series: CPI (`CPIAUCSL`), Fed Funds Rate (`FEDFUNDS`), GDP (`GDP`), 10Y Yield (`DGS10`).
- [x] **PHASE-118: Data Vintages & Revision History Extractor**  
  Extract `realtime_start` and `realtime_end` to track how economic statistics are revised over time (Point-in-Time Bitemporal Engine).
- [x] **PHASE-119: FinanceCalendar API Client**  
  Implement client for keyless economic calendar JSON endpoints (`biquote.io/api/calendar`) for forward-looking event releases.
- [ ] **PHASE-120: ForexFactory Economic Calendar Scraper Adapter**  
  Scrape weekly economic release schedules with impact level categorization (Red, Orange, Yellow).
- [ ] **PHASE-121: Deterministic Event ID Generator**  
  Generate SHA-256 hash from `(country, indicator_code, scheduled_utc)` for idempotent event tracking.
- [x] **PHASE-122: Actual vs Forecast vs Previous Alignment**  
  Standardize triple-metric structure (`actual`, `forecast`, `previous`, `revised_previous`).
- [ ] **PHASE-123: Economic Event Surprise Index Calculator**  
  Compute economic surprise: $Surprise = \frac{Actual - Forecast}{|Forecast|} \times 100$.
- [x] **PHASE-124: ISO-3166-1 Alpha-2 Country Code Normalizer**  
  Normalize source country strings ("United States", "USA", "US") to standard `"US"`.
- [x] **PHASE-125: ISO-4217 Currency Code Normalizer**  
  Map impacted economies to official currency tickers (`USD`, `EUR`, `GBP`, `JPY`, `IDR`).
- [ ] **PHASE-126: Central Bank Decision Tracker**  
  Track FOMC, ECB, BOJ, BOE, and Bank Indonesia rate decisions and statement release dates.
- [ ] **PHASE-127: Non-Farm Payrolls (NFP) Specialized Pipeline**  
  Ingest Employment Situation releases including Unemployment Rate and Average Hourly Earnings.
- [ ] **PHASE-128: Inflation Basket Pipeline (CPI, Core CPI, PPI, PCE)**  
  Capture headline and core inflation measures across US, Europe, and Asia.
- [ ] **PHASE-129: Purchasing Managers' Index (PMI) Pipeline**  
  Ingest Manufacturing and Services PMIs (ISM and S&P Global).
- [ ] **PHASE-130: Real-Time Event Polling near Release Window**  
  Accelerate polling frequency to 1-second intervals during $T - 30s$ to $T + 60s$ of high-impact releases.
- [ ] **PHASE-131: Economic Release Parquet Partition Layout**  
  Partition files into `data/lake/economic/country=<CC>/year=<YYYY>/events.parquet`.
- [ ] **PHASE-132: Market Holiday Master Calendar**  
  Aggregate exchange holidays across NYSE, LSE, TSE, and IDX into `holidays.parquet`.
- [ ] **PHASE-133: Macro Release Notification Dispatcher**  
  Publish event release triggers to internal pub/sub channels for UI overlay alerts.
- [ ] **PHASE-134: Bank Indonesia (BI Rate) & Inflation Adapter**  
  Ingest Bank Indonesia benchmark interest rates (BI-Rate) and BPS Indonesian CPI.
- [ ] **PHASE-135: World Bank & OECD SDMX Structural Ingestion**  
  Parse international macroeconomic datasets formatted in SDMX-JSON.
- [ ] **PHASE-136: ASOF Join Verification with Market Candles**  
  Verify Polars `join_asof` between EUR/USD 1m candles and US CPI releases operates in $< 5$ ms.
- [ ] **PHASE-137: Historical Re-release Compactor**  
  Merge updated `actual` numbers into historical calendar files without altering original forecast records.
- [ ] **PHASE-138: Economic Indicator Metadata Store**  
  Store unit specifications ("%", "Thousands", "Index", "USD Billions") and publishing agency names.
- [ ] **PHASE-139: Event Impact Filter API**  
  Provide query filters: `GET /v1/economic/events?country=US&min_impact=3`.
- [ ] **PHASE-140: Multi-Language Event Title Mapping**  
  Support multilingual event names (English and Indonesian translations).
- [ ] **PHASE-141: Sentiment / Directional Bias Extractor**  
  Determine whether `actual > forecast` is fundamentally bullish or bearish for the currency.
- [ ] **PHASE-142: Zero-Cost Proxy Failover for ForexFactory**  
  Automatically switch to mirror proxies if primary ForexFactory scraper gets HTTP 429.
- [ ] **PHASE-143: Macro Data Freshness Watchdog**  
  Monitor whether scheduled events successfully populated an `actual` value within 5 minutes.
- [ ] **PHASE-144: Offline Historical Economic Dataset Packager**  
  Package 10 years of historical macroeconomic calendar data into compressed Parquet.
- [ ] **PHASE-145: Milestone 5 Integration Verification**  
  Verify simultaneous polling, storage, and ASOF joining of US macro calendar with market prices.
