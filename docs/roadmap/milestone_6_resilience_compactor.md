# Milestone 6: Gap Healing & Lake Compaction

**Phases:** 146 – 175  
**Domain:** Small-Files Compactor, Automated Gap Healing, Rate Limiters, Dead-Letter Queues  
**Primary Goal:** Self-healing data pipelines with zero file fragmentation and no data loss.

---

### Phase Catalog

- [ ] **PHASE-146: Background Compactor Task Orchestrator**  
  Implement scheduled `tokio` background task running daily compaction routines.
- [x] **PHASE-147: Partition Micro-Chunk File Scanner**  
  Identify partition directories containing $> 100$ small `.parquet` chunks.
- [x] **PHASE-148: Multi-Chunk In-Memory Merger**  
  Load multiple micro-chunks, sort chronologically by `timestamp_utc`, and deduplicate by `natural_key`.
- [x] **PHASE-149: Consolidated Parquet Writer**  
  Write consolidated daily file: `compacted_YYYYMMDD.parquet` with dictionary encoding and ZSTD.
- [x] **PHASE-150: Atomic Compaction Switch & Cleanup**  
  Verify consolidated file integrity before deleting legacy micro-chunk files safely.
- [x] **PHASE-151: Compaction Space Savings Metric**  
  Log storage compression gains and file count reduction after every compaction run.
- [ ] **PHASE-152: Stream Disconnection Event Listener**  
  Hook into WebSocket connection drop events recording exact timestamp of disconnection $T_{drop}$.
- [ ] **PHASE-153: Stream Reconnection Gap Range Calculator**  
  Compute missing time window: $[T_{drop}, T_{reconnect}]$ upon successful socket reconnection.
- [ ] **PHASE-154: Automated REST Gap Healing Trigger**  
  Dispatch asynchronous REST batch requests to fetch the missing historical window automatically.
- [ ] **PHASE-155: Gap Healing Stitching Pipeline**  
  Tag healed records with `ingestion_mode: "reconciliation"` and insert into the storage buffer.
- [ ] **PHASE-156: Deduplication Priority Enforcer**  
  Ensure REST-healed records (`is_settled: true`) gracefully overwrite provisional live ticks.
- [ ] **PHASE-157: Token-Bucket Rate Limiter Implementation**  
  Build generic in-memory token bucket rate limiter with replenish rate and capacity parameters.
- [ ] **PHASE-158: Per-Vendor Rate Limit Configuration**  
  Configure distinct token buckets for Binance (1200 weight/min), Twelve Data (8/min), Alpaca (200/min).
- [ ] **PHASE-159: Exponential Backoff with Full Jitter**  
  Implement jittered backoff algorithm: $Sleep = \text{random}(0, \min(MaxSleep, Base \times 2^{retry}))$.
- [ ] **PHASE-160: Circuit Breaker Pattern Implementation**  
  Trip circuit breaker to OPEN state after 5 consecutive HTTP 5xx errors; probe after cooldown.
- [ ] **PHASE-161: Dead-Letter Queue (DLQ) File Storage**  
  Write unparseable or corrupted network payloads into `data/lake/dlq/YYYY/MM/` for audit.
- [ ] **PHASE-162: DLQ Retry CLI Utility**  
  Provide command `economy-aggregator dlq replay` to re-process payloads after bug fix.
- [ ] **PHASE-163: Stream Sequence Monotonicity Auditor**  
  Assert that within any single partition, timestamps and sequence IDs are strictly non-decreasing.
- [ ] **PHASE-164: Out-of-Order Message Absorber**  
  Gracefully insert late-arriving trade ticks into the appropriate historical time window.
- [ ] **PHASE-165: Disk Full Safety Shutdown**  
  Monitor available disk storage space; safely flush buffers and pause ingestion if free disk $< 500$ MB.
- [ ] **PHASE-166: Memory High-Water Mark Throttler**  
  Slow down WebSocket read polling if process RAM usage reaches 120 MB.
- [ ] **PHASE-167: Historical Gap Report Generator**  
  Scan analytical lake and produce report of missing hourly/daily candles per instrument.
- [ ] **PHASE-168: Automated Missing Candle Backfill CLI**  
  Provide command `economy-aggregator repair --symbol BTC/USDT --from 2026-01-01`.
- [ ] **PHASE-169: Parquet File CRC Checksum Verification**  
  Verify CRC32 checksums on all written Parquet chunks before serving to consumers.
- [ ] **PHASE-170: Live vs Settled Candle Audit Benchmark**  
  Quantify discrepancies between provisional live-built candles and final exchange-settled candles.
- [ ] **PHASE-171: Multi-Threaded Compaction Pool**  
  Run daily compaction across multiple symbol partitions in parallel via `rayon` thread pool.
- [ ] **PHASE-172: Compaction Lock Mechanism**  
  Prevent concurrent compactor runs from operating on the same partition directory.
- [ ] **PHASE-173: Emergency Flush Signal Handler (SIGINT / SIGTERM)**  
  Catch termination signals, immediately flush all in-memory buffers to disk, and exit cleanly with code 0.
- [ ] **PHASE-174: Chaos Engineering: Artificial Disconnection Test**  
  Inject random network disconnections every 15 minutes; assert zero lost candles over 6 hours.
- [ ] **PHASE-175: Milestone 6 Resilience Certification**  
  Certify that system recovers from network partitions, throttles, and disk write pauses autonomously.
