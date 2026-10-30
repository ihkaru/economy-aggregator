# 5. Building Block View

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `4e98035`
> - **Verified Timestamp**: `2026-10-30T20:29:28+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 5.1 Level 1: System Decomposition

Sistem dibagi menjadi modul-modul independen dengan tanggung jawab tunggal (Vertical Slice Architecture):

```mermaid
graph TD
    subgraph IngestionSlice["Fitur Ingesti (src/features/ingestion)"]
        WS[WebSocket Live Streamer]
        REST[REST Batch Fetcher]
        SAN[Sanity Gate & Gap Healer]
    end

    subgraph StorageSlice["Fitur Penyimpanan (src/features/storage)"]
        BUF[Micro-Batch Buffer]
        WRITER[Parquet Lake Writer]
        RESOLVER[Partition Resolver]
    end

    subgraph ServingSlice["Fitur Pelayanan (src/features/serving)"]
        API[Axum REST Server]
        BC[WebSocket Broadcaster]
    end

    subgraph CliSlice["Fitur Antarmuka Agen (src/core/cli.rs)"]
        CLI[Agent-First CLI Engine]
        CFG[Config & Secret Resolver]
    end

    subgraph AnalyticsSlice["Fitur Analitika Kuantitatif (src/features/analytics)"]
        ALIGN[Trading Calendar Aligner]
        FX[FX Base Currency Normalizer]
        HAMPEL[Hampel Outlier Filter MAD]
        SPEC[Declarative PortfolioSpec]
        BOND[Synthetic Bond Accumulator]
        PORT[Portfolio Risk & Return Engine]
    end

    WS -->|Validasi| SAN
    REST -->|Validasi| SAN
    SAN -->|Kirim Record| BUF
    BUF -->|Flush Berkala| WRITER
    WRITER -->|Tentukan Folder| RESOLVER
    WRITER -->|Tulis Parquet| DISK[(Parquet Lake)]
    DISK -->|Baca Cepat| API
    DISK -->|Scan Data Panel| PORT
    SAN -->|Pembaruan Live| BC
    CLI -->|Baca/Tulis Konfigurasi| CFG
    CLI -->|Pemicu Backfill| REST
    CLI -->|Audit Kesehatan| DISK
    CLI -->|Eksekusi Analitik| PORT
    SPEC -->|Spesifikasi Bobot & Aset| PORT
    DISK -->|Runtun Waktu Mentah| HAMPEL
    HAMPEL -->|Data Bersih Non-Outlier| FX
    FX -->|Normalisasi Kurs IDR| ALIGN
    ALIGN -->|Panel Sinkron 100% Empiris| PORT
    BOND -.->|TRI Teoritis| PORT
```

### 5.1.1 Level 1 Component Catalog

| Komponen | Tanggung Jawab Utama | Antarmuka Terkait |
| :--- | :--- | :--- |
| **Ingestion Engine** | Menangani koneksi jaringan ke bursa luar, baik secara live stream maupun unduhan berkala. | `IF-01`, `IF-02` |
| **Sanity Gate & Gap Healer** | Memfilter data rusak (harga minus atau lonjakan anomali) dan mendeteksi data bolong untuk ditambal ulang. | `IF-01`, `IF-02` |
| **Micro-Batch Buffer** | Menampung data masuk di RAM secara aman sebelum dieksekusi ke harddisk. | `IF-01`, `IF-03` |
| **Parquet Lake Writer & Compactor** | Menulis data terkompresi ke partisi Parquet Hive dan memadatkan micro-chunks menjadi single file bulanan. | `IF-03` |
| **Serving Engine** | Menyediakan jalur bagi aplikasi luar untuk mengambil data via REST API atau WebSocket real-time. | `IF-04`, `IF-05` |
| **Quantitative Analytics Engine** | Menyediakan sinkronisasi kalender multi-bursa, normalisasi FX, sintesis kupon obligasi, dan kalkulasi risiko portofolio (CAGR, Volatilitas, Max Drawdown, Sharpe, Korelasi). | `IF-07` |
| **Agent-First CLI & Config Engine** | Menyediakan eksekusi perintah headless, inisialisasi konfigurasi deklaratif, dan orkestrasi backfill 2020 via JSON envelope. | `IF-06` |

---

## 5.2 Level 2: Detail Struktur Kode (Rust Layout)

```
src/
├── main.rs                   # Titik masuk program utama & inisialisasi server
├── core/                     # Model data dasar dan konfigurasi
│   ├── mod.rs
│   ├── types.rs              # AggregatorCandle, EconomicEventRecord, Enums
│   ├── config.rs             # LakehouseConfig & batas budget memori
│   └── cli.rs                # Agent-First CLI dispatcher & argument parser
├── features/                 # Vertical Slice Architecture
│   ├── ingestion/            # SanityGate, Binance WS/REST, Yahoo Ingestion, Economic Calendar
│   ├── storage/              # Parquet Writer, Compactor, Hive Resolver, Symbology
│   ├── analytics/            # Calendar Aligner, FX Normalizer, HampelFilter, PortfolioSpec, Portfolio Engine
│   └── serving/              # Axum REST endpoints (/health, /candles, /stats)
economy-aggregator.toml       # Template konfigurasi deklaratif 'one-config-away'
```
