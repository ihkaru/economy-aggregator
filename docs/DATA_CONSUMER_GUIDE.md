# Downstream Data Consumer & Modeling Guide

Panduan resmi bagi quant trader, data scientist, dan engineer machine learning yang mengonsumsi data dari lakehouse `economy-aggregator`.

---

## 1. Filosofi Medallion Lakehouse: Kenapa Data Mentah Tidak Diubah?

Danau data (`economy-aggregator`) beroperasi dengan prinsip **Imutabilitas Bronze Tier (Raw Data Fidelity to Source)**:
* **Tidak Ada Penghapusan Sepihak:** Rekaman bursa mentah disimpan apa adanya (*as-is*). Harga negatif riil (seperti WTI -$37.63/bbl pada April 2020) atau penurunan harga semalam akibat stock split 1:4 **TIDAK DIHAPUS** dari danau data.
* **Separation of Concerns:** Pembersihan destruktif prematur di danau data merusak audit trail historis. Tugas membersihkan data didelegasikan kepada konsumen hilir (*downstream*) sesuai tujuan model masing-masing.

---

## 2. Invarian Pemodelan Wajib (Downstream Guardrails)

Ketika Anda membangun model prediksi, simulasi portofolio, atau backtest strategi di atas data kami, Anda **WAJIB** mematuhi 5 invarian berikut:

### 1. Invarian Instrumen Derivatif & Opsi (Anti-Compounding Glitch)
> [!CAUTION]
> **DILARANG MENG-COMPOUND HARGA OPSI SECARA LINIER HARIAN!**
> Memodelkan instrumen opsi (seperti *Long Volatility / Tail Risk Put Options*) sebagai deret waktu return harian $R_t$ yang di-*compound* menghasilkan anomali matematis tidak nyata ($10^{32}$ kumulatif atau CAGR 46%).
* **Kenyataan Finansial:**
  1. Opsi memiliki umur kedaluwarsa (*expiration*) dan menderita peluruhan nilai waktu harian (*theta decay*).
  2. Saat krisis pasar, delta opsi mendekati batas fisik $\Delta \le 1.0$ (tidak bisa menghasilkan leverage tak terbatas).
  3. Setelah pasar memantul (*rebound*), terjadi *volatility crush* yang memangkas harga opsi secara instan.
* **Solusi Baku:**
  Gunakan model proteksi ekuitas berbasis anggaran tetap tahunan (alokasi 1%–2% kas yang hangus saat tenang dan dicairkan tunai saat krisis), atau kalibrasikan ke benchmark riil seperti **CBOE Eurekahedge Long Volatility Index (`EHFI451`)**.

---

### 2. Invarian Kalender Multi-Bursa (Calendar Alignment)
Bursa global memiliki hari libur berbeda:
* NYSE tutup saat Martin Luther King Jr. Day.
* IDX (Bursa Efek Indonesia) tutup saat Libur Lebaran.
* Kripto buka 24 jam sehari, 7 hari seminggu.
* Pasar Forex tutup Sabtu-Minggu tetapi buka Minggu pukul 17:00 NY.

> [!IMPORTANT]
> **Wajib Menggunakan Forward-Fill Alignment:**
> Jangan pernah melakukan *inner join* tanpa penyelarasan kalender karena akan membuang 30%+ titik data. Gunakan `TradingCalendarAligner` bawaan untuk menyelaraskan spine tanggal sebelum menghitung matriks kovariansi atau korelasi Pearson.

---

### 3. Invarian Deteksi Anomali Non-Parametrik (Hampel Filter)
API publik kadang-kadang memancarkan *fat-finger glitch* (misal kurs USD/IDR bernilai 888.11).
* **Jangan Gunakan Nilai Batas Statis (Hardcoded Bounds):** Dilarang menulis filter seperti `if price < 5000` karena batas tersebut akan patah saat rezim pasar berubah.
* **Gunakan Hampel Filter:** Filter berbasis Rolling Median Absolute Deviation (MAD):
  $$|x_t - M_t| > n \times 1.4826 \times \text{MAD}_t$$
  Gunakan sliding window simetris $2k + 1 = 15$ titik ($k=7$) dan ambang $n = 3.5\sigma$.

---

### 4. Invarian Total Return Index & Stock Split (Aksi Korporasi)
* Saham mentah mengalami penurunan harga saat *ex-dividend date* dan melompat tajam saat *stock split*.
* **Penyesuaian Ganda (Harga & Volume):** Saat stock split 5:1 terjadi, harga historis pra-split dibagi 5 ($P \times 0.2$), sedangkan volume perdagangan historis dikalikan 5 ($V / 0.2$). Hal ini menjamin nilai omzet transaksi (*dollar turnover*) masa lalu tetap ekuivalen secara ekonomi.
* Gunakan `CorporateActionAdjuster::adjust_ohlcv_backward()` untuk mentransformasi lilin mentah ke lilin yang telah disesuaikan secara berkesinambungan.

---

### 5. Invarian Point-in-Time (Bitemporal Macro Data)
* Rilis data ekonomi makro (GDP, CPI, Payrolls) mengalami revisi berbulan-bulan setelah rilis awal.
* Membaca angka revisi final untuk keputusan perdagangan di masa lalu merupakan kesalahan fatal (*Look-Ahead Bias*).
* Gunakan `PointInTimeEngine` untuk menyaring data makro *strictly as-of* tanggal simulasi yang bersangkutan.

---

### 6. Invarian Symbology Temporal (Point-in-Time Ticker Drift)
* Simbol ticker dapat berganti sewaktu-waktu akibat rebranding (misal `FB` $\to$ `META` pada 9 Juni 2022) atau restrukturisasi emiten.
* **Jangan Menggunakan Pemetaan String Statis:** Kueri data historis sebelum tanggal rebranding wajib menggunakan fungsi resolusi temporal `InstrumentRegistry::resolve_at(vendor, raw_ticker, as_of_date)` agar riwayat emiten sebelum perubahan nama tidak hilang atau terpecah.

---

## 3. Toolkit Pengolahan Rust Bawaan

| Modul Pengolahan | Lokasi Kode Sumber | Fungsi Utama |
| :--- | :--- | :--- |
| `InstrumentRegistry` | [`src/features/storage/symbology.rs`](file:///root/projects/economy-aggregator/src/features/storage/symbology.rs) | Resolusi Point-in-Time Ticker & identitas permanen (ISIN/FIGI). |
| `HampelFilter` | [`src/features/analytics/outlier.rs`](file:///root/projects/economy-aggregator/src/features/analytics/outlier.rs) | Deteksi outlier statistik non-parametrik adaptif. |
| `TradingCalendarAligner` | [`src/features/analytics/calendar_aligner.rs`](file:///root/projects/economy-aggregator/src/features/analytics/calendar_aligner.rs) | Sinkronisasi multi-bursa dengan deterministic forward-fill. |
| `FxNormalizer` | [`src/features/analytics/fx_normalizer.rs`](file:///root/projects/economy-aggregator/src/features/analytics/fx_normalizer.rs) | Normalisasi valuasi aset asing ke IDR. |
| `CorporateActionAdjuster` | [`src/features/analytics/corporate_actions.rs`](file:///root/projects/economy-aggregator/src/features/analytics/corporate_actions.rs) | Backward adjustment OHLCV (harga dan volume) untuk split & dividen. |
| `PointInTimeEngine` | [`src/features/analytics/bitemporal.rs`](file:///root/projects/economy-aggregator/src/features/analytics/bitemporal.rs) | Kueri bitemporal untuk mencegah look-ahead bias makro. |
| `FuturesRollEngine` | [`src/features/analytics/futures_roll.rs`](file:///root/projects/economy-aggregator/src/features/analytics/futures_roll.rs) | Penyambungan kontrak berjangka via Panama Ratio & Additive Diff. |
| `PortfolioEngine` | [`src/features/analytics/portfolio.rs`](file:///root/projects/economy-aggregator/src/features/analytics/portfolio.rs) | Evaluasi CAGR, Volatilitas, Max Drawdown, Sharpe, & Korelasi. |
| `LakeReader` | [`src/features/storage/reader.rs`](file:///root/projects/economy-aggregator/src/features/storage/reader.rs) | Pembacaan partisi Parquet lakehouse dengan deduplikasi settled records. |
| `ReconciliationEngine` | [`src/features/analytics/reconciliation.rs`](file:///root/projects/economy-aggregator/src/features/analytics/reconciliation.rs) | Audit integritas candle lakehouse vs feed eksternal (Yahoo, Binance, Dukascopy, TradingView). |

---

## 4. Contoh Pemanggilan Cepat dari CLI

```bash
# 1. Analisis performa multi-aset 100% data empiris riil
cargo run -- analyze --start-year 2004

# 2. Rekonsiliasi integritas data lakehouse terhadap feed live (misal BBCA via Yahoo)
cargo run -- reconcile --symbol BBCA --asset equities --source yahoo --days 30

# 3. Rekonsiliasi data kripto terhadap Binance REST API
cargo run -- reconcile --symbol BTC/USDT --asset crypto --source binance --days 30

# 4. Rekonsiliasi dengan toleransi kustom dan format JSON (ramah CI/CD & Agent)
cargo run -- reconcile --symbol GSPC --source yahoo --tolerance 0.2 --days 60 --json

# 5. Padatkan partisi mikro-batch menjadi file konsolidasi bulanan
cargo run -- compact

# 6. Status danau data & integritas disk
cargo run -- status
```
