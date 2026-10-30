# Spesifikasi Kebutuhan Data: Pengujian Hipotesis All-Weather & Risk Parity

> **Tujuan:** Menyusun kebutuhan data historis, seri waktu harga, dan indikator makroekonomi untuk menguji secara empiris (*backtesting*) hipotesis portofolio All-Weather versi Sander Parawira (25% SBN, 25% Obligasi Korporasi, 12.5% Saham Bank RI, 12.5% Saham Global, 10% Emas, 7.5% RDPT USD, 7.5% RDPU USD) terhadap tolok ukur (*benchmarks*) pasar tunggal (100% S&P 500, 100% IHSG, 100% Nikkei 225, 100% Bitcoin, Portofolio Klasik 60/40, dan 100% Deposito IDR).

---

## 1. Matriks 7 Kelas Aset Utama Portofolio Sander

Untuk merekonstruksi simulasi portofolio All-Weather secara presisi harian (*daily frequency*), kita memerlukan seri data harga/return historis dari instrumen berikut:

| No | Komponen Portofolio | Bobot Nominal | Proksi Simbol Standar | Provider / Sumber Data | Rentang Waktu Minimum | Variabel yang Dibutuhkan |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **1** | **RDPT Obligasi Pemerintah (SBN)** | 25.0% | `ID10YT=RR` (Indonesia 10Y Bond Yield) / Proksi Indeks SBN IDR | Yahoo Finance / Stooq / Bank Indonesia | 2005 – 2026 (21 tahun) | Daily Close Yield, Kupon Tersirat, Total Return Index |
| **2** | **RDPT Obligasi Korporasi IDR** | 25.0% | IBPA Corporate AAA/AA Spread / Proksi Reksadana RDPT Korporasi | OJK / Bareksa / IBPA / SBN + 1.5–2.5% Spread | 2010 – 2026 (16 tahun) | Daily NAV / Synthetic Corp Bond Return |
| **3** | **Saham Perbankan Indonesia** | 12.5% | `BBCA.JK`, `BBRI.JK`, `BMRI.JK`, `BBNI.JK` (Top 4 Big Banks) | Yahoo Finance / IDX / Stooq | 2004 – 2026 (22 tahun) | Open, High, Low, Close, Adj Close, Volume, Dividend Yield |
| **4** | **Indeks Saham Global** | 12.5% | `^GSPC` (S&P 500) atau `SPY` / `URTH` (MSCI World) | Yahoo Finance / Stooq / Alpaca | 1989 – 2026 (37 tahun) | Daily OHLC, Adjusted Close (Split & Dividend Factored) |
| **5** | **Emas Fisik (Gold)** | 10.0% | `GC=F` (Gold Futures) / `XAU/USD` (Spot Gold) | Yahoo Finance / Binance (`PAXGUSDT`) / Stooq | 1989 – 2026 (37 tahun) | Daily Close (USD/oz & Konversi IDR/gram) |
| **6** | **Obligasi US Dollar (USD Bonds)** | 7.5% | `IEF` (iShares 7-10Y US Treasury) / `EMB` (USD Emerging Market Bond) | Yahoo Finance / Stooq | 2003 – 2026 (23 tahun) | Daily NAV / Total Return Adjusted Close |
| **7** | **Pasar Uang USD (Cash Equivalent)** | 7.5% | `BIL` (SPDR 1-3 Month T-Bill) / SOFR / Fed Funds Effective Rate | FRED (`FEDFUNDS`) / Yahoo Finance (`BIL`) | 2000 – 2026 (26 tahun) | Daily Yield / Compounded Return |

---

## 2. Seri Data Benchmark & Pengujian Stres Ekstrem (Drawdown Invariant)

Transkrip menyebutkan beberapa kasus sejarah ekstrem yang menjadi landasan filosofis mengapa portofolio terkonsentrasi sangat berbahaya:

### A. Nikkei 225 (`^N225`) — The 34-Year Recovery Trap (1989 – 2023)
* **Kebutuhan Data:** Data harian Nikkei 225 dari **1989 hingga 2024**.
* **Hipotesis yang Diuji:** Puncak All-Time High di Desember 1989 ($\approx 38.915$) baru berhasil dilampaui kembali pada Februari 2024 ($\approx 34$ tahun).
* **Objektif:** Menghitung *holding period return*, *maximum drawdown* (mencapai $-82\%$), dan simulasi *Dollar-Cost Averaging* (DCA) top-up bulanan vs Lump-sum.

### B. IHSG (`^JKSE`) — 2008 GFC "Cheap Trap" (-60% Drawdown)
* **Kebutuhan Data:** Data harian IHSG dari **2007 hingga 2010**.
* **Hipotesis yang Diuji:** IHSG sempat anjlok dari puncak $\approx 2.830$ ke level $\approx 1.700$ (turun $-40\%$). Investor yang menganggap "sudah murah dan all-in" di level tersebut tetap mengalami penurunan susulan sebesar $-30\%$ hingga menyentuh dasar $\approx 1.111$ (total drawdown $-60.7\%$).
* **Objektif:** Membuktikan secara matematis bahwa penurunan harga $-40\%$ yang diikuti penurunan lanjutan $-30\%$ menghasilkan drawdown kumulatif sebesar:
  $$1 - (1 - 0.40) \times (1 - 0.30) = 1 - (0.60 \times 0.70) = 1 - 0.42 = 58\% \approx 60\%$$

### C. S&P 500 (`^GSPC`) — Dot-com (2000) & GFC (2008)
* **Kebutuhan Data:** Data harian S&P 500 dari **1999 hingga 2014**.
* **Hipotesis yang Diuji:**
  - Dot-com bubble crash (Maret 2000 – Oktober 2002: drawdown $-49.1\%$, butuh 7 tahun untuk breakeven nominal pada 2007).
  - Great Financial Crisis (Oktober 2007 – Maret 2009: drawdown $-56.8\%$, butuh 5.5 tahun untuk pulih pada Maret 2013).

### D. Bitcoin (`BTC/USDT` / `BTC-USD`) — Siklus 4 Tahunan & Diminishing Multipliers
* **Kebutuhan Data:** Data harian Bitcoin dari **2015 hingga 2026** (melalui Binance REST & Yahoo).
* **Hipotesis yang Diuji:** Kelipatan titik terendah ke titik tertinggi (*cycle low-to-peak multiplier*) mengalami degradasi:
  - Siklus 2015: $\approx 100\times$
  - Siklus 2018: $\approx 20\times$
  - Siklus 2022: $\approx 8\times$
* **Objektif:** Menguji volatilitas, maximum drawdown (rutin $-75\%$ hingga $-85\%$), dan efek likuidasi mendadak pada investor yang tidak memiliki *dry powder*.

### E. Inflasi Riil Indonesia vs Deposito IDR (Rule of 72 Erosion)
* **Kebutuhan Data:**
  - Laju inflasi tahunan BPS (Indeks Harga Konsumen / IHK) dari **1990 hingga 2026**.
  - Suku bunga rata-rata deposito bank umum 1 bulan IDR (Bank Indonesia) dan suku bunga penjaminan LPS.
* **Hipotesis yang Diuji:**
  - Pada inflasi rata-rata 4%, daya beli tergerus $50\%$ setiap $72 / 4 = 18$ tahun ($25\%$ tersisa dalam 35 tahun).
  - Pada bunga deposito net 3% setelah pajak (20%), real return adalah $-1\%$ per tahun, sehingga dalam 35 tahun daya beli riil tersisa hanya $\approx 67\%$ ($2/3$).

---

## 3. Seri Data Makroekonomi untuk Memetakan 4 Kuadran Musim Ray Dalio

Untuk memisahkan regim ekonomi historis menjadi 4 kuadran:

| Variabel Makroekonomi | Sumber Data / Kode Seri | Frekuensi | Aturan Klasifikasi Kuadran |
| :--- | :--- | :--- | :--- |
| **Pertumbuhan PDB Riil AS (US Real GDP YoY)** | FRED: `GDPC1` | Triwulanan | $\Delta \text{GDP} > 0$ (Growth Naik) vs $\Delta \text{GDP} \le 0$ (Growth Turun) |
| **Pertumbuhan PDB Riil Indonesia (ID GDP YoY)** | BPS / Bank Indonesia | Triwulanan | Domestik growth regime indicator |
| **Inflasi IHK AS (US CPI YoY)** | FRED: `CPIAUCSL` / `CPIYOY` | Bulanan | $\Delta \text{CPI} > 0$ (Inflasi Naik) vs $\Delta \text{CPI} \le 0$ (Inflasi Turun) |
| **Inflasi IHK Indonesia (ID CPI YoY)** | BPS / Bank Indonesia | Bulanan | Domestik inflation regime indicator |
| **Suku Bunga Kebijakan Bank Indonesia (BI Rate)** | Bank Indonesia (BI-Rate / 7-Day RR) | Bulanan | Yield curve & corporate bond pricing driver |
| **Suku Bunga Federal Reserve (Fed Funds Rate)** | FRED: `FEDFUNDS` | Bulanan | Global liquidity & USD/IDR pressure driver |
| **Nilai Tukar Rupiah terhadap US Dollar (USD/IDR)** | Yahoo: `USDIDR=X` / Bank Indonesia JISDOR | Harian | FX translation gain/loss pada porsi aset USD |

---

## 4. Evaluasi Arsitektur `economy-aggregator` & Rencana Perbaikan

Saat mengaudit arsitektur `economy-aggregator` saat ini, ditemukan 3 kelemahan mendasar yang perlu diperbaiki:

1. **Ketiadaan Interface CLI yang Responsif & Fungsional:**
   - *Masalah:* `src/main.rs` saat ini langsung menjalankan server HTTP Axum di port 8080 tanpa parser argumen baris perintah (`CLI args`). Tidak ada opsi untuk menarik data tertentu, melakukan backfill, atau mengekspor ringkasan portofolio dari terminal.
   - *Solusi:* Bangun CLI modular dengan parser argumen (mendukung subcommands: `serve`, `pull`, `macro`, `backfill`, `analyze-all-weather`, `info`) dengan output format JSON terstandar (`{"success": true, ...}`) sesuai panduan Agen AI.

2. **Keterbatasan Provider Pasar (Hanya Binance & Biquote):**
   - *Masalah:* `src/features/ingestion/` baru memiliki `BinanceRestFetcher` (khusus Crypto) dan `EconomicCalendarFetcher` (Biquote kalender event). Belum ada fetcher untuk multi-aset global (saham perbankan Indo `BBCA.JK`, indeks global `^GSPC`, `^N225`, `^JKSE`, komoditas emas `GC=F`, obligasi `^TNX`, forex `USDIDR=X`).
   - *Solusi:* Buat modul baru `src/features/ingestion/yahoo_finance.rs` (dan integrasi FRED) untuk menarik daily historical bars instrumen ekuitas, indeks, emas, obligasi, dan forex secara keyless, stabil, dan otomatis terintegrasi ke Parquet lakehouse.

3. **Perluasan Skema Symbology & Engine Backfill Multi-Aset:**
   - *Masalah:* `InstrumentRegistry` di `symbology.rs` baru mencakup BTC, ETH, dan EUR/USD. Belum mencakup semesta instrumen All-Weather (IHSG, S&P 500, Nikkei 225, BBCA, Gold, USD/IDR).
   - *Solusi:* Tambahkan instrumen kanonikal All-Weather ke symbology registry dan bangun pipeline ingestion otomatis yang menyimpan data langsung ke Hive partition Parquet `data/lake/`.
