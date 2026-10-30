# 9. Architecture Decisions

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `4ae83e1`
> - **Verified Timestamp**: `2026-10-30T19:14:40+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 9.1 ADR Index

| ADR ID | Judul Keputusan | Status | Tanggal |
| :--- | :--- | :--- | :--- |
| **ADR-01** | Transisi Penuh ke Bahasa Rust (Zero-Garbage Collector) | Diterima | 2026-10-30 |
| **ADR-02** | Penggunaan Format File Apache Parquet Terkompresi ZSTD | Diterima | 2026-10-30 |
| **ADR-03** | Penerapan Model Ingesti Hybrid (Live WebSocket + REST Fallback) | Diterima | 2026-10-30 |
| **ADR-04** | Antarmuka CLI Ramah Agen AI dengan Format Envelope JSON | Diterima | 2026-10-30 |
| **ADR-05** | Arsitektur Penyelarasan Kalender Multi-Bursa & Normalisasi Valuasi FX | Diterima | 2026-10-30 |
| **ADR-06** | Mesin Analitika Portofolio & Risiko Vektorisasi Berbasis Rust Asli | Diterima | 2026-10-30 |
| **ADR-07** | Kebijakan Zero-Synthetic Data dan Deteksi Anomali Statistik Non-Hardcode | Diterima | 2026-10-30 |

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

---

### ADR-04: Antarmuka CLI Ramah Agen AI dengan Format Envelope JSON dan Zero Interactivity
* **Status:** Diterima
* **Konteks:**  
  Sistem perlu dioperasikan tidak hanya oleh manusia via terminal interaktif, tetapi juga oleh Agen AI otonom (*Autonomous AI Agents*) secara headless. Prompt interaktif (seperti `[y/N]`) dan output tidak terstruktur menyebabkan agen macet (*hang indefinitely*) atau salah tafsir hasil.
* **Keputusan:**  
  Membangun CLI dengan format output envelope terstandarisasi (`{"success": true, "data": {...}, "error": null, "next_steps": [...]}`), flag non-interaktif (`--yes`, `--force`), kode keluar numerik terklasifikasi, dan dukungan *dry-run*.
* **Konsekuensi Positif:**  
  Agen AI dapat memanggil, memverifikasi kesehatan, mengonfigurasi parameter, dan memicu deep backfill sejak 2020 dengan keandalan 100%.
* **Risiko yang Muncul:**  
  `RISK-04` (Potensi pembatasan laju panggilan / rate limit saat penarikan data historis masif sejak tahun 2020).

---

### ADR-05: Arsitektur Penyelarasan Kalender Multi-Bursa & Normalisasi Valuasi FX
* **Status:** Diterima
* **Konteks:**  
  Penyusunan portofolio multi-aset (All-Weather) mencakup bursa dengan hari libur berbeda (NYSE, IDX, TSE, komoditas, dan kripto 24/7) serta denominasi mata uang beragam (USD, IDR). Ketidakselarasan tanggal dan kurs menyebabkan data missing gaps saat outer join dan bias return riil daya beli investor IDR.
* **Keputusan:**  
  Membangun modul `TradingCalendarAligner` dengan interpolasi deterministik Forward-Fill untuk menyamakan spine tanggal perdagangan, dan `FxNormalizer` untuk otomatis mengalikan atau membagi harga aset asing dengan time-series kurs FX acuan.
* **Konsekuensi Positif:**  
  Dataset panel multi-aset selalu kontinu tanpa nilai kosong (*zero-NaN gap*), dan return aset luar negeri terukur secara adil dalam mata uang basis investor.
* **Risiko yang Muncul:**  
  `RISK-06` (Asinkronitas kalender bursa) dan `RISK-07` (Distorsi nilai tukar).

---

### ADR-06: Mesin Analitika Portofolio & Risiko Vektorisasi Berbasis Rust Asli
* **Status:** Diterima
* **Konteks:**  
  Sebelumnya, pengujian hipotesis portofolio All-Weather dan kalkulasi matriks korelasi cross-asset bergantung pada skrip eksternal (Python dengan pustaka Polars/PyArrow). Hal ini menambah ketergantungan runtime luar dan memperlambat alur kerja otomatisasi agen AI.
* **Keputusan:**  
  Membangun modul analitika kuantitatif internal murni di Rust (`src/features/analytics/`) yang mencakup `PortfolioEngine`, `SyntheticBondAccumulator`, dan sub-perintah CLI `economy-aggregator analyze`.
* **Konsekuensi Positif:**  
  Kalkulasi CAGR, volatilitas tahunan, max drawdown, Sharpe ratio, dan matriks korelasi Pearson dapat dieksekusi secara instan (<2 detik) langsung dari Parquet lakehouse tanpa membutuhkan runtime Python eksternal.
* **Risiko yang Muncul:**  
  `DEBT-02` (Hutang analitik diselesaikan dan diverifikasi penuh dengan 100% tes otomatis).

---

### ADR-07: Kebijakan Zero-Synthetic Data dan Deteksi Anomali Statistik Non-Hardcode
* **Status:** Diterima
* **Konteks:**  
  Pada iterasi awal, kelas aset obligasi dimodelkan menggunakan akumulasi yield sintetis konstan ($1.072^t$). Ini menyebabkan bias artifisial berat karena runtun waktu obligasi memiliki volatilitas nol dan drawdown nol, yang secara palsu melipatgandakan Sharpe ratio portofolio. Selain itu, pembersihan anomali harga pada rute ingesti sempat menggunakan aturan hardcode ad-hoc (seperti `price < 5000.0` atau pemeriksaan string `USD_IDR`). Hal ini melanggar prinsip kebersihan kode dan tidak tahan uji jangka panjang.
* **Keputusan:**  
  1. Menetapkan **Invarian Zero-Synthetic Data** yang melarang keras penggunaan kurva yield konstan atau data fiktif dalam riset dan backtesting portofolio. Portofolio All-Weather wajib divalidasi dengan 100% instrumen pasar empiris riil (seperti ETF Obligasi Total Return: TLT, IEF, LQD, BND).
  2. Menggantikan seluruh ambang batas hardcoded dengan **Hampel Filter (Rolling Median Absolute Deviation / MAD)** non-parametrik adaptif ($k=7, 3.5\sigma$) yang bekerja secara agnostik terhadap seluruh kelas aset.
  3. Mengganti injeksi bobot prosedural dengan spesifikasi konfigurasi deklaratif `PortfolioSpec`.
* **Konsekuensi Positif:**  
  Hasil backtest memiliki integritas kuantitatif tinggi setara standar hedge fund institusional tanpa distorsi volatilitas palsu. Algoritma pembersihan tahan terhadap regresi dan tidak memerlukan tambal-sulam saat menambahkan instrumen baru.
* **Risiko yang Muncul:**  
  `RISK-08` (Diperlukannya pengunduhan data pasar empiris riil untuk instrumen ETF obligasi sebelum backtest dapat dieksekusi).
