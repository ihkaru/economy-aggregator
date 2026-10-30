# 6. Runtime View

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `cc06233`
> - **Verified Timestamp**: `2026-10-30T16:05:09.720863+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 6.1 Skenario 1: Penerimaan Data Live & Penulisan ke Disk

Menjelaskan alur saat pesan tick dari bursa masuk hingga tersimpan di file Parquet:

```mermaid
sequenceDiagram
    autonumber
    participant Bursa as Bursa Luar (WebSocket)
    participant Ingest as Ingestion Live Stream
    participant Gate as Sanity Gate
    participant Buf as Micro-Batch Buffer
    participant Writer as Parquet Lake Writer
    participant Disk as Harddisk Lokal

    Bursa->>Ingest: Kirim Pesan Tick (JSON)
    Ingest->>Gate: Validasi Integritas Data
    alt Data Tidak Valid (misal: harga <= 0)
        Gate-->>Ingest: Tolak & Log Peringatan
    else Data Valid
        Gate->>Buf: Masukkan ke Antrean RAM
    end

    Note over Buf,Writer: Terjadi ketika waktu >= 5 detik atau baris >= 10.000
    Buf->>Writer: Kirim Kumpulan Batch Data
    Writer->>Disk: Tulis File Parquet (ZSTD Level 3)
    Disk-->>Writer: Berhasil Simpan
```

## 6.2 Skenario 2: Deteksi Gap & Penambalan Otomatis (*Gap Healing*)

Menjelaskan alur saat koneksi internet sempat putus dan data tertinggal:

```mermaid
sequenceDiagram
    autonumber
    participant Ingest as Ingestion Stream
    participant Healer as Gap Healer
    participant REST as REST Batch Fetcher
    participant Bursa as Bursa Luar (REST API)
    participant Buf as Storage Buffer

    Ingest->>Healer: Pesan masuk dengan sequence_id = 150 (sebelumnya = 145)
    Note over Healer: Terdeteksi 4 pesan terlewat!
    Healer->>REST: Minta Data Rentang Waktu Hilang
    REST->>Bursa: GET /historical_candles?from=145&to=150
    Bursa-->>REST: Mengembalikan Data Lengkap
    REST->>Buf: Masukkan Data Pengganti (is_settled = true)
```

## 6.3 Skenario 3: Konsumsi Data Historis via REST API

Aplikasi grafik atau skrip analis meminta data historis:

1. Klien mengirim permintaan `GET /v1/market/candles?symbol=BTC/USDT&timeframe=1m&from=...&to=...`.
2. Router `Axum` mengarahkan permintaan ke modul pembaca Parquet.
3. Pembaca langsung menuju folder partisi tanggal yang sesuai (*folder pruning*).
4. Data dikirimkan kembali ke klien dalam format JSON terkompresi atau stream Arrow.

## 6.4 Skenario 4: Eksekusi Backfill Historis 2020 via Agent-First CLI

Alur saat Agen AI atau operator mengeksekusi penarikan data historis sejak tahun 2020 secara headless:

```mermaid
sequenceDiagram
    autonumber
    participant Agent as Agen AI / Operator
    participant CLI as Agent CLI Engine
    participant Config as Config & Secret Loader
    participant Fetcher as Historical REST Paginator
    participant Lake as Parquet Lake Writer

    Agent->>CLI: economy-aggregator backfill start --asset crypto --start-year 2020 --json
    CLI->>Config: Muat economy-aggregator.toml & Injeksi Token Infisical
    Config-->>CLI: Konfigurasi Terverifikasi
    CLI->>Fetcher: Inisiasi Paginasi dari 2020-01-01T00:00:00Z
    loop Paginasi Bulanan
        Fetcher->>Lake: Tulis Chunks Parquet Harian (1D bars)
    end
    Lake-->>CLI: Total Baris Tersimpan & Statistik Partisi
    CLI-->>Agent: Output Envelope JSON: {"success": true, "data": {...}, "next_steps": [...]}
```
