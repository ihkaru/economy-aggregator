# 8. Cross-Cutting Concepts

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `fae2423`
> - **Verified Timestamp**: `2026-10-30T14:52:27+07:00`
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

## 8.3 Keamanan Data & Pembersihan Input (Input Sanitization)
- **Pembersihan Simbol Ticker:** Karakter berbahaya pada simbol (seperti `..`, `/`, `\`) dibersihkan sebelum dijadikan nama folder partisi agar tidak terjadi *directory traversal*.
- **Filter Nilai Anomali:** Harga bernilai nol, minus, atau tidak terhingga (*NaN / Infinity*) langsung dibuang di pintu gerbang *Sanity Gate*.

## 8.4 Model Konkurensi & Manajemen Memori
- Menggunakan antrean terkendali `tokio::sync::mpsc::channel(50_000)` dengan kapasitas tetap (*bounded channel*).
- Jika disk mengalami perlambatan sesaat, antrean akan menahan laju penerimaan (*backpressure*) secara elegan daripada membiarkan RAM meluap kehabisan memori.
- Operasi tulis file Parquet yang bersifat sinkron dijalankan menggunakan `tokio::task::spawn_blocking` agar tidak memperlambat thread asinkron jaringan.
