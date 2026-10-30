# 11. Risks and Technical Debt

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e773941`
> - **Verified Timestamp**: `2026-10-30T20:02:43+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 11.1 Known Technical Risks

| ID | Risiko Teknis | Dampak | Strategi Mitigasi |
| :--- | :--- | :--- | :--- |
| **RISK-01** | Waktu kompilasi awal Rust lebih lama dibanding bahasa skrip. | Memperlambat siklus build pertama kali di mesin berdaya rendah. | Memanfaatkan caching Cargo (`target/` cache) dan memisahkan modul-modul independen. |
| **RISK-02** | Penumpukan potongan file Parquet berukuran kecil (*Small Files Problem*) jika sering melakukan flush. | Penurunan kecepatan scan jika terdapat ribuan file kecil dalam satu folder. | Menyediakan modul pemadatan (*compactor*) `LakeCompactor::compact_all()` yang menggabungkan file-file kecil menjadi satu file Parquet terkonsolidasi per bulan. |
| **RISK-03** | Pembatasan kuota (*rate limiting*) pada REST API saat melakukan penambalan data (*gap healing*). | Permintaan penambalan gagal jika terjadi pemutusan berulang kali. | Menggunakan antrean penambalan dengan algoritma jeda bertingkat (*exponential backoff*) dan token bucket limiter. |
| **RISK-04** | Pembatasan kuota rate limit atau pemblokiran IP saat deep backfill data sejak tahun 2020. | Penarikan data historis terhenti di tengah jalan. | Menerapkan token-bucket rate limiter lokal, checkpoint timestamp progress (resume tanpa download ulang), dan jeda batched adaptive. |
| **RISK-05** | Glitch / fat-finger outlier quotes dari API publik (misal USD/IDR bernilai 888.11). | Data kotor merusak kalkulasi drawdown dan volatilitas portofolio jika tidak ditangani. | Menerapkan `HampelFilter` statistik non-parametrik (Rolling MAD $k=7, 3.5\sigma$) pada analitika downstream dan audit logger, tanpa menghapus data mentah di lakehouse. |
| **RISK-06** | Asinkronitas kalender libur bursa global (NYSE libur MLK, IDX libur Lebaran, Kripto 24/7). | Matriks cross-asset memiliki tanggal bolong / missing gaps yang menggagalkan join. | Menerapkan `TradingCalendarAligner` dengan kebijakan deterministik Forward-Fill untuk menyinkronkan timeline panel. |
| **RISK-07** | Distorsi valuasi multi-mata uang (aset USD dianalisis tanpa penyesuaian IDR investor). | Return aset luar negeri bias terhadap pergerakan riil daya beli investor domestik. | Menyediakan `FxNormalizer` untuk mentransformasi valuasi aset asing ke mata uang basis (IDR) berbasis runtun waktu kurs harian. |
| **RISK-08** | Distorsi dan bias artifisial akibat kurva obligasi sintetis flat yield (Zero Volatility bias). | Sharpe ratio dan drawdown portofolio terdistorsi secara palsu. | Menetapkan Invarian Zero-Synthetic Data: analisis portofolio wajib menggunakan 100% instrumen pasar empiris riil (seperti ETF Obligasi Total Return: TLT, IEF, LQD) dengan sensitivitas durasi-konveksitas nyata. |
| **RISK-09** | Distorsi harga akibat stock split dan dividen tunai saham (false crash & return bias). | Backtest memicu sinyal jual palsu dan salah hitung drawdown pada tanggal ex-date. | Menerapkan `CorporateActionAdjuster` backward-adjustment kumulatif untuk menyelaraskan runtun harga historis ke Total Return Index (TRI). |
| **RISK-10** | Look-Ahead Bias pada rilis indikator makroekonomi (GDP, CPI, NFP) akibat revisi berkala. | Model prediktif overfitting karena membaca angka revisi masa depan yang belum ada di masa lalu. | Menerapkan `PointInTimeEngine` dengan model bitemporal untuk mengunci pembacaan data strictly as-of tanggal simulasi. |
| **RISK-11** | Step jump artifisial akibat pergantian kontrak berjangka komoditas (Contango/Backwardation). | False breakout dan volatilitas palsu pada tanggal kedaluwarsa kontrak. | Menerapkan `FuturesRollEngine` dengan metode rasio Panama dan selisih aditif backward-adjusted. |
| **RISK-12** | Pembersihan destruktif prematur di layer ingesti/penyimpanan (Bronze tier). | Kehilangan jejak sejarah pasar riil (misal minyak WTI negatif -$37.63, stock split 4:1 terhapus karena filter outlier, depeg mata uang, flash crash). | Menetapkan Invarian Preservasi Bronze: Danau data menyimpan data mentah apa adanya (*as-is*). Seluruh pembersihan, filter outlier, dan adjustment wajib dieksekusi di downstream (Silver/Gold) sesuai kebutuhan strategi kuantitatif. |

## 11.2 Technical Debt Ledger

| ID | Catatan Hutang Teknis | Status & Lokasi | Rencana Penyelesaian |
| :--- | :--- | :--- | :--- |
| **DEBT-01** | Scraper kalender ekonomi ForexFactory masih mengandalkan parsing HTML/JSON pihak ketiga karena ketiadaan REST API resmi. | Terkelola (`src/features/ingestion/`) | Mengisolasi scraper di balik trait antarmuka terstandarisasi agar mudah diganti jika ada API resmi di masa depan. |
| **DEBT-02** | Mesin komputasi kuantitatif sebelumnya memerlukan runtime eksternal (Python/Polars) untuk analisis risiko portofolio. | Selesai / Terintegrasi (`src/features/analytics/`) | Mengimplementasikan `PortfolioEngine` native murni di Rust yang menghitung CAGR, Volatilitas, Max Drawdown, Sharpe, dan Korelasi Pearson langsung dari Parquet lakehouse via sub-perintah `economy-aggregator analyze`. |
