# 1. Introduction and Goals

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `3f57cd9`
> - **Verified Timestamp**: `2026-10-30T14:52:27+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 1.1 Requirements Overview
`economy-aggregator` adalah sistem pengumpul data (data aggregator) keuangan dan ekonomi mandiri berbasis Rust. Sistem ini dirancang untuk:
1. Mengambil data pasar secara langsung (*live streaming*) via WebSocket dan secara berkala (*batch*) via REST API dari berbagai sumber (Crypto, Saham, Forex, dan rilis data makro ekonomi).
2. Menyimpan data langsung ke disk lokal dalam format kolumnar Apache Parquet dengan struktur partisi rapi.
3. Melayani data kembali ke aplikasi klien (seperti chart grafik atau bot analisis) melalui REST API berkecepatan tinggi dan WebSocket broadcast.

## 1.2 Quality Goals
Sistem ini memprioritaskan performa tinggi dengan konsumsi daya dan memori yang sangat hemat:

| Prioritas | Quality Property | Sasaran Arsitektur |
| :--- | :--- | :--- |
| 1 | #performance | Latensi pemrosesan data live di bawah 100 mikrodetik per tick; query data 1 bulan selesai dalam waktu kurang dari 10 milidetik. |
| 2 | #efficiency | Konsumsi memori RAM stabil di bawah 50 MB saat idle dan di bawah 150 MB saat menerima arus data 50.000 tick/detik. |
| 3 | #reliability | Tidak ada data hilang atau ganda berkat deduplikasi berbasis kunci alami (*natural key*) dan penambalan otomatis (*gap healing*). |
| 4 | #maintainability | Struktur kode modular (Vertical Slice Architecture) berbasis Rust murni tanpa runtime eksternal yang berat. |

## 1.3 Stakeholders
- **Analis Data & Kuantitatif**: Memerlukan data historis dan live yang bersih, konsisten, dan cepat dipindai.
- **Pengembang Aplikasi Frontend / UI**: Memerlukan endpoint data yang stabil, berlatensi rendah, dan mendukung streaming maupun query rentang tanggal.
- **Operator Sistem**: Memerlukan aplikasi ringan yang bisa berjalan stabil 24/7 di workstation lokal atau server kecil tanpa kebocoran RAM.
