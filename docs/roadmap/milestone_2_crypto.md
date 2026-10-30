# Milestone 2: Cryptocurrency High-Frequency Ingestion

**Phases:** 026 – 055  
**Domain:** Binance & Bybit Public Streams, Perps, Funding Rates, Open Interest, CVD  
**Primary Goal:** Capture 24/7 high-throughput spot and perpetual futures market data.

---

### Phase Catalog

- [ ] **PHASE-026: Binance WebSocket Connection Manager**  
  Establish persistent `tokio-tungstenite` connection to `wss://stream.binance.com:9443`.
- [ ] **PHASE-027: WebSocket Auto-Reconnect & Ping/Pong Heartbeat**  
  Implement 3-minute ping keepalive and exponential backoff on dropped sockets.
- [ ] **PHASE-028: Binance Public Kline/Candle Stream Parser**  
  Parse `@kline_1m` payload into `AggregatorCandle` using zero-copy `serde_json`.
- [ ] **PHASE-029: Taker Buy Base & Quote Volume Extraction**  
  Extract fields `V` and `Q` from Binance kline to support Volume Delta / CVD.
- [ ] **PHASE-030: Multi-Pair Stream Multiplexing**  
  Support combined streams (`/stream?streams=btcusdt@kline_1m/ethusdt@kline_1m/...`).
- [ ] **PHASE-031: Binance Historical Klines REST Fetcher**  
  Implement `GET /api/v3/klines` backfill fetcher for initial historical seeding.
- [ ] **PHASE-032: REST Rate Limit Tracker (Weight Header Parser)**  
  Parse `x-mbx-used-weight-1m` header preventing IP bans by throttling before limit.
- [ ] **PHASE-033: Binance Futures WebSocket Adapter (Perpetuals)**  
  Connect to `wss://fstream.binance.com` for USDT-margined perpetual futures.
- [ ] **PHASE-034: Mark Price & Index Price Stream Ingestion**  
  Ingest `@markPrice@1s` streams into `AggregatorCandle` with `price_type: "mark"`.
- [ ] **PHASE-035: Funding Rate History Extractor**  
  Poll `GET /fapi/v1/fundingRate` storing historical 8-hour funding rates.
- [ ] **PHASE-036: Real-Time Open Interest (OI) Poller**  
  Poll `GET /fapi/v1/openInterest` storing base and quote contract open values.
- [ ] **PHASE-037: Liquidations Feed Listener**  
  Subscribe to `@forceOrder` stream recording major market liquidation spikes.
- [ ] **PHASE-038: Bybit V5 Public WebSocket Client**  
  Connect to `wss://stream.bybit.com/v5/public/linear` for Bybit perpetuals.
- [ ] **PHASE-039: Bybit Kline Stream Normalizer**  
  Normalize Bybit `kline.1.BTCUSDT` schema into canonical `AggregatorCandle`.
- [ ] **PHASE-040: Bybit Historical REST Backfill Adapter**  
  Implement `GET /v5/market/kline` paginator for Bybit historical data.
- [ ] **PHASE-041: Coinbase Advanced Public WebSocket Client**  
  Connect to `wss://advanced-trade-ws.coinbase.com` for institutional USD spot pricing.
- [ ] **PHASE-042: Coinbase Trade Stream Aggregator (1s Bar Synthesizer)**  
  Synthesize 1-second candles locally in memory from raw trades.
- [ ] **PHASE-043: Crypto Symbology Normalizer**  
  Map `BTCUSDT` (Binance/Bybit) and `BTC-USD` (Coinbase) to canonical `BTC/USDT` and `BTC/USD`.
- [ ] **PHASE-044: Smart Contract Address & Chain Metadata Extractor**  
  Attach token network identifiers (`ethereum`, `solana`) to `instruments_mapping`.
- [ ] **PHASE-045: 24/7 Session Flag & UTC 00:00 Daily Cutoff**  
  Enforce continuous 24/7 calendar tracking without market close gaps.
- [ ] **PHASE-046: High-Frequency WebSocket Packet Benchmark**  
  Stress test ingestion engine with 25,000 tick events/sec without memory growth.
- [ ] **PHASE-047: Dynamic Subscription Management via CLI**  
  Enable subscribing/unsubscribing to crypto symbols without stopping the service.
- [ ] **PHASE-048: WebSocket Buffer Overflow Protection**  
  Drop or downsample provisional ticks if processing queue reaches 90% capacity.
- [ ] **PHASE-049: Dual Ingestion Synchronization Test**  
  Verify simultaneous WebSocket stream and REST backfill merge without duplicates.
- [ ] **PHASE-050: Synthetic VWAP Calculator**  
  Calculate intraday VWAP on-the-fly from accumulated price and quote volume.
- [ ] **PHASE-051: Binance Order Book Depth (L2 Snapshot Ingest)**  
  Capture top 20 bid/ask depth snapshots every 5 seconds for order book replay.
- [ ] **PHASE-052: Micro-Chunk Compression Benchmark for Crypto**  
  Measure compression ratio of high-volatility 1-second crypto bars in Parquet.
- [ ] **PHASE-053: Spot vs Futures Spread Tracker**  
  Compute real-time basis spread: $Basis = Price_{perp} - Price_{spot}$.
- [ ] **PHASE-054: Dead-Letter Queue for Malformed Exchange Frames**  
  Route unknown payload schemas to `data/lake/dlq/crypto/` for debugging.
- [ ] **PHASE-055: Milestone 2 Integration Verification**  
  Run 24-hour continuous live ingestion test on top 10 crypto pairs.
