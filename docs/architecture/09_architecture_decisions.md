# 9. Architecture Decisions

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `329892f`
> - **Verified Timestamp**: `2026-10-30T15:33:05.959236+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 9.1 ADR Index

| ADR ID | Judul Keputusan | Status | Tanggal |
| :--- | :--- | :--- | :--- |
| **ADR-01** | Transisi Penuh ke Bahasa Rust (Zero-Garbage Collector) | Diterima | 2026-10-30 |
| **ADR-02** | Penggunaan Format File Apache Parquet Terkompresi ZSTD | Diterima | 2026-10-30 |
| **ADR-03** | Penerapan Model Ingesti Hybrid (Live WebSocket + REST Fallback) | Diterima | 2026-10-30 |

---

### ADR-01: Transisi Penuh ke Bahasa Rust (Zero-Garbage Collector)
* **Status:** Diterima
* **Konteks:**  
  Awalnya sistem dipertimbangkan memakai Python. Namun untuk data aggregator berkecepatan tinggi yang menerima puluhan ribu tick per detik via WebSocket, runtime Python akan boros RAM (setiap angka float memakan 28 byte vs 8 byte di Rust), terhambat oleh GIL (Global Interpreter Lock), dan memicu jeda pembersihan memori (*Garbage Collector pauses*).
* **Keputusan:**  
  Membangun seluruh inti agregator menggunakan **Rust** dengan runtime asinkron `tokio` dan framework web `axum`.
* **Konsekuensi Positif:**  
  Konsumsi RAM sangat stabil di bawah 50 MB, latensi sub-milidetik yang konsisten, dan file executable tunggal yang mudah dijalankan di mana saja.
* **Risiko yang Muncul:**  
  `RISK-01` (Waktu kompilasi awal Rust lebih lama dibanding bahasa skrip).

---

### ADR-02: Penggunaan Format File Apache Parquet Terkompresi ZSTD
* **Status:** Diterima
* **Konteks:**  
  Menyimpan jutaan data transaksi di database relasional tradisional (seperti SQLite/Postgres) memakan banyak disk dan lambat saat memindai rentang tanggal panjang.
* **Keputusan:**  
  Menggunakan format kolumnar terbuka **Apache Parquet** dengan kompresi `ZSTD` level 3 dan partisi gaya Hive.
* **Konsekuensi Positif:**  
  Ukuran file 70% lebih kecil dibanding CSV atau database row-based, serta sangat cepat dibaca oleh mesin analitik modern seperti Polars dan DuckDB.
* **Risiko yang Muncul:**  
  `RISK-02` (Potensi penumpukan file kecil jika interval flush terlalu sering tanpa proses penggabungan).

---

### ADR-03: Penerapan Model Ingesti Hybrid (Live WebSocket + REST Fallback)
* **Status:** Diterima
* **Konteks:**  
  Koneksi WebSocket real-time bisa putus sewaktu-waktu akibat gangguan jaringan, menyebabkan data bolong (*stream gaps*).
* **Keputusan:**  
  Menerapkan mekanisme ganda: data diterima via WebSocket secara live, namun jika terdeteksi nomor urut melompat atau koneksi putus, sistem otomatis memanggil REST API untuk menambal data yang terlewat.
* **Konsekuensi Positif:**  
  Data selalu lengkap dan akurat tanpa perlu intervensi manual dari pengguna.
* **Risiko yang Muncul:**  
  `RISK-03` (Batas kuota panggilan REST API / rate limit dari penyedia data eksternal).
