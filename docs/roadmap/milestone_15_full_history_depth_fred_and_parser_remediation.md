# Milestone 15: Full-History Depth, Official FRED REST v1 Connector & Parser Remediation

**Phases:** 326 – 332  
**Domain:** Remediation of Production Coverage Gaps (OCC Quoted CSV, FINRA Direct XLSX, CoinMetrics Pagination, Treasury 10y Pagination, Yahoo 10y Initial Backfill, BGeometrics Pacing, & Official FRED REST v1 Connector).  
**Primary Goal:** Mengatasi seluruh keterbatasan kedalaman historis dan kegagalan parsing yang ditemukan pada verifikasi produksi M14, tanpa melanggar invarian **Zero Cross-Source Mixing** (setiap seri sumber FRED menggunakan `series_id` dan `source="fred_api"` terpisah).

---

## 1. Root-Cause Audit Temuan Produksi M14

| Temuan Produksi M14 | Akar Masalah Empiris | Rencana Perbaikan |
| :--- | :--- | :--- |
| **`occ_options_oi` = 0 baris** | CSV OCC memakai tanda kutip + pemisah ribuan koma (`11/25/2026,"376,236,882",...,"694,225,811",`); `line.split(',')` memecah angka & `cols.last()` membaca kolom kosong setelah koma trailing. | Gunakan `csv::ReaderBuilder::new().flexible(true)`, bersihkan koma ribuan, baca indeks kolom eksak (`col[11]` OCC Total, `col[3]` Equity Total), *fail-fast* jika 0 baris, tarik 24 bulan dengan tanggal antar-bulan unik. |
| **`finra_margin_debt` = `403 Forbidden`** | Halaman HTML indeks FINRA dihalangi Cloudflare challenge, sedangkan URL statis resmi `https://www.finra.org/sites/default/files/2021-03/margin-statistics.xlsx` merespons **`HTTP/2 200` (`20,993` byte)** secara langsung. | Unduh langsung URL `.xlsx` resmi (`https://www.finra.org/sites/default/files/2021-03/margin-statistics.xlsx` atau override env `FINRA_MARGIN_XLSX_URL`), tanpa menyentuh halaman HTML challenge Cloudflare. |
| **Lag DBnomics Fed H.4.1 / H.8 & ketidaklengkapan histori 10 thn** | DBnomics memiliki lag ~2–3 minggu; `FRED_API_KEY` ditemukan valid di Infisical (`env dev`). | Bangun `FredFetcher` (`source="fred_api"`) opsional via `FRED_API_KEY` untuk `WALCL`, `WTREGEN`, `RRPONTSYD`, `TOTBKCR`, `DTB3`, `T10Y2Y`, `BAMLH0A0HYM2` (2016–2026) pada `series_id` khusus `macro:US:FRED_*` (dan sanitasi `api_key` dari `source_url`). |
| **`treasury_tga` & `TBILL_3M` terbatas** | `fetch_tga_balance` hanya memanggil 500 baris terbaru tanpa filter tanggal; `TBILL_3M` hanya 2 tahun. | Tambahkan `filter=record_date:gte:2016-01-01` + pagination loop di `treasury.rs`, dan loop `TBILL_3M` 10+ tahun (`cur_year - 10 ..= cur_year`). |
| **`Stage-1` SPY & GLD hanya 249 poin (bukan 10 thn)** | `watermark.rs` mengembalikan `365` saat lake kosong dan `catch_up.rs` memetakan Yahoo maksimal `"2y"`. | Naikkan initial empty backfill ke `3650` hari dan petakan `>730` hari ke Yahoo range `"5y"` / `"10y"`. |
| **`coinmetrics` & `bgeometrics` lambat / terpotong** | `coinmetrics.rs` memakai `limit_per_asset` tanpa `page_size=10000` & loop `next_page_url`; `bgeometrics` rentan rate-limit beruntun. | Gunakan `page_size=10000` + `next_page_url` di `coinmetrics.rs`, dan tambahkan jeda sopan (`1.2s`) serta validasi non-empty fail-fast di `bgeometrics.rs`. |

---

## 2. Granular Implementation Phases (Phase 326 – 332)

- **Phase 326 — Official FRED REST v1 Connector (`fred.rs`):**
  Tambahkan `src/features/ingestion/liquidity/fred.rs` membaca `FRED_API_KEY` dari environment untuk menarik `WALCL` (*$M -> USD*), `WTREGEN` (*$M -> USD*), `RRPONTSYD` (*$B -> USD*), `TOTBKCR` (*$B -> USD*), `DTB3`, `T10Y2Y`, dan `BAMLH0A0HYM2` sejak `2016-01-01` dengan `source="fred_api"`, `SourceTier::OfficialApi`, URL tersanitasi bebas secret, dan SHA-256 checksum. Jika `FRED_API_KEY` tidak diset, task dicatat transparan sebagai skipped/unconfigured.
- **Phase 327 — OCC Quoted CSV Parser & 24-Month Pull (`occ_oi.rs`):**
  Ganti `line.split(',')` menjadi `csv::ReaderBuilder::new().flexible(true)`, ekstrak `col[11]` (OCC Total) & `col[3]` (Equity Total) dengan normalisasi koma ribuan, fail-fast bila 0 baris terurai, dan ubah `liquidity_sync.rs` menarik 24 bulan penuh.
- **Phase 328 — FINRA Direct Official XLSX Connector (`finra_margin.rs`):**
  Arahkan `FinraMarginFetcher` mengunduh langsung berkas resmi `/sites/default/files/2021-03/margin-statistics.xlsx` (atau `FINRA_MARGIN_XLSX_URL`) menggunakan Pure Rust `build_api_client()` (`User-Agent: economy-aggregator/<ver>` jujur, bebas penolakan Cloudflare UA-mismatch tanpa subprocess `curl` dan tanpa menyentuh halaman HTML challenge), menghasilkan 358 bulan historis (1997–2026).
- **Phase 329 — US Treasury 10y TGA Pagination & 10y T-Bill Loop (`treasury.rs`, `liquidity_sync.rs`):**
  Tambahkan filter `record_date:gte:2016-01-01` + pemetaan nama akun pra-2022 (`Federal Reserve Account` → `Treasury General Account (TGA)` → `Treasury General Account (TGA) Closing Balance`, mengecualikan `Opening Balance`) + loop `page[number]` di `fetch_tga_balance`, serta perluas loop `fetch_tbill_3m_year` ke `cur_year - 10 ..= cur_year`.
- **Phase 330 — Multi-Page CoinMetrics & Paced BGeometrics (`coinmetrics.rs`, `bgeometrics.rs`):**
  Tambahkan `page_size=10000` + paginasi `next_page_url` pada `CoinMetricsFetcher`, jeda antar-endpoint pada `BGeometricsFetcher`, dan fail-fast non-empty assertion.
- **Phase 331 — 10-Year Initial Yahoo Candle Backfill (`watermark.rs`, `catch_up.rs`):**
  Ubah `None => (true, 3650)` pada `WatermarkScanner` dan perluas pemetaan Yahoo `range` pada `catch_up.rs` (`>730 => "5y"`, `>1825 => "10y"`) agar `SPY`, `GLD`, `^GSPC`, `^MOVE`, `DX-Y.NYB` memiliki kedalaman Stage-1 hingga 10 tahun.
- **Phase 332 — Catalog, Analytics Lineage & Gauntlet Verification (`series_catalog.rs`, `observation_routes.rs`):**
  Daftarkan seluruh seri `macro:US:FRED_*` ke `SeriesCatalog`, perbarui `query_liquidity_panel_handler` agar memilih seri `FRED_WALCL_USD` / `FED_TOTAL_ASSETS_USD` secara eksplisit tanpa mencampur sumber + melaporkan `series_used`, dan lulus seluruh test suite + VSA + arc42-lint + deploy Coolify dengan `FRED_API_KEY` dari Infisical.
