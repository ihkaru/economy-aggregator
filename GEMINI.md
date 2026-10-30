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
    └── storage/              # Slice Penyimpanan
        ├── mod.rs
        ├── buffer.rs         # Bounded micro-batch buffer thread-safe
        ├── partition.rs      # Resolver path partisi Hive
        └── writer.rs         # Writer data Parquet ZSTD & deduplikator
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

---

## 3. Verifikasi & Pengujian (Verification Gauntlet)

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

## 4. Lisensi Dependensi
- Semua dependensi pihak ketiga wajib berlisensi bebas komersial: **MIT, Apache-2.0, atau BSD**.
- Dilarang keras menggunakan pustaka copyleft (GPL, AGPL).
