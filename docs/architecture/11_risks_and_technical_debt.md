# 11. Risks and Technical Debt

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `271cb11`
> - **Verified Timestamp**: `2026-10-30T18:53:00+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 11.1 Known Technical Risks

| ID | Risiko Teknis | Dampak | Strategi Mitigasi |
| :--- | :--- | :--- | :--- |
| **RISK-01** | Waktu kompilasi awal Rust lebih lama dibanding bahasa skrip. | Memperlambat siklus build pertama kali di mesin berdaya rendah. | Memanfaatkan caching Cargo (`target/` cache) dan memisahkan modul-modul independen. |
| **RISK-02** | Penumpukan potongan file Parquet berukuran kecil (*Small Files Problem*) jika sering melakukan flush. | Penurunan kecepatan scan jika terdapat ribuan file kecil dalam satu folder. | Menyediakan modul pemadatan (*compactor*) `LakeCompactor::compact_all()` yang menggabungkan file-file kecil menjadi satu file Parquet terkonsolidasi per bulan. |
| **RISK-03** | Pembatasan kuota (*rate limiting*) pada REST API saat melakukan penambalan data (*gap healing*). | Permintaan penambalan gagal jika terjadi pemutusan berulang kali. | Menggunakan antrean penambalan dengan algoritma jeda bertingkat (*exponential backoff*) dan token bucket limiter. |
| **RISK-04** | Pembatasan kuota rate limit atau pemblokiran IP saat deep backfill data sejak tahun 2020. | Penarikan data historis terhenti di tengah jalan. | Menerapkan token-bucket rate limiter lokal, checkpoint timestamp progress (resume tanpa download ulang), dan jeda batched adaptive. |
| **RISK-05** | Glitch / fat-finger outlier quotes dari API publik (misal USD/IDR bernilai 888.11). | Data kotor merusak kalkulasi drawdown dan volatilitas portofolio. | Menerapkan `SanityGate::is_price_outlier` pada ingestion path untuk me-reject lonjakan harga abnormal (>5x jump/crash) sebelum masuk ke storage. |
| **RISK-06** | Asinkronitas kalender libur bursa global (NYSE libur MLK, IDX libur Lebaran, Kripto 24/7). | Matriks cross-asset memiliki tanggal bolong / missing gaps yang menggagalkan join. | Menerapkan `TradingCalendarAligner` dengan kebijakan deterministik Forward-Fill untuk menyinkronkan timeline panel. |
| **RISK-07** | Distorsi valuasi multi-mata uang (aset USD dianalisis tanpa penyesuaian IDR investor). | Return aset luar negeri bias terhadap pergerakan riil daya beli investor domestik. | Menyediakan `FxNormalizer` untuk mentransformasi valuasi aset asing ke mata uang basis (IDR) berbasis runtun waktu kurs harian. |
| **RISK-08** | Ketiadaan continuous tick pada instrumen fixed income (SBN 10Y dan Obligasi Korporasi). | Analisis portofolio multi-aset (All-Weather) tidak dapat menyertakan kelas aset obligasi. | Menyediakan `SyntheticBondAccumulator` yang mengompound rate yield menjadi Continuous Total Return Index (TRI). |

## 11.2 Technical Debt Ledger

| ID | Catatan Hutang Teknis | Status & Lokasi | Rencana Penyelesaian |
| :--- | :--- | :--- | :--- |
| **DEBT-01** | Scraper kalender ekonomi ForexFactory masih mengandalkan parsing HTML/JSON pihak ketiga karena ketiadaan REST API resmi. | Terkelola (`src/features/ingestion/`) | Mengisolasi scraper di balik trait antarmuka terstandarisasi agar mudah diganti jika ada API resmi di masa depan. |
| **DEBT-02** | Mesin komputasi kuantitatif sebelumnya memerlukan runtime eksternal (Python/Polars) untuk analisis risiko portofolio. | Selesai / Terintegrasi (`src/features/analytics/`) | Mengimplementasikan `PortfolioEngine` native murni di Rust yang menghitung CAGR, Volatilitas, Max Drawdown, Sharpe, dan Korelasi Pearson langsung dari Parquet lakehouse via sub-perintah `economy-aggregator analyze`. |
