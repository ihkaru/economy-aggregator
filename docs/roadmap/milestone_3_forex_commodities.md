# Milestone 3: Forex & Commodities Tick Lake

**Phases:** 056 – 085  
**Domain:** Dukascopy Tick Feeds, Twelve Data, OANDA, Spreads, Gold, Oil  
**Primary Goal:** High-precision institutional FX and commodity time-series persistence.

---

### Phase Catalog

- [ ] **PHASE-056: Dukascopy Historical URL Resolver**  
  Compute daily/hourly `.bi5` binary URLs based on instrument, year, month, day, hour.
- [ ] **PHASE-057: Dukascopy LZMA Binary Decompressor**  
  Implement high-speed LZMA decompression for binary `.bi5` chunks.
- [ ] **PHASE-058: Dukascopy Tick Binary Struct Parser**  
  Parse 20-byte packed binary records: `(time_offset_ms, ask_price, bid_price, ask_vol, bid_vol)`.
- [ ] **PHASE-059: Dukascopy Tick to OHLCV Synthesizer**  
  Aggregate millions of raw FX ticks into 1-minute and 5-minute canonical candles.
- [ ] **PHASE-060: Forex Bid/Ask/Mid Multi-Price Partitioning**  
  Store separate candles for `price_type: "bid"`, `"ask"`, and `"mid"`.
- [ ] **PHASE-061: Real-Time Spread Calculator**  
  Compute floating bid-ask spread: $Spread = Price_{ask} - Price_{bid}$ in pips.
- [ ] **PHASE-062: Twelve Data REST API Client**  
  Implement client for Twelve Data `/time_series` endpoint with API key injection.
- [ ] **PHASE-063: Twelve Data Credit-Bucket Rate Limiter**  
  Enforce strict limit of $\le$ 8 credits/minute and 800 credits/day.
- [ ] **PHASE-064: Twelve Data Real-Time WebSocket Client**  
  Establish WebSocket to `wss://ws.twelvedata.com/v1/quotes/price` for live FX quotes.
- [ ] **PHASE-065: OANDA v20 REST Client**  
  Implement client for OANDA v20 `/v3/instruments/{instrument}/candles`.
- [ ] **PHASE-066: OANDA Streaming Pricing Adapter**  
  Connect to OANDA live pricing stream for tick-by-tick FX updates.
- [ ] **PHASE-067: Major FX Pairs Master Registry**  
  Register EUR/USD, GBP/USD, USD/JPY, USD/CHF, AUD/USD, USD/CAD, NZD/USD.
- [ ] **PHASE-068: Cross-Currency Synthesizer (Triangulation)**  
  Synthesize minor cross rates (e.g. EUR/JPY = EUR/USD $\times$ USD/JPY) if direct feed drops.
- [ ] **PHASE-069: Precious Metals Ingestion (Spot Gold & Silver)**  
  Ingest Gold (`XAU/USD`) and Silver (`XAG/USD`) with ounce-based volume parsing.
- [ ] **PHASE-070: Energy Commodities Ingestion (Crude Oil)**  
  Ingest WTI Crude Oil (`USOIL` / `CL`) and Brent Crude (`UKOIL`).
- [ ] **PHASE-071: Forex Market Session Boundaries**  
  Implement Asian (Tokyo), European (London), and US (New York) session time filters.
- [ ] **PHASE-072: Weekend Market Close Detection**  
  Gracefully pause FX listeners during Friday 17:00 NY to Sunday 17:00 NY close.
- [ ] **PHASE-073: Sunday Market Open Gap Handler**  
  Detect and flag Sunday open price gaps against Friday close without false error alerts.
- [ ] **PHASE-074: Fractional Pip (Pipette) Precision Support**  
  Maintain 5-decimal precision (e.g. `1.08452`) and JPY 3-decimal precision (`154.231`).
- [ ] **PHASE-075: Multi-Month Historical FX Bulk Loader**  
  Download and ingest multi-year Dukascopy historical ticks in parallel using Tokio tasks.
- [ ] **PHASE-076: FX Tick Deduplication via Epoch Microseconds**  
  Deduplicate high-frequency FX quotes occurring within identical microsecond timestamps.
- [ ] **PHASE-077: Quote Volume Normalization to USD**  
  Convert non-USD quote volumes (e.g. EUR/GBP volume) to standardized USD value.
- [ ] **PHASE-078: Inverted Spread Sanity Check**  
  Reject corrupted broker ticks where $Price_{bid} > Price_{ask}$ (crossed market error).
- [ ] **PHASE-079: High-Spread News Spike Tagging**  
  Flag candles where spread expands by $> 5\times$ normal baseline (common during CPI/NFP).
- [ ] **PHASE-080: Commodity Expiration Calendar Support**  
  Track continuous commodity contract roll dates (front-month to next-month).
- [ ] **PHASE-081: Stooq Polish & Global Market Fallback Client**  
  Implement Stooq CSV historical downloader as a zero-cost secondary historical fallback.
- [ ] **PHASE-082: FX Lakehouse Partition Pruning Verification**  
  Verify scanning 1 year of 1-minute EUR/USD data takes $< 15$ ms in Polars.
- [ ] **PHASE-083: Memory Profile under Concurrent FX Pair Ingest**  
  Monitor memory footprint while concurrently processing 15 major FX pairs.
- [ ] **PHASE-084: Forex Symbology Alias Mapper**  
  Map `EUR_USD` (OANDA), `EURUSD=X` (Yahoo), `EUR/USD` (Dukascopy) to canonical `EUR/USD`.
- [ ] **PHASE-085: Milestone 3 End-to-End Verification**  
  Validate ingestion, storage, and retrieval across 10 FX pairs and 2 commodities.
