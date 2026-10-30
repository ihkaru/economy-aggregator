# 8. Cross-Cutting Concepts

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `936d34c`
> - **Verified Timestamp**: `2026-10-30T20:19:40+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 8.1 Pola Penanganan Error (Robust Error Handling)
- Semua fungsi yang dapat gagal mengembalikan tipe standar Rust `Result<T, AppError>`.
- Dilarang keras menggunakan `.unwrap()` atau `.expect()` pada alur data live untuk mencegah aplikasi mati mendadak (*panic*).
- Jika ada satu baris pesan bursa yang gagal diparsing, baris tersebut dicatat sebagai log peringatan, sementara aliran data tetap berjalan normal tanpa gangguan.

## 8.2 Logging & Observabilitas Berperforma Tinggi
- Menggunakan pustaka resmi Rust `tracing` dan `tracing-subscriber`.
- Format log terstruktur (*structured logging*) yang efisien tanpa alokasi string boros.
- Level log dapat diatur dinamis (`RUST_LOG=info` atau `RUST_LOG=debug`).

## 8.3 Keamanan Data & Pembersihan Input (Input Sanitization & Data Hygiene)
- **Pembersihan Simbol Ticker:** Karakter berbahaya pada simbol (seperti `..`, `/`, `\`) dibersihkan sebelum dijadikan nama folder partisi agar tidak terjadi *directory traversal*.
- **Sanitasi Ingesti Non-Destruktif (Bronze Tier):** Gerbang ingesti hanya menolak payload bursa yang rusak secara matematis (*NaN* atau *Infinity*) dan payload bursa malformed. Gerbang ingesti dilarang membuang transaksi pasar riil secara sepihak (termasuk lonjakan split 4:1 atau harga negatif komoditas). Seluruh rekaman mentah disimpan utuh apa adanya ke Parquet lakehouse.
- **Pembersihan Anomali Statistik Downstream (Hampel Filter & Rolling MAD):** Pada tahap konsumsi analitika downstream, pembersihan outlier runtun waktu dilakukan secara non-parametrik menggunakan formula:
  $$|x_t - M_t| > n \times 1.4826 \times \text{MAD}_t$$
  dengan sliding window simetris $2k + 1 = 15$ titik ($k=7$) dan ambang deviasi $n = 3.5\sigma$. Pendekatan ini menyaring anomali fat-finger tanpa merusak rekaman asli di danau data.
- **Pemisahan Tanggung Jawab Downstream (Downstream Decoupling):** Transformasi aksi korporasi (`CorporateActionAdjuster`), eliminasi look-ahead bias (`PointInTimeEngine`), dan perataan futures (`FuturesRollEngine`) dieksekusi secara non-destruktif di lapisan downstream sesuai kebutuhan pemodelan spesifik.
- **Kebijakan Zero-Synthetic Data:** Jalur analitika portofolio menolak kurva compounding sintetis konstan. Semua estimasi risiko wajib menggunakan data empiris historis riil.

## 8.4 Model Konkurensi & Manajemen Memori
- Menggunakan antrean terkendali `tokio::sync::mpsc::channel(50_000)` dengan kapasitas tetap (*bounded channel*).
- Jika disk mengalami perlambatan sesaat, antrean akan menahan laju penerimaan (*backpressure*) secara elegan daripada membiarkan RAM meluap kehabisan memori.
- Operasi tulis file Parquet yang bersifat sinkron dijalankan menggunakan `tokio::task::spawn_blocking` agar tidak memperlambat thread asinkron jaringan.

## 8.5 Standar CLI Ramah AI-Agent (Agent-First Envelope & Protocol)
- **Envelope JSON Standar:** Seluruh perintah mode `--json` mengembalikan payload baku:
  ```json
  {
    "success": true,
    "error": null,
    "data": { ... },
    "next_steps": ["economy-aggregator backfill status", "economy-aggregator lake stats"]
  }
  ```
- **Zero Interactivity:** Perintah mutasi tidak pernah memblokir `stdin`. Penggunaan otomatis wajib mendukung opsi `--yes`, `--force`, dan `--non-interactive`.
- **Kode Keluar Deterministik:** `0` (Sukses), `1` (Argumen/Konfigurasi Salah), `2` (Rate Limit/Network Error), `3` (Storage I/O Error), `4` (Integritas Data Gagal).

## 8.6 Manajemen Kunci Rahasia & Konfigurasi Bertingkat (Infisical & ENV Hierarchy)
- Mengadopsi hierarki resolusi 3-tier:
  1. *Command-line Flags* (Prioritas tertinggi, misal `--start-year 2020`).
  2. *Injeksi Rahasia Mesin* via Infisical Universal Auth (`infisical run -- dev`) dan Environment Variables (`AGGREGATOR_*`).
  3. *File Konfigurasi Deklaratif* (`economy-aggregator.toml`) untuk kemudahan setup "one config away".
- Token rahasia atau API key vendor tidak pernah disimpan ke git commit atau dicetak mentah di log terminal (wajib dimaskir: `sk_***`).
