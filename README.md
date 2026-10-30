# Economy Aggregator

A high-performance, self-contained financial and macroeconomic data aggregator engine built in **100% Rust** (Zero-GC, low RAM footprint $\le$ 50 MB idle, $\le$ 150 MB load).

---

## 1. Filosofi Inti: Preservasi Data Mentah (Raw Data Lakehouse Invariant)

> [!IMPORTANT]
> **Prinsip Imutabilitas Data Mentah (Bronze Tier):**  
> Sistem pengumpul data (`economy-aggregator`) memegang teguh prinsip **"Biarkan data apa adanya sesuai sumber aslinya (Raw Data Fidelity to Source)"**.  
> Lapisan penyimpanan danau data (*lakehouse*) dilarang keras melakukan manipulasi *lossy*, pemotongan sepihak, pembulatan destruktif, atau penghapusan harga anomali/negatif pada saat ingesti.
> 
> **Pemisahan Tanggung Jawab (Separation of Concerns):**  
> 1. **Lapisan Agregator & Danau Data (Bronze Layer):** Bertanggung jawab menyimpan rekaman transaksi pasar mentah secara deterministik, lengkap, dan tanpa filter destruktif.
> 2. **Lapisan Pengolah Data Downstream (Silver/Gold Tier):** Setiap analis data, kuantitatif trader, dan algoritma machine learning memiliki kebutuhan matematis yang berbeda. Seluruh pembersihan anomali statistik, penyesuaian pemecahan saham (*stock split*), eliminasi *look-ahead bias*, dan perataan kontrak berjangka didelegasikan kepada konsumen data downstream menggunakan perkakas modul yang telah disediakan.

---

## 2. Katalog Potensi Permasalahan Data per Kelas Aset (Downstream Awareness Guide)

Pengolah data hilir (*downstream consumers*) wajib menyadari anomali struktural berikut pada data mentah:

| Kelas Aset | Potensi Masalah pada Data Mentah | Dampak Jika Diolah Mentah-mentah | Solusi / Tool Downstream yang Disediakan |
| :--- | :--- | :--- | :--- |
| **Equities (Saham & ETF)** | **Stock Splits & Reverse Splits** | Harga mentah anjlok hingga 75%–90% dalam semalam saat split (misal 1:4), memicu sinyal jual atau perhitungan *drawdown* palsu. | Gunakan [`CorporateActionAdjuster`](src/features/analytics/corporate_actions.rs) untuk menghitung faktor penyesuaian kumulatif ke belakang (*backward adjustment*). |
| **Equities (Saham & ETF)** | **Cash Dividends Drop** | Harga saham turun pada tanggal *ex-dividend* sebesar nilai dividen, meremehkan pertumbuhan majemuk emiten dividen tinggi. | Gunakan penyesuaian dividen pada [`CorporateActionAdjuster`](src/features/analytics/corporate_actions.rs) untuk menghasilkan *Total Return Index (TRI)*. |
| **Equities (Saham & ETF)** | **Survivorship Bias** | Data emiten masa lalu yang telah bangkrut/delisted hilang dari indeks saat ini, membuat backtest terlihat terlalu menguntungkan. | Lacak symbology master table dan *delisting flag* pada danau data. |
| **Cryptocurrency (24/7)** | **Sequence Gaps & Frame Dropouts** | Lonjakan volume ekstrem saat likuidasi massal menyebabkan frame WebSocket TCP terputus, merusak state order book. | Gunakan [`SanityGate::track_sequence`](src/features/ingestion/sanity_gate.rs) dan penambal otomatis (*gap healing*) via REST snapshot. |
| **Cryptocurrency (24/7)** | **Depeg Wicks & Flash Crashes** | Transaksi likuidasi di buku order tipis menciptakan sumbu (*wick*) anomali sesaat yang meracuni rata-rata bergerak (SMA/RSI). | Terapkan [`HampelFilter`](src/features/analytics/outlier.rs) (Rolling MAD non-parametrik) untuk menyaring spike ekstrem tanpa merusak volatilitas riil. |
| **Cryptocurrency (24/7)** | **Dual Timestamps Drift** | Perbedaan jam server bursa (*Event Time*) dengan waktu tiba di jaringan lokal (*Ingestion Time*). | Gunakan field `timestamp_utc` untuk waktu bursa dan urutkan antrean berbasis watermark. |
| **Forex & Fixed Income** | **Sunday Opening & Weekend Gaps** | Pasar Valas buka Minggu 17:00 NY; perbedaan vendor dalam menandai candle Minggu menimbulkan tanggal bolong (*missing joins*). | Gunakan [`ForexSessionNormalizer`](src/features/analytics/futures_roll.rs) untuk menyatukan sesi Minggu malam ke Senin secara deterministik. |
| **Forex & Fixed Income** | **Broken Currency Pegs** | Pemutusan pasak kurs mendadak (misal depeg EUR/CHF 1.20 pada Jan 2015) memicu lompatan harga 30% dalam hitungan detik. | Hindari asumsi distribusi normal Gaussian; gunakan model *heavy-tail jump* dan Rosetta Stone. |
| **Forex & Fixed Income** | **Negative / Zero Yields** | Yield obligasi dan suku bunga pasar uang dapat menyentuh 0% atau negatif di era krisis, merusak kalkulasi $\ln(r_t / r_{t-1})$. | Gunakan selisih absolut (*basis points*) alih-alih rasio logaritmik saat yield $\le 0$. |
| **Commodities** | **Futures Roll Gaps (Contango/Backwardation)** | Pergantian kontrak berjangka yang kedaluwarsa memicu lompatan harga semu antarkontrak (*step gap*). | Gunakan [`FuturesRollEngine`](src/features/analytics/futures_roll.rs) dengan metode rasio Panama (*Panama Ratio*) atau selisih aditif. |
| **Commodities** | **Negative Commodity Pricing** | Harga minyak mentah WTI anjlok ke -$37.63/bbl pada April 2020. Metode rasio matematika membagi dengan nilai negatif. | Gunakan metode selisih aditif (*Additive Difference*) pada [`FuturesRollEngine`](src/features/analytics/futures_roll.rs) saat harga $\le 0$. |
| **Macroeconomic Calendar** | **Data Revisions & Look-Ahead Bias** | Angka PDB, Inflasi (CPI), dan Ketenagakerjaan (NFP) direvisi berbulan-bulan setelah rilis awal. Membaca data revisi final memicu bias masa depan. | Gunakan [`PointInTimeEngine`](src/features/analytics/bitemporal.rs) dengan filter bitemporal strictly `as_of` tanggal simulasi (*Zero Look-Ahead Bias*). |

---

## 3. Perkakas Pengolahan Downstream Bawaan (Native Rust Analytics Tools)

Repositori ini menyertakan mesin pengolahan downstream berbasis Rust murni tanpa ketergantungan luar:

1. **`HampelFilter` ([`src/features/analytics/outlier.rs`](src/features/analytics/outlier.rs)):**  
   Pembersihan outlier statistik non-parametrik adaptif berbasis Median Absolute Deviation (MAD):
   $$|x_t - M_t| > n \times 1.4826 \times \text{MAD}_t$$
2. **`CorporateActionAdjuster` ([`src/features/analytics/corporate_actions.rs`](src/features/analytics/corporate_actions.rs)):**  
   Perhitungan faktor kumulatif ke belakang (*backward adjustment*) untuk split dan dividen tunai.
3. **`PointInTimeEngine` ([`src/features/analytics/bitemporal.rs`](src/features/analytics/bitemporal.rs)):**  
   Kueri bitemporal indikator makroekonomi berdasarkan tanggal rilis historis yang sah (*as-of date*).
4. **`FuturesRollEngine` & `ForexSessionNormalizer` ([`src/features/analytics/futures_roll.rs`](src/features/analytics/futures_roll.rs)):**  
   Penyambung runtun kontrak berjangka berkelanjutan dan penyelarasan sesi Valas 24/5.
5. **`TradingCalendarAligner` & `FxNormalizer` ([`src/features/analytics/calendar_aligner.rs`](src/features/analytics/calendar_aligner.rs)):**  
   Sinkronisasi kalender multi-bursa via forward-fill dan normalisasi multi-mata uang ke IDR.
6. **`PortfolioEngine` ([`src/features/analytics/portfolio.rs`](src/features/analytics/portfolio.rs)):**  
   Kalkulasi CAGR, Volatilitas Tahunan, Max Drawdown, Sharpe Ratio, dan Matriks Korelasi Pearson cross-asset.

---

## 4. Antarmuka Baris Perintah (Agent-First CLI)

```bash
# 1. Cek status danau data
cargo run -- status

# 2. Tarik data historis multi-dekade (disimpan apa adanya di Parquet lake)
cargo run -- pull --symbol BBCA.JK --canonical BBCA --asset equities --range max
cargo run -- pull --symbol ^GSPC --canonical GSPC --asset equities --range max
cargo run -- pull --symbol TLT --canonical TLT --asset equities --range max
cargo run -- pull --symbol IEF --canonical IEF --asset equities --range max
cargo run -- pull --symbol LQD --canonical LQD --asset equities --range max
cargo run -- pull --symbol GC=F --canonical XAU_USD --asset commodities --range max
cargo run -- pull --symbol USDIDR=X --canonical USD/IDR --asset forex --range max

# 3. Eksekusi simulasi portofolio All-Weather 100% empiris riil
cargo run -- analyze --start-year 2004

# 4. Padatkan potongan partisi kecil menjadi file Parquet bulanan terkonsolidasi
cargo run -- compact
```

---

## 5. Roadmap & Kepatuhan Arsitektur
- 🗺️ **[Master Implementation Roadmap (260 Fase)](docs/roadmap/ROADMAP.md)**
- 🏛️ **[Dokumentasi Arsitektur arc42 (Seksi 01 - 12)](docs/architecture/)**
- 📋 **[Pedoman & Invarian Teknis GEMINI.md](GEMINI.md)**
