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
* Jika menguji strategi jangka panjang (seperti All-Weather atau Dividend Aristocrats), konversikan harga mentah ke **Total Return Index (TRI)** menggunakan `CorporateActionAdjuster` backward-adjustment.

---

### 5. Invarian Point-in-Time (Bitemporal Macro Data)
* Rilis data ekonomi makro (GDP, CPI, Payrolls) mengalami revisi berbulan-bulan setelah rilis awal.
* Membaca angka revisi final untuk keputusan perdagangan di masa lalu merupakan kesalahan fatal (*Look-Ahead Bias*).
* Gunakan `PointInTimeEngine` untuk menyaring data makro *strictly as-of* tanggal simulasi yang bersangkutan.

---

## 3. Toolkit Pengolahan Rust Bawaan

| Modul Pengolahan | Lokasi Kode Sumber | Fungsi Utama |
| :--- | :--- | :--- |
| `HampelFilter` | [`src/features/analytics/outlier.rs`](file:///root/projects/economy-aggregator/src/features/analytics/outlier.rs) | Deteksi outlier statistik non-parametrik adaptif. |
| `TradingCalendarAligner` | [`src/features/analytics/calendar_aligner.rs`](file:///root/projects/economy-aggregator/src/features/analytics/calendar_aligner.rs) | Sinkronisasi multi-bursa dengan deterministic forward-fill. |
| `FxNormalizer` | [`src/features/analytics/fx_normalizer.rs`](file:///root/projects/economy-aggregator/src/features/analytics/fx_normalizer.rs) | Normalisasi valuasi aset asing ke IDR. |
| `CorporateActionAdjuster` | [`src/features/analytics/corporate_actions.rs`](file:///root/projects/economy-aggregator/src/features/analytics/corporate_actions.rs) | Backward adjustment untuk split dan dividen tunai. |
| `PointInTimeEngine` | [`src/features/analytics/bitemporal.rs`](file:///root/projects/economy-aggregator/src/features/analytics/bitemporal.rs) | Kueri bitemporal untuk mencegah look-ahead bias makro. |
| `FuturesRollEngine` | [`src/features/analytics/futures_roll.rs`](file:///root/projects/economy-aggregator/src/features/analytics/futures_roll.rs) | Penyambungan kontrak berjangka via Panama Ratio & Additive Diff. |
| `PortfolioEngine` | [`src/features/analytics/portfolio.rs`](file:///root/projects/economy-aggregator/src/features/analytics/portfolio.rs) | Evaluasi CAGR, Volatilitas, Max Drawdown, Sharpe, & Korelasi. |

---

## 4. Contoh Pemanggilan Cepat dari CLI

```bash
# 1. Analisis performa multi-aset 100% data empiris riil
cargo run -- analyze --start-year 2004

# 2. Padatkan partisi mikro-batch menjadi file konsolidasi bulanan
cargo run -- compact

# 3. Status danau data & integritas disk
cargo run -- status
```
