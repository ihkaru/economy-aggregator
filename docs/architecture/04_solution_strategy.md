# 4. Solution Strategy

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `271cb11`
> - **Verified Timestamp**: `2026-10-30T18:53:00+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 4.1 Pendekatan Arsitektur Tanpa Jargon

Untuk mencapai sistem pengumpul data yang sangat kencang, hemat memori, dan andal, dipilih pendekatan berikut:

### 1. Ingesti Model Penampung Singkat (*Micro-Batching*)
- **Masalah:** Jika setiap transaksi (tick) yang masuk via WebSocket langsung ditulis satu-per-satu ke harddisk, harddisk akan lambat (*disk bottleneck*) dan komputer cepat panas.
- **Solusi:** Data ditampung terlebih dahulu di memori RAM dalam antrean kecil. Setiap 5 detik sekali (atau setelah terkumpul 10.000 baris), data ditulis sekaligus sebagai satu potongan file Parquet.

### 2. Penataan File Berdasarkan Folder (*Hive Partitioning*)
- **Masalah:** Membuka file database raksasa berukuran puluhan gigabyte hanya untuk mencari harga kemarin akan sangat lambat.
- **Solusi:** File dipecah rapi ke dalam folder:
  `data/lake/market/asset_class=crypto/symbol=BTC_USDT/timeframe=1m/year=2026/month=10/`
  Ketika aplikasi meminta data BTC bulan Oktober 2026, sistem hanya membaca file di folder tersebut dan mengabaikan jutaan data lainnya.

### 3. Penyatuan Simbol Universal (*Rosetta Stone*)
- **Masalah:** Nama ticker berbeda di tiap sumber (misal Yahoo memakai `EURUSD=X`, Dukascopy memakai `EUR/USD`, TradingView memakai `FX:EURUSD`).
- **Solusi:** Sistem memiliki tabel pemetaan internal yang menerjemahkan semua simbol ke dalam satu format standar (`EUR/USD`).

### 4. Pencegah Data Duplikat & Penambal Data Hilang (*Gap Healing*)
- **Masalah:** Saat koneksi internet terputus beberapa detik, ada data yang hilang. Saat koneksi pulih, data bisa ganda antara rekaman live dan data batch.
- **Solusi:** 
  1. Setiap baris memiliki kunci identitas unik (`natural_key`).
  2. Data resmi dari REST API yang sudah final (*settled*) otomatis menggantikan data live sementara (*provisional*).
  3. Sistem mendeteksi nomor urut pesan yang melompat dan langsung memanggil REST API untuk menambal data yang terlewat.

### 5. Setup Instan Satu Konfigurasi (*One-Config-Away*) & CLI Ramah AI-Agent
- **Masalah:** Menyiapkan pengumpulan data multi-aset (crypto, saham, komoditas, kalender makro) secara manual memakan waktu lama dan rentan salah konfigurasi saat dieksekusi otomatis oleh skrip atau agen AI.
- **Solusi:** Menyediakan satu file konfigurasi deklaratif (`economy-aggregator.toml`) yang dapat dibuat instan via `economy-aggregator config init`, didukung antarmuka CLI yang mengembalikan JSON terstruktur, kode keluar deterministik, dan zero-interactivity.

### 6. Sinkronisasi Kalender Multi-Bursa & Normalisasi Mata Uang Basis
- **Masalah:** Bursa saham global beroperasi pada hari libur berbeda (NYSE, IDX, TSE), pasar kripto 24/7, dan denominasi mata uang beragam (USD, IDR, JPY), menyebabkan missing dates dan bias kurs saat di-join.
- **Solusi:** Menyediakan `TradingCalendarAligner` dengan interpolasi deterministik Forward-Fill untuk menyamakan spine tanggal perdagangan, serta `FxNormalizer` untuk otomatis mengonversi valuasi instrumen ke mata uang basis investor (misal IDR).

### 7. Pemodelan Instrumen Kupon & Suku Bunga Sintetis
- **Masalah:** Instrumen pasar uang dan obligasi negara (SBN) atau korporasi tidak memiliki candle continuous tick publik layaknya saham.
- **Solusi:** Modul `SyntheticBondAccumulator` mengompound rate yield acuan menjadi time-series *Total Return Index (TRI)* continuous harian.

### 8. Mesin Analitika Portofolio Terintegrasi (In-Engine Analytics)
- **Masalah:** Menghitung metrik risiko portofolio (CAGR, volatilitas tahunan, max drawdown, Sharpe ratio, dan matriks korelasi Pearson) biasanya memerlukan ekspor data ke Python/Polars eksternal.
- **Solusi:** Mengintegrasikan modul `PortfolioEngine` langsung di dalam Rust binary yang mengevaluasi portofolio multi-aset secara instan via `economy-aggregator analyze`.
