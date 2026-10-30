# GEMINI.md - Architecture & Project Guidelines: Economy Aggregator (Full Rust)

## 1. Project Overview & Vision
`economy-aggregator` adalah sistem pengumpul data (data aggregator) keuangan dan ekonomi mandiri berperforma tinggi berbasis **100% Rust**.
Karakteristik utama:
- **Zero-Garbage Collector & Low RAM:** Berjalan sangat hemat memori (< 50 MB idle, < 150 MB under load 50k ticks/sec) tanpa jeda pembersihan memori.
- **Penyimpanan Danau Kolumnar (Columnar Lakehouse):** Apache Parquet dengan partisi gaya Hive (`asset_class / symbol / timeframe / year / month`) dan kompresi ZSTD.
- **Model Ingesti Hybrid:** Mendukung penerimaan live stream WebSocket dan batch backfill REST secara simultan dengan pencegah data duplikat dan penambal otomatis (*gap healing*).
- **Arsitektur Berstandar arc42:** Seluruh arsitektur didokumentasikan di `docs/architecture/` (01–12) dan diverifikasi otomatis via `arc42-lint`.

---

## 2. Struktur Proyek: Vertical Slice Architecture (VSA)

```
src/
├── main.rs                   # Entrypoint server & CLI
├── lib.rs                    # Library root untuk integration tests
├── core/                     # Fondasi domain model & konfigurasi
│   ├── mod.rs
│   ├── config.rs             # Parameter batas RAM & path data
│   └── types.rs              # AggregatorCandle, MarketTick, MacroEvent
└── features/
    ├── ingestion/            # Slice Ingesti
    │   ├── mod.rs
    │   └── sanity_gate.rs    # Filter anti-data kotor & detektor gap
    ├── storage/              # Slice Penyimpanan
    │   ├── mod.rs
    │   ├── buffer.rs         # Bounded micro-batch buffer thread-safe
    │   ├── partition.rs      # Resolver path partisi Hive
    │   └── writer.rs         # Writer data Parquet ZSTD & deduplikator
    └── analytics/            # Slice Analitika Portofolio & Risiko
        ├── mod.rs
        ├── bitemporal.rs     # Point-in-Time (PIT) Macro Vintage Engine (Zero Lookahead)
        ├── calendar_aligner.rs # Forward-fill sinkronisasi kalender multi-bursa
        ├── config.rs         # PortfolioSpec & AssetSpec deklaratif
        ├── corporate_actions.rs # Split & Cash Dividend Backward Adjuster (Total Return)
        ├── futures_roll.rs   # Panama / Additive Futures Roll & 24/5 FX Session Normalizer
        ├── fx_normalizer.rs  # Normalisasi valuasi multi-currency ke IDR
        ├── outlier.rs        # Hampel Filter (Rolling MAD) non-parametrik
        ├── portfolio.rs      # Engine CAGR, Volatilitas, Max DD, Sharpe, Korelasi
        └── runner.rs         # CLI runner `economy-aggregator analyze`
```

### Batasan Teknis & Aturan Performa:
1. **Single Responsibility Principle (SRP):**
   - Panjang fungsi maksimal: 50 baris.
   - Panjang struct/impl maksimal: 180 baris.
   - Panjang file source maksimal: 400 baris.
2. **Kapasitas Antrean Terukur (Bounded Buffer):**
   - Kapasitas maksimal antrean RAM: 50.000 record untuk mencegah kebocoran memori.
3. **Pemisahan Thread CPU:**
   - I/O jaringan ditangani Tokio async runtime.
   - Penulisan disk Parquet/file wajib didelegasikan ke `tokio::task::spawn_blocking`.
4. **Zero-Copy Hot Path:**
   - Jalur pemrosesan WebSocket tick dilarang mengalokasikan string baru secara boros.
5. **Invarian Keras Zero Synthetic Data (Zero-Synthetic Data Mandate):**
   - **Dilarang Keras Data Sintetis:** Simulasi, riset, dan backtest portofolio mutlak dilarang memalsukan data atau merekayasa runtun waktu menggunakan kurva bunga majemuk konstan (seperti formula yield eksponensial datar $P_0 (1+y)^t$). Kurva sintetis datar menghasilkan nol volatilitas dan nol drawdown palsu yang merusak validitas Sharpe ratio dan metrik risiko institusional.
   - **100% Instrumen Pasar Empiris:** Seluruh kelas aset obligasi dan pasar uang dalam pengujian portofolio wajib menggunakan instrumen empiris pasar riil (seperti ETF Obligasi Total Return: TLT, IEF, LQD, BND) atau runtun yield historis riil dengan sensitivitas durasi-konveksitas nyata.
6. **Pembersihan Anomali Non-Parametrik Berbasis Statistik (No Hardcoded Price Bounds):**
   - **Anti-Hardcode:** Dilarang keras menaruh filter harga statis atau nama ticker spesifik (misal `price < 5000.0` atau `symbol == "USD_IDR"`) di jalur analitika dan ingesti.
   - **Hampel Filter (Rolling MAD):** Pembersihan lonjakan harga anomali / fat-finger quotes wajib menggunakan metode statistik non-parametrik berbasis Median Absolute Deviation (MAD):
     $$|x_t - M_t| > n \times 1.4826 \times \text{MAD}_t$$
     di mana $M_t = \text{median}(x_{t-k..t+k})$ dan $\text{MAD}_t = \text{median}(|x_i - M_t|)$ dengan jendela rolling simetris ($k=7$, batas $3.5\sigma$).
7. **Konfigurasi Portofolio Deklaratif:**
   - Seluruh alokasi aset, bobot, dan instrumen wajib dirumuskan melalui struktur spesifikasi deklaratif (`PortfolioSpec` & `AssetSpec`), bukan melalui injeksi variabel prosedural di dalam loop kalkulasi.
8. **Penanganan Corporate Actions Saham (Stock Splits & Cash Dividends):**
   - Setiap peristiwa pemecahan saham (*stock split*) dan dividen tunai disesuaikan ke belakang (*backward adjustment*) secara matematis untuk menjamin kontinuitas runtun harga historis tanpa *false crash* atau distorsi pengembalian investasi.
9. **Model Bitemporal Point-in-Time (PIT) untuk Data Makroekonomi:**
   - Seluruh rilis statistik ekonomi makro (GDP, CPI, NFP) disimpan dengan mencatat tanggal rilis (*release timestamp*) dan versi vintage. Kueri simulasi hanya boleh membaca data yang telah diketahui publik pada titik waktu simulasi (*as-of date*) guna mencegah *Look-Ahead Bias*.
10. **Penataan Runtun Komoditas Berkelanjutan & Sesi Valas 24/5:**
   - Mengeliminasi lonjakan harga artifisial akibat kedaluwarsa kontrak berjangka (*futures roll*) menggunakan metode rasio Panama atau selisih aditif, serta menyelaraskan sesi pembukaan Valas Minggu malam ke sesi Senin pagi.
11. **Prinsip Preservasi Data Mentah (Bronze Layer As-Is Invariant & Downstream Decoupling):**
   - **Data Mentah Disimpan Apa Adanya:** Lapisan ingesti dan penyimpanan Parquet dilarang keras melakukan manipulasi *lossy*, pemotongan split, atau penghapusan harga anomali/negatif pada saat penulisan ke danau data (*Bronze Tier*). Danau data wajib merekam kebenaran mentah bursa (*ground truth*).
   - **Pemisahan Tanggung Jawab Downstream:** Semua transformasi, pembersihan anomali statistik (*Hampel filter*), penyesuaian pemecahan saham (*Corporate Actions*), perataan kontrak berjangka (*Futures Roll*), dan pencegahan *Look-Ahead Bias* (*Point-in-Time*) adalah tanggung jawab konsumen data hilir (*downstream processing / Silver-Gold tier*) menggunakan modul analitika bawaan, BUKAN filter destruktif pada saat ingesti.

---

## 3. Implementation Roadmap (260 Granular Phases)
Pengembangan jangka panjang diorganisasi ke dalam 9 Milestone yang mencakup 260 fase atomik terukur:
- Master Index: [`docs/roadmap/ROADMAP.md`](docs/roadmap/ROADMAP.md)
  - **M1 (Phase 001 - 025):** Core Lakehouse & Storage Engine ([docs/roadmap/milestone_1_core_storage.md](docs/roadmap/milestone_1_core_storage.md))
  - **M2 (Phase 026 - 055):** Cryptocurrency High-Frequency Engine ([docs/roadmap/milestone_2_crypto.md](docs/roadmap/milestone_2_crypto.md))
  - **M3 (Phase 056 - 085):** Forex & Commodities Tick Lake ([docs/roadmap/milestone_3_forex_commodities.md](docs/roadmap/milestone_3_forex_commodities.md))
  - **M4 (Phase 086 - 115):** Global Equities & Corporate Actions ([docs/roadmap/milestone_4_equities.md](docs/roadmap/milestone_4_equities.md))
  - **M5 (Phase 116 - 145):** Macroeconomic Calendar & Vintages ([docs/roadmap/milestone_5_macro_calendar.md](docs/roadmap/milestone_5_macro_calendar.md))
  - **M6 (Phase 146 - 175):** Gap Healing, Dedup & Lake Compactor ([docs/roadmap/milestone_6_resilience_compactor.md](docs/roadmap/milestone_6_resilience_compactor.md))
  - **M7 (Phase 176 - 200):** Serving Layer (REST, WS & Arrow Flight) ([docs/roadmap/milestone_7_serving_streaming.md](docs/roadmap/milestone_7_serving_streaming.md))
  - **M8 (Phase 201 - 225):** Observability, Replay & Production Ops ([docs/roadmap/milestone_8_operations_replay.md](docs/roadmap/milestone_8_operations_replay.md))
  - **M9 (Phase 226 - 260):** One-Config Setup, Deep Backfill 2020 & Agent CLI ([docs/roadmap/milestone_9_one_config_deep_backfill_agent_cli.md](docs/roadmap/milestone_9_one_config_deep_backfill_agent_cli.md))

---

## 4. Verifikasi & Pengujian (Verification Gauntlet)

Jalankan perintah berikut sebelum melakukan commit:

```bash
# 1. Kompilasi & test suite paralel Rust
cargo test

# 2. Linter kode Rust & deteksi anti-pattern
cargo clippy -- -D warnings

# 3. Verifikasi dokumentasi arsitektur arc42 & integritas git
arc42-lint docs/architecture

# 4. Verifikasi isolasi Vertical Slice Architecture (VSA)
vsa-boundary-checker src/
```

---

## 5. Lisensi Dependensi
- Semua dependensi pihak ketiga wajib berlisensi bebas komersial: **MIT, Apache-2.0, atau BSD**.
- Dilarang keras menggunakan pustaka copyleft (GPL, AGPL).
