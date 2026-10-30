# 4. Solution Strategy

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e773941`
> - **Verified Timestamp**: `2026-10-30T20:02:43+07:00`
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

### 7. Penggunaan Instrumen Fixed Income Empiris (Zero-Synthetic Bond Mandate)
- **Masalah:** Menggunakan yield konstan sintetis ($1.072^t$) pada obligasi menghasilkan runtun waktu tanpa volatilitas dan nol drawdown palsu, yang merusak akurasi backtesting dan melebih-lebihkan rasio Sharpe.
- **Solusi:** Mesin analisis portofolio mewajibkan penggunaan instrumen empiris pasar riil (seperti ETF Obligasi Total Return: TLT, IEF, LQD) yang memuat dinamika durasi, konveksitas, dan fluktuasi harga pasar sekunder nyata. Modul `SyntheticBondAccumulator` hanya dipertahankan sebagai alat komparasi teoritis.

### 8. Mesin Analitika Portofolio Terintegrasi (In-Engine Analytics)
- **Masalah:** Menghitung metrik risiko portofolio (CAGR, volatilitas tahunan, max drawdown, Sharpe ratio, dan matriks korelasi Pearson) biasanya memerlukan ekspor data ke Python/Polars eksternal.
- **Solusi:** Mengintegrasikan modul `PortfolioEngine` langsung di dalam Rust binary yang mengevaluasi portofolio multi-aset secara instan via `economy-aggregator analyze`.

### 9. Deteksi Anomali Non-Parametrik Berbasis Statistik (Hampel Filter & Rolling MAD)
- **Masalah:** Filter berbasis ambang batas statis (hardcoded) seperti `price < 5000.0` atau `symbol == "USD_IDR"` rapuh, tidak skalabel, dan melanggar prinsip kebersihan kode arsitektur jangka panjang.
- **Solusi:** Mengimplementasikan `HampelFilter` berbasis Median Absolute Deviation (MAD) dengan jendela simetris bergerak ($k=7$) dan ambang $3.5\sigma$ yang secara otomatis mendeteksi dan membersihkan outlier tanpa asumsi nilai absolut maupun nama simbol.

### 10. Penyesuaian Corporate Action Saham (Split & Dividend Backward Adjustment)
- **Masalah:** Pemecahan saham (misal 1:4) dan pembayaran dividen tunai menimbulkan lonjakan atau kejatuhan harga artifisial semalam (-75%), merusak perhitungan drawdown dan kontinuitas return.
- **Solusi:** Mengimplementasikan `CorporateActionAdjuster` yang menghitung faktor penyesuaian kumulatif ke belakang (*backward adjustment*) sehingga grafik dan analisis historis mencerminkan Total Return Index (TRI) investor yang sesungguhnya.

### 11. Bitemporal Point-in-Time (PIT) untuk Data Makroekonomi
- **Masalah:** Angka makro (GDP, CPI, NFP) sering direvisi 1–12 bulan setelah rilis awal. Melatih atau menguji model dengan data revisi final memicu *Look-Ahead Bias* berbahaya karena investor di masa lalu hanya memiliki data estimasi awal.
- **Solusi:** Mengadopsi model bitemporal `PointInTimeEngine` yang melacak `reference_period`, `release_timestamp_utc`, dan versi vintage. Kueri simulasi hanya boleh membaca data yang telah resmi beredar pada tanggal simulasi (*as-of date*).

### 12. Penataan Runtun Komoditas Berkelanjutan (Futures Roll) & Sesi Valas 24/5
- **Masalah:** Kontrak berjangka komoditas yang berganti setiap bulan/kuartal menimbulkan gap lonjakan harga (*contango/backwardation*), sedangkan pasar valas Minggu malam menimbulkan bar kosong atau desinkronisasi timestamp.
- **Solusi:** Mengimplementasikan `FuturesRollEngine` dengan metode rasio Panama dan selisih aditif (aman untuk komoditas bernilai negatif seperti WTI 2020), serta `ForexSessionNormalizer` yang menyatukan tick Minggu malam ke sesi perdagangan Senin.

### 13. Arsitektur Medallion: Preservasi Data Mentah (Bronze) vs Transformasi Downstream (Silver/Gold)
- **Masalah:** Menyaring atau membuang data anomali (seperti kejatuhan harga split 4:1 atau harga komoditas negatif) secara destruktif pada pintu masuk danau data (*ingestion gate*) merusak rekaman kebenaran mentah bursa (*ground truth*) dan membatasi fleksibilitas analisis masa depan.
- **Solusi:** Menerapkan pemisahan bertingkat gaya Medallion:
  1. *Bronze Tier (Storage Lake):* Menyimpan 100% data mentah apa adanya dari bursa tanpa pemotongan destruktif.
  2. *Silver/Gold Tier (Downstream Analytics):* Menyediakan perkakas transformasi non-destruktif (`HampelFilter`, `CorporateActionAdjuster`, `PointInTimeEngine`, `FuturesRollEngine`) yang dieksekusi saat data dikonsumsi sesuai kebutuhan spesifik model kuantitatif hilir.
