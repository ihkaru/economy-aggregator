# 1. Introduction and Goals

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `4e98035`
> - **Verified Timestamp**: `2026-10-30T20:29:28+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 1.1 Requirements Overview
`economy-aggregator` adalah sistem pengumpul data (data aggregator) keuangan dan ekonomi mandiri berbasis Rust. Sistem ini dirancang untuk:
1. Mengambil data pasar secara langsung (*live streaming*) via WebSocket dan secara berkala (*batch*) via REST API dari berbagai sumber (Crypto, Saham, Komoditas, dan rilis data kalender makro ekonomi).
2. Menyimpan data langsung ke disk lokal dalam format kolumnar Apache Parquet dengan struktur partisi rapi.
3. Melayani data kembali ke aplikasi klien (seperti chart grafik atau bot analisis) melalui REST API berkecepatan tinggi dan WebSocket broadcast.
4. Mendukung inisialisasi instan (*One-Config-Away Bootstrap*) dengan konfigurasi deklaratif dan kemampuan backfill historis harian dari tahun 2020 untuk top 20 crypto, top 25 saham per sektor industri, 10 komoditas utama, dan kalender ekonomi.
5. Menyediakan Command Line Interface ramah agen AI (*Agent-First CLI*) dengan standar output JSON terstruktur, kode keluar deterministik, dan zero-interactivity.
6. Menyediakan mesin komputasi kuantitatif terintegrasi (*In-Engine Analytics*) untuk mengevaluasi metrik risiko dan return portofolio multi-aset (CAGR, Volatilitas, Max Drawdown, Sharpe Ratio, Korelasi Pearson) dengan sinkronisasi kalender multi-bursa, normalisasi mata uang basis (IDR), dan invarian 100% instrumen empiris pasar riil (*Zero Synthetic Data*).

## 1.2 Quality Goals
Sistem ini memprioritaskan performa tinggi dengan konsumsi daya dan memori yang sangat hemat:

| Prioritas | Quality Property | Sasaran Arsitektur |
| :--- | :--- | :--- |
| 1 | #performance | Latensi pemrosesan data live di bawah 100 mikrodetik per tick; query data 1 bulan selesai dalam waktu kurang dari 10 milidetik. |
| 2 | #efficiency | Konsumsi memori RAM stabil di bawah 50 MB saat idle dan di bawah 150 MB saat menerima arus data 50.000 tick/detik. |
| 3 | #reliability | Tidak ada data hilang atau ganda berkat deduplikasi berbasis kunci alami (*natural key*) dan penambalan otomatis (*gap healing*). |
| 4 | #maintainability | Struktur kode modular (Vertical Slice Architecture) berbasis Rust murni tanpa runtime eksternal yang berat. |
| 5 | #agent_readiness | Seluruh fungsionalitas dapat dikonfigurasi, diaudit, dan dijalankan secara otomatis oleh agen AI otonom tanpa prompt interaktif. |
| 6 | #fidelity | 100% menolak data sintetis dalam backtest portofolio dan menerapkan pembersihan anomali statistik non-parametrik (Hampel Filter) tanpa hardcode threshold. |

## 1.3 Stakeholders
- **Analis Data & Kuantitatif**: Memerlukan data historis dan live yang bersih, konsisten, dan cepat dipindai.
- **Pengembang Aplikasi Frontend / UI**: Memerlukan endpoint data yang stabil, berlatensi rendah, dan mendukung streaming maupun query rentang tanggal.
- **Agen AI Otonom**: Memerlukan CLI bertenaga JSON terstruktur (`--json`) dengan petunjuk langkah berikutnya (*next_steps*) untuk eksekusi tugas multi-langkah.
- **Operator Sistem**: Memerlukan aplikasi ringan yang bisa berjalan stabil 24/7 di workstation lokal atau server kecil tanpa kebocoran RAM.
