# Milestone 7: Serving Layer & High-Speed Streaming

**Phases:** 176 – 200  
**Domain:** Axum REST Server, Cursor Pagination, WebSocket Pub/Sub, Apache Arrow Flight IPC  
**Primary Goal:** Serve historical and live market/macro data with sub-10ms response times.

---

### Phase Catalog

- [ ] **PHASE-176: Axum HTTP Server Bootstrap**  
  Initialize high-performance `axum` router listening on configurable port (default `:8080`).
- [ ] **PHASE-177: Health Check & System Status Endpoint**  
  Implement `GET /health` and `GET /status` returning uptime, RAM usage, active streams, and lake stats.
- [ ] **PHASE-178: Instruments Metadata Endpoint**  
  Implement `GET /v1/instruments` with filtering by `asset_class`, `is_active`, and keyword search.
- [ ] **PHASE-179: Single Instrument Lookup Endpoint**  
  Implement `GET /v1/instruments/{symbol}` returning full metadata and vendor mappings.
- [ ] **PHASE-180: Historical Candle Range Query Endpoint**  
  Implement `GET /v1/market/candles?symbol=...&timeframe=...&from=...&to=...`.
- [ ] **PHASE-181: Cursor-Based Pagination for Deep Historical Queries**  
  Implement opaque cursor tokens (`next_cursor`) preventing offset-based database degradation.
- [ ] **PHASE-182: Parquet Partition Pushdown Query Engine**  
  Translate query time range into exact Hive directory paths, bypassing irrelevant partition files.
- [ ] **PHASE-183: Response Compression Middleware (Gzip & Zstandard)**  
  Integrate `tower-http` compression middleware reducing payload size over HTTP by up to 80%.
- [ ] **PHASE-184: ETag & HTTP Caching Headers**  
  Generate deterministic ETags based on file modification times for settled historical datasets.
- [ ] **PHASE-185: Latest Quote / Snapshot Endpoint**  
  Implement `GET /v1/market/snapshot/{symbol}` returning the most recent candle and 24h change.
- [ ] **PHASE-186: WebSocket Broadcast Server Architecture**  
  Build Axum WebSocket endpoint (`GET /v1/ws`) supporting concurrent client connections.
- [ ] **PHASE-187: Client Topic Subscription Protocol**  
  Support JSON subscribe/unsubscribe commands: `{"action": "subscribe", "channels": ["candles:BTC/USDT:1m"]}`.
- [ ] **PHASE-188: Real-Time Pub/Sub Fan-Out Engine**  
  Broadcast newly ingested candles from the ingestion pipeline directly to connected WebSocket clients.
- [ ] **PHASE-189: Client Heartbeat & Idle Disconnect Manager**  
  Send WebSocket ping frames every 30 seconds; disconnect zombie clients that fail to respond.
- [ ] **PHASE-190: Slow Consumer Protection (Drop Slow Clients)**  
  Enforce bounded per-client outgoing buffers; disconnect clients lagging by $> 1,000$ messages.
- [ ] **PHASE-191: Economic Calendar Query Endpoint**  
  Implement `GET /v1/economic/events?country=...&from=...&to=...&min_impact=...`.
- [ ] **PHASE-192: Next Upcoming High-Impact Event Endpoint**  
  Implement `GET /v1/economic/next` returning the closest scheduled Tier-3 (Red) economic release.
- [ ] **PHASE-193: Raw Parquet Direct Download Endpoint**  
  Implement `GET /v1/download/market/{symbol}/{year}/{month}` for direct multi-megabyte bulk transfers.
- [ ] **PHASE-194: Apache Arrow Flight Server Bootstrap**  
  Initialize gRPC Arrow Flight server on `:50051` for high-throughput zero-copy DataFrame transport.
- [ ] **PHASE-195: Arrow Flight `DoGet` Ticket Handler**  
  Stream Arrow RecordBatches directly from Parquet memory buffers without JSON serialization overhead.
- [ ] **PHASE-196: Python Polars Client Integration Example**  
  Provide example script connecting Polars directly to Arrow Flight endpoint via `pyarrow.flight`.
- [ ] **PHASE-197: CORS & Security Headers Middleware**  
  Configure Cross-Origin Resource Sharing (CORS) allowing browser-based TradingView charts to connect.
- [ ] **PHASE-198: REST API Request Concurrency Benchmark**  
  Verify REST server sustains 10,000 requests/sec with average latency $< 8$ ms.
- [ ] **PHASE-199: WebSocket Concurrent Connection Benchmark**  
  Stress test WebSocket broadcaster with 2,000 concurrent connected browser sessions.
- [ ] **PHASE-200: Milestone 7 API Certification**  
  Validate seamless data flow from live ingestion $\to$ lake storage $\to$ REST & WebSocket serving.
