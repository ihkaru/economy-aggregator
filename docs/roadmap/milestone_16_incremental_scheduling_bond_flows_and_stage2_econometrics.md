# Milestone 16: Incremental Frequency-Aware Scheduling, Bond/Global Flows & Stage-2 Spillover Engine

**Phases:** 333 – 344  
**Domain:** Per-Frequency/Watermark Incremental Ingestion, Sub-20ms Footer-Based Catalog, Expanded Bond/T-Bill ETF & Weekly MMF Coverage, & Pure-Rust Stage-2 Diebold-Yilmaz Connectedness Engine.  
**Primary Goal:** Menutup dua kategori keterbatasan tersisa dari M15: (1) menghentikan penarikan ulang *full history* setiap 15 menit agar penarikan mematuhi frekuensi & watermark asli tiap sumber sekaligus menjaga `/v1/observations/catalog` tetap instan (<20ms), serta (2) menutup celah data "Balon Air" (*Weekly MMF*, *T-Bill / High-Yield / Long Treasury ETF flows*, *ECB/BOJ global liquidity*) dan membangun mesin ekonometrika Tahap 2 (*spillover/connectedness*) langsung di Rust.

---

## 1. Hasil Eksplorasi Celah & Peluang Empiris (Basis Desain M16)

1. **Audit Scheduler & Storage Saat Ini:**
   - `IngestionOrchestrator::start()` (`src/features/ingestion/orchestrator.rs`) hanya memakai satu timer `tick_counter % 15 == 0` (15 menit) untuk seluruh 22 *subscriptions*, **mengabaikan** `SubscriptionItem::poll_interval_secs`.
   - `CatchUpEngine::catch_up_subscription()` (`src/features/ingestion/catch_up.rs`) memanggil `sync_observations()` **full history** (~64.080 baris/siklus) tanpa mengecek watermark seri maupun frekuensi (`1d`/`1w`/`1M`), yang menciptakan file chunk mikroskopis berulang, memicu duplikasi snapshot pada tanggal yang sama (`stockanalysis`/`deribit`), dan memicu `HTTP 429` pada `bgeometrics`.
   - `ObservationReader::summarize_coverage()` (`src/features/storage/observation_reader.rs`) membaca ulang **100% seluruh baris Parquet ke memori** untuk menyusun `/v1/observations/catalog`, sehingga rentan *timeout* (>30 detik) ketika chunk belum terkompaksi.
2. **Peluang Penutupan Celah Data Baru (Terverifikasi HTTP 200 di Produksi):**
   - **FRED Weekly & Global (`FRED_API_KEY`):**
     - `WRMFNS`: *Retail Money Market Funds* (**Mingguan**, sejak 1980 — menggantikan celah penghalusan `MMF` bulanan OFR).
     - `ECBASSETSW`: *Eurosystem Central Bank Assets* (**Mingguan**, sejak 1999) & `JPNASSETS` (*Bank of Japan Total Assets*, bulanan) — menutup celah likuiditas bank sentral non-AS.
     - `SOFR` (Harian), `STLFSI4` (*St. Louis Fed Financial Stress Index*, Mingguan), `WM2NS` (*Weekly M2*, Mingguan sejak 1980).
   - **SSGA Official Fixed Income & Cash ETF XLSX (`navhist-us-en-{ticker}.xlsx` → `HTTP/2 200`):**
     - `BIL` (1–3M US T-Bill ETF — proksi aliran harian **Fiat Cash Park**).
     - `JNK` (High Yield Bond ETF — proksi pelarian kredit korporat / **Deleveraging**).
     - `SPTL` (Long-Term US Treasury ETF — proksi aliran *flight-to-safety* obligasi jangka panjang).

---

## 2. Granular Implementation Phases (Phase 333 – 344)

### Group A: Frequency- & Watermark-Aware Incremental Ingestion + Fast Catalog (Phase 333 – 336)

- **Phase 333 — Per-Subscription Cadence Scheduler (`src/features/ingestion/orchestrator.rs`, `src/core/subscription.rs`):**
  - Simpan peta `HashMap<String, DateTime<Utc>>` (`last_attempt_by_sub`) di loop supervisor `IngestionOrchestrator::start()`.
  - Evaluasi setiap 60 detik hanya subscription yang selisih waktunya telah mencapai `sub.poll_interval_secs` (default: `300s` Yahoo/TV, `600s` Calendar/Prediction, `3600s` Crypto/Daily Observations, `21600s` TradFi/Weekly Macro Observations).
  - *Kriteria Lolos:* Unit test memverifikasi subscription `3600s` tidak dipicu pada tick 15 menit.

- **Phase 334 — Series Watermark Gate, Incremental Delta Fetch & Snapshot Dedup Guard (`src/features/ingestion/liquidity_sync.rs`, `src/features/storage/observation_reader.rs`, `observation_writer.rs`, `binance_rest.rs`):**
  - Tambahkan pembacaan watermark ringan per `(series_id, source)` (`ObservationWatermark`) sebelum pemanggilan HTTP:
    - **Seri `1M`** (`finra_xlsx`, `ofr_stfm`, `FRED JPNASSETS`): lewati penarikan bila `latest_obs_ts` sudah bulan terbaru yang rilis / dicek <24 jam.
    - **Seri `1w`** (`cftc_cot`, `fed_h41_html`, `FRED WALCL/WTREGEN/TOTBKCR/WRMFNS/ECBASSETSW/STLFSI4`): lewati penarikan bila sudah ditarik pasca-jadwal rilis minggu berjalan.
    - **Seri `1d` Multi-Tahun (Incremental Mode):**
      - `TreasuryFetcher`: hanya menarik `current_year` pada `fetch_tbill_3m_year` bila tahun `2016..cur_year-1` sudah ada di lakehouse; gunakan filter tanggal terbaru untuk TGA bila watermark tersedia.
      - `OccOiFetcher`: cukup tarik 2 bulan terakhir bila histori 24 bulan awal sudah terisi.
      - `CoinMetricsFetcher`: gunakan `start_date = watermark - 7 hari` (agar revisi `flash → final` tetap tercatat) alih-alih selalu menarik 3.252 hari dari 2018.
    - **Snapshot Harian (`stockanalysis`, `deribit`):** lewati penulisan chunk baru bila `(series_id, source, obs_date, value)` hari ini identik dengan baris terakhir (*zero duplicate daily chunks*).
    - **Binance Multi-Year Backfill:** tambahkan pagination maju/mundur di `catch_up.rs` (`BinanceRestFetcher`) bila `watermark.total_rows == 0` agar `BTCUSDT` tidak berhenti di 1.000 hari terakhir.
  - *Kriteria Lolos:* Re-sync kedua berturut-turut menulis **0–50 baris delta** (bukan 64.080 baris ulang) dan menjaga StockAnalysis/Deribit tepat 1 baris per tanggal perdagangan.

- **Phase 335 — Footer-Metadata Observation Catalog & 60s TTL Cache (`src/features/storage/observation_reader.rs`, `src/features/serving/observation_routes.rs`):**
  - Optimalkan `ObservationReader::summarize_coverage()` membaca statistik per partisi secara paralel / selektif tanpa memuat seluruh payload historis bila tidak diminta, serta pasangkan `Arc<RwLock<CachedObsCatalog>>` (TTL 60 detik, terinvalidasi otomatis saat `sync` atau `compact` selesai) pada `GET /v1/observations/catalog`.
  - *Kriteria Lolos:* Latency `/v1/observations/catalog` < 50ms saat warm dan selalu < 2 detik saat cold scan.

- **Phase 336 — Unified Startup & Scheduled Compaction (`src/features/ingestion/orchestrator.rs`, `src/features/storage/compactor.rs`, `observation_writer.rs`):**
  - Pastikan siklus `compactor` (startup threshold & interval 6 jam) menggabungkan file chunk baik pada partisi `candles` maupun `observations`, serta `ObservationWriter::write_observations` melewati penulisan file baru bila seluruh baris identik dengan state watermark.
  - *Kriteria Lolos:* Tes integrasi membuktikan jumlah file chunk per partisi tahunan terkendali ($\le 2$) setelah siklus compaction.

---

### Group B: Closing "Balon Air" Coverage Gaps (Phase 337 – 340)

- **Phase 337 — FRED Weekly MMF, Global Central Banks & Financial Stress (`src/features/ingestion/liquidity/fred.rs`, `src/core/series_catalog.rs`):**
  - Tambahkan 6 seri FRED terisolasi (`source="fred_api"`):
    - `macro:US:FRED_WRMFNS_USD` (`WRMFNS`, `$B -> USD`, Mingguan — **Retail MMF mingguan**)
    - `macro:EU:FRED_ECB_ASSETS_EUR` (`ECBASSETSW`, `$M EUR -> EUR`, Mingguan — **Neraca ECB**)
    - `macro:JP:FRED_BOJ_ASSETS_JPY` (`JPNASSETS`, `100M JPY -> JPY`, Bulanan — **Neraca Bank of Japan**)
    - `macro:US:FRED_SOFR_PCT` (`SOFR`, Harian)
    - `macro:US:FRED_STLFSI4_IDX` (`STLFSI4`, Indeks stres finansial mingguan)
    - `macro:US:FRED_WM2NS_USD` (`WM2NS`, `$B -> USD`, M2 mingguan)
  - *Kriteria Lolos:* Seluruh 6 seri tervalidasi unit & skala dari JSON resmi FRED, tersimpan tanpa `api_key` di provenance, dan terdaftar di `SeriesCatalog`.

- **Phase 338 — SSGA Bond & Cash-Park ETF XLSX (`BIL`, `JNK`, `SPTL`) (`src/features/ingestion/flows/ssga_xlsx.rs`, `src/core/subscription.rs`, `src/features/ingestion/liquidity_sync.rs`):**
  - Perluas `SsgaXlsxFetcher` agar mendukung `fetch_bil_history` (Fiat Cash ETF), `fetch_jnk_history` (High Yield Bond ETF), dan `fetch_sptl_history` (Long Treasury ETF) lengkap dengan SHA-256 checksum & 0 mixing.
  - Tambahkan default Yahoo `1d` candle subscriptions untuk `BIL`, `JNK`, `SPTL` di `SubscriptionManager::generate_core_defaults()`.
  - *Kriteria Lolos:* `Stage-1` dapat menghitung *implied net flow* 10 tahun untuk saham (`SPY`), emas (`GLD`), kas T-Bill (`BIL`), obligasi panjang (`SPTL`), dan kredit risiko tinggi (`JNK`).

- **Phase 339 — BTC Spot ETF Flow USD Normalization (`src/features/analytics/liquidity_flows.rs`):**
  - Tambahkan fungsi deterministik konversi `etf:BTC_US:bgeometrics_flow_btc` (BTC) $\times$ harga penutupan `BTCUSDT` menjadi seri turunan **`etf:BTC_US:implied_flow_usd`** (`source="economy_aggregator_stage1"`, mencatat kedua sumber hulu pada `extra_json`).
  - *Kriteria Lolos:* Unit test memverifikasi perhitungan USD flow & penautan lineage dua sumber tanpa mencampur raw data.

- **Phase 340 — Methodological Break & Frequency-Reliability Flagging (`src/features/serving/observation_routes.rs`, `src/core/series_catalog.rs`):**
  - Tambahkan anotasi metadata kualitas pada `/v1/observations/matrix` & `/v1/observations/catalog`: menandai seri berfrekuensi `1M` saat `forward_fill=true` di panel mingguan (mis. `FINRA_MARGIN_DEBT_USD`, `JPNASSETS`) dan menandai perubahan metodologi Bybit OI (`bilateral -> unilateral`).

---

### Group C: Stage-2 Closed-System Econometric Engine & Verification (Phase 341 – 344)

- **Phase 341 — Synchronized Weekly Flow Panel Builder (`src/features/analytics/liquidity_panel.rs`):**
  - Konversikan stok level mingguan (`FRED_WALCL_USD`, `TGA_BALANCE_USD`, `FRED_RRPONTSYD_USD`, `FRED_WRMFNS_USD`, `stablecoin:ALL:mcap_usd`, `FRED_TOTBKCR_USD`) menjadi **aliran bersih mingguan ($\Delta V_t$)** dan gabungkan bersama *Stage-1 Implied Net Flows* (`SPY`, `GLD`, `BIL`, `JNK`, `SPTL`, `BTC_ETF_USD`, `CoinMetrics`) pada grid Jumat EOW yang sinkron.
  - *Kriteria Lolos:* Zero `NaN`/`Inf`, unit konsisten `$ USD`, dan tes paritas akuntansi memastikan nol kebocoran tanggal masa depan (*PIT-safe*).

- **Phase 342 — Pure-Rust VAR(p) & Diebold-Yilmaz Spillover / Connectedness Engine (`src/features/analytics/spillover.rs`):**
  - Implementasikan estimasi OLS `VAR(p)` dan *Generalized Forecast Error Variance Decomposition* (**GFEVD / Diebold-Yilmaz Connectedness Table**) dalam Rust murni tanpa ketergantungan Python/R eksternal:
    - Output: Matriks keterhubungan $N \times N$, **Total Connectedness Index (`TCI`)**, **`TO`**, **`FROM`**, dan **`NET`** spillover per kompartemen balon air.
  - *Kriteria Lolos:* Unit test memverifikasi properti matematis GFEVD (jumlah baris normalisasi = 100%, total `NET` = 0.0, stabil terhadap matriks singular via regularisasi ringan/fail-fast).

- **Phase 343 — Serving Endpoints & Remote CLI Stage-2 (`src/features/serving/observation_routes.rs`, `src/core/cli.rs`, `src/core/cli_remote.rs`):**
  - Perluas `GET /v1/analytics/liquidity-panel` mencakup seluruh 6 kompartemen lengkap, dan tambahkan `GET /v1/analytics/liquidity-spillover?start_date=...&end_date=...&lags=2&horizon=10` (mendukung mode rolling window krisis seperti *2020 Dash for Cash* vs *2022 QT* vs *2025–2026*).
  - Tambahkan subcommand CLI remote `liquidity-panel` & `spillover` di `cli.rs`/`cli_remote.rs` (sebelumnya CLI belum mengekspos dua subcommand observasi/likuiditas ini!).

- **Phase 344 — ADR-21, Full Verification Gauntlet & Production Deployment:**
  - Catat **ADR-21** di `docs/architecture/09_architecture_decisions.md`, jalankan `cargo test --all-targets`, `vsa-boundary-checker src/`, pemeriksaan $\le 400$ baris/file, `osv-scanner`, dan `arc42-lint docs/architecture --strict`.
  - Deploy ke ThinkStation Coolify (`https://market.dvlpid.my.id`) dan verifikasi: `/v1/observations/catalog` merespons < 50ms, re-sync tidak menulis ulang 64k baris, `BIL`/`JNK`/`SPTL`/`WRMFNS`/`ECBASSETSW` teringesti penuh, dan `GET /v1/analytics/liquidity-spillover` mengembalikan matriks spillover 6 kompartemen yang valid.
