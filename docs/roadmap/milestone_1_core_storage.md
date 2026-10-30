# Milestone 1: Core Engine & Columnar Storage Hardening

**Phases:** 001 – 025  
**Domain:** Core Types, Parquet / Arrow Serialization, Bounded Buffering, Sanity Filtering  
**Primary Goal:** Establish zero-overhead memory and disk persistence layer.

---

### Phase Catalog

- [x] **PHASE-001: Rust Workspace & Crate Structure Bootstrap**  
  Initialize Cargo workspace with zero-copy dependencies (`tokio`, `arrow`, `parquet`, `serde`, `chrono`).
- [x] **PHASE-002: Core Primitive Types Definition**  
  Implement `AssetClass`, `InstrumentType`, `TimeFrame`, `IngestionMode` with zero-allocation string matching.
- [x] **PHASE-003: AggregatorCandle Domain Model**  
  Define `AggregatorCandle` supporting TradFi, Crypto, Base/Quote Volume, Order Flow, and Natural Key.
- [x] **PHASE-004: Economic Event Data Model**  
  Define `EconomicEventRecord` with SDMX compatibility, ISO-3166 country codes, and ISO-4217 currency.
- [x] **PHASE-005: Lakehouse Storage Configuration**  
  Implement `LakehouseConfig` with configurable flush intervals, max rows, and directory creation.
- [x] **PHASE-006: Deterministic Hive Partition Resolver**  
  Implement `PartitionResolver` for `market/asset_class=<ac>/symbol=<sym>/timeframe=<tf>/year=<YYYY>/month=<MM>`.
- [x] **PHASE-007: Glob Pattern Generator for Partition Pruning**  
  Implement `resolve_glob_pattern` enabling sub-10ms date-range query scans.
- [x] **PHASE-008: Thread-Safe Bounded Micro-Batch Buffer**  
  Build `MicroBatchBuffer` with atomic threshold checking and bounded memory allocation.
- [x] **PHASE-009: In-Memory Deduplication Engine**  
  Implement `LakeWriter::deduplicate` where settled official records overwrite provisional live records.
- [x] **PHASE-010: Sanity Gate: Mathematical Invariant Filter**  
  Implement validation ensuring $high \ge \max(open, close)$ and $low \le \min(open, close)$.
- [x] **PHASE-011: Sanity Gate: Negative & Non-Positive Price Guard**  
  Reject corrupted feed payloads ($open \le 0$, $close \le 0$, $volume < 0$).
- [x] **PHASE-012: Sequence Gap Detector**  
  Implement `SanityGate::track_sequence` calculating missing message delta ($seq_{curr} - seq_{prev} - 1$).
- [ ] **PHASE-013: Apache Arrow Schema Definition**  
  Build explicit `arrow_schema::Schema` mapping `AggregatorCandle` to Arrow RecordBatch fields.
- [ ] **PHASE-014: Direct Arrow RecordBatch Builder**  
  Implement vectorized chunk serialization from `&[AggregatorCandle]` into `RecordBatch` without intermediate heap strings.
- [ ] **PHASE-015: Parquet Native ZSTD Chunk Writer**  
  Implement `ArrowWriter` integration writing Parquet files with ZSTD compression level 3.
- [ ] **PHASE-016: Parquet Dictionary Encoding for Categorical Columns**  
  Enable dictionary encoding on `canonical_symbol`, `source`, `ingestion_mode` to slash storage size by >75%.
- [ ] **PHASE-017: Configurable Row Group Sizing**  
  Enforce 32,768 rows per row group for optimal parallel multi-core decompression.
- [ ] **PHASE-018: Atomic Chunk File Renaming**  
  Write new Parquet files to a temporary `.tmp` extension and rename atomically to prevent partial reads.
- [ ] **PHASE-019: Memory Budget Guard (Max Heap Monitor)**  
  Implement background heap memory watchdog logging alerts if RAM usage exceeds 45 MB during active buffering.
- [ ] **PHASE-020: Backpressure Channel Integration**  
  Integrate `tokio::sync::mpsc::channel(50_000)` pausing socket ingestion if disk writer slows down.
- [ ] **PHASE-021: Non-Blocking Disk Thread Isolation**  
  Wrap all Parquet disk I/O in `tokio::task::spawn_blocking` to protect the async network event loop.
- [ ] **PHASE-022: File Metadata Header Caching**  
  Cache recently touched Parquet partition file lists to accelerate consecutive range queries.
- [ ] **PHASE-023: Symbology Rosetta Stone Registry**  
  Implement `InstrumentRegistry` with in-memory HashMap for instant $O(1)$ symbol translation.
- [ ] **PHASE-024: Symbology File Persistence**  
  Serialize and load `instruments_mapping.json` from `data/lake/meta/`.
- [ ] **PHASE-025: Milestone 1 Benchmark & Gauntlet**  
  Verify write throughput $\ge$ 100,000 candles/sec and memory consumption $\le$ 40 MB.
