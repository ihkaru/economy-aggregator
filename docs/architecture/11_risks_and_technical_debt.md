# 11. Risks and Technical Debt

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `cc06233`
> - **Verified Timestamp**: `2026-10-30T16:05:09.720863+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 11.1 Known Technical Risks

| ID | Risiko Teknis | Dampak | Strategi Mitigasi |
| :--- | :--- | :--- | :--- |
| **RISK-01** | Waktu kompilasi awal Rust lebih lama dibanding bahasa skrip. | Memperlambat siklus build pertama kali di mesin berdaya rendah. | Memanfaatkan caching Cargo (`target/` cache) dan memisahkan modul-modul independen. |
| **RISK-02** | Penumpukan potongan file Parquet berukuran kecil (*Small Files Problem*) jika sering melakukan flush. | Penurunan kecepatan scan jika terdapat ribuan file kecil dalam satu folder. | Menyediakan modul pemadatan (*compactor*) otomatis harian yang menggabungkan file-file kecil menjadi satu file Parquet terkonsolidasi. |
| **RISK-03** | Pembatasan kuota (*rate limiting*) pada REST API saat melakukan penambalan data (*gap healing*). | Permintaan penambalan gagal jika terjadi pemutusan berulang kali. | Menggunakan antrean penambalan dengan algoritma jeda bertingkat (*exponential backoff*) dan token bucket limiter. |
| **RISK-04** | Pembatasan kuota rate limit atau pemblokiran IP saat deep backfill data sejak tahun 2020. | Penarikan data historis terhenti di tengah jalan. | Menerapkan token-bucket rate limiter lokal, checkpoint timestamp progress (resume tanpa download ulang), dan jeda batched adaptive. |

## 11.2 Technical Debt Ledger

| ID | Catatan Hutang Teknis | Status & Lokasi | Rencana Penyelesaian |
| :--- | :--- | :--- | :--- |
| **DEBT-01** | Scraper kalender ekonomi ForexFactory masih mengandalkan parsing HTML/JSON pihak ketiga karena ketiadaan REST API resmi. | Terkelola (`src/features/ingestion/`) | Mengisolasi scraper di balik trait antarmuka terstandarisasi agar mudah diganti jika ada API resmi di masa depan. |
