# Milestone 8: Observability, Replay & Production Hardening

**Phases:** 201 – 225+  
**Domain:** OpenTelemetry, Prometheus, Historical Replay Engine, Chaos Testing, Distroless Packaging  
**Primary Goal:** Production enterprise readiness, continuous observability, and quant replay feeds.

---

### Phase Catalog

- [ ] **PHASE-201: Structured Tracing with OpenTelemetry**  
  Integrate `tracing-opentelemetry` emitting distributed trace spans for every ingestion micro-batch.
- [ ] **PHASE-202: Prometheus Metrics Exporter Endpoint**  
  Implement `GET /metrics` exporting standard Prometheus gauges, counters, and histograms.
- [ ] **PHASE-203: Key Performance Indicators (KPI) Gauges**  
  Track: `active_streams_count`, `ingested_messages_total`, `ingest_latency_seconds`, `memory_rss_bytes`.
- [ ] **PHASE-204: Parquet Lake Disk Space Gauge**  
  Export total analytical lake byte size and partition directory count to Prometheus.
- [ ] **PHASE-205: Gap Healing Frequency Counter**  
  Track number of detected sequence gaps and healed candles per hour.
- [ ] **PHASE-206: Grafana Production Dashboard Template**  
  Create ready-to-import Grafana JSON dashboard visualizing throughput, RAM, and stream health.
- [ ] **PHASE-207: Historical Tick/Bar Replayer Engine**  
  Build backtest replay service reading Parquet chunks and simulating live real-time WebSocket feeds.
- [ ] **PHASE-208: Configurable Replay Speed Multiplier**  
  Support $1\times$ (real-time), $10\times$, $100\times$, and maximum-throughput replay modes.
- [ ] **PHASE-209: Multi-Asset Historical Synchronization**  
  Replay ticks from equities, crypto, and macro releases aligned chronologically on a shared virtual clock.
- [ ] **PHASE-210: Backtest Slippage & Spread Injection Model**  
  Simulate realistic execution slippage during high-volatility macro news release replays.
- [ ] **PHASE-211: CLI Interactive Control Mode**  
  Provide rich CLI subcommands: `economy-aggregator start`, `status`, `compact`, `backfill`, `replay`.
- [ ] **PHASE-212: Configuration File Hot-Reloading**  
  Support reloading `config.toml` without restarting the process or dropping active WebSockets.
- [ ] **PHASE-213: Self-Healing Memory Watchdog**  
  Gracefully trigger buffer drain and GC hints if operating system emits low-memory signals.
- [ ] **PHASE-214: Cross-Platform Single-Binary Compilation**  
  Verify cross-compilation builds for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-gnu`.
- [ ] **PHASE-215: Distroless Minimal Docker Container**  
  Build ultra-secure, scratch/distroless Docker image with final image size $< 25$ MB.
- [ ] **PHASE-216: Systemd Service Unit Configuration**  
  Provide production `economy-aggregator.service` file with `Restart=always` and memory limits.
- [ ] **PHASE-217: Automated Log Rotation & Compression**  
  Rotate local logs daily with ZSTD compression; purge logs older than 14 days.
- [ ] **PHASE-218: Cryptographic Hash Verification on Lake Partitions**  
  Generate daily SHA-256 manifest files for all partition files to ensure data tamper-resistance.
- [ ] **PHASE-219: High-Availability Failover (Active-Passive Standby)**  
  Implement primary/standby node heartbeats via shared filesystem locks.
- [ ] **PHASE-220: Chaos Monkey: High-Latency Simulation**  
  Simulate 2-second network latency spikes; verify buffer backpressure prevents memory exhaustion.
- [ ] **PHASE-221: Chaos Monkey: Corrupt Data Injection Test**  
  Inject random bit-flips into incoming JSON payloads; assert Sanity Gate catches 100% of anomalies.
- [ ] **PHASE-222: 7-Day Continuous Longevity Test**  
  Run 7-day uninterrupted live ingestion across 50 instruments; confirm zero memory drift ($< 5\%$ variance).
- [ ] **PHASE-223: Regulatory Compliance Audit Trail (MiFID II Clock Sync)**  
  Record microsecond delta between exchange event timestamp and local receive timestamp.
- [ ] **PHASE-224: Quantitative Research SDK / Python Bindings**  
  Publish lightweight PyO3 Python binding `economy_aggregator` for native zero-copy dataframe extraction.
- [ ] **PHASE-225: Enterprise Production Sign-Off & Release v1.0.0**  
  Final certification across all 225 phases, complete with comprehensive performance and security audits.
