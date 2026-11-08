# 9. Architecture Decisions

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `794e20f`
> - **Verified Timestamp**: `2026-11-08T14:42:00+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 9.1 ADR Index

| ADR ID | Judul Keputusan | Status | Tanggal |
| :--- | :--- | :--- | :--- |
| **ADR-01** | Transisi Penuh ke Bahasa Rust (Zero-Garbage Collector) | Diterima | 2026-10-30 |
| **ADR-02** | Penggunaan Format File Apache Parquet Terkompresi ZSTD | Diterima | 2026-10-30 |
| **ADR-03** | Penerapan Model Ingesti Hybrid (Live WebSocket + REST Fallback) | Diterima | 2026-10-30 |
| **ADR-04** | Antarmuka CLI Ramah Agen AI dengan Format Envelope JSON | Diterima | 2026-10-30 |
| **ADR-05** | Arsitektur Penyelarasan Kalender Multi-Bursa & Normalisasi Valuasi FX | Diterima | 2026-10-30 |
| **ADR-06** | Mesin Analitika Portofolio & Risiko Vektorisasi Berbasis Rust Asli | Diterima | 2026-10-30 |
| **ADR-07** | Kebijakan Zero-Synthetic Data dan Deteksi Anomali Statistik Non-Hardcode | Diterima | 2026-10-30 |
| **ADR-08** | Mitigasi Struktural Multi-Aset (Corporate Actions, Bitemporal PIT Macro, Futures Roll) | Diterima | 2026-10-30 |
| **ADR-09** | Prinsip Preservasi Data Mentah (Raw Data Fidelity) & Pemisahan Downstream | Diterima | 2026-10-30 |
| **ADR-10** | Standar Pemodelan Derivatif Non-Linier & Pencegahan Bug Compounding | Diterima | 2026-10-30 |
| **ADR-11** | Pipeline Transformasi Silver Terpadu & Engine Alokasi Rust Native | Diterima | 2026-10-30 |
| **ADR-12** | Symbology Temporal (Point-in-Time Alias) & Penyesuaian Aksi Korporasi Volume | Diterima | 2026-10-31 |
| **ADR-13** | Rekonsiliasi Lilin Multi-Sumber & Audit Integritas Data Lakehouse | Diterima | 2026-11-08 |
| **ADR-14** | Built-in High-Density Svelte Candle Inspection Terminal & Batasan Deterministik 400 Baris | Diterima | 2026-11-08 |

---

### ADR-01: Transisi Penuh ke Bahasa Rust (Zero-Garbage Collector)
* **Status:** Diterima
* **Konteks:**  
  Awalnya sistem dipertimbangkan memakai Python. Namun untuk data aggregator berkecepatan tinggi yang menerima puluhan ribu tick per detik via WebSocket, runtime Python akan boros RAM (setiap angka float memakan 28 byte vs 8 byte di Rust), terhambat oleh GIL (Global Interpreter Lock), dan memicu jeda pembersihan memori (*Garbage Collector pauses*).
* **Keputusan:**  
  Membangun seluruh inti agregator menggunakan **Rust** dengan runtime asinkron `tokio` dan framework web `axum`.
* **Konsekuensi Positif:**  
  Konsumsi RAM sangat stabil di bawah 50 MB, latensi sub-milidetik yang konsisten, dan file executable tunggal yang mudah dijalankan di mana saja.
* **Risiko yang Muncul:**  
  `RISK-01` (Waktu kompilasi awal Rust lebih lama dibanding bahasa skrip).

---

### ADR-02: Penggunaan Format File Apache Parquet Terkompresi ZSTD
* **Status:** Diterima
* **Konteks:**  
  Menyimpan jutaan data transaksi di database relasional tradisional (seperti SQLite/Postgres) memakan banyak disk dan lambat saat memindai rentang tanggal panjang.
* **Keputusan:**  
  Menggunakan format kolumnar terbuka **Apache Parquet** dengan kompresi `ZSTD` level 3 dan partisi gaya Hive.
* **Konsekuensi Positif:**  
  Ukuran file 70% lebih kecil dibanding CSV atau database row-based, serta sangat cepat dibaca oleh mesin analitik modern seperti Polars dan DuckDB.
* **Risiko yang Muncul:**  
  `RISK-02` (Potensi penumpukan file kecil jika interval flush terlalu sering tanpa proses penggabungan).

---

### ADR-03: Penerapan Model Ingesti Hybrid (Live WebSocket + REST Fallback)
* **Status:** Diterima
* **Konteks:**  
  Koneksi WebSocket real-time bisa putus sewaktu-waktu akibat gangguan jaringan, menyebabkan data bolong (*stream gaps*).
* **Keputusan:**  
  Menerapkan mekanisme ganda: data diterima via WebSocket secara live, namun jika terdeteksi nomor urut melompat atau koneksi putus, sistem otomatis memanggil REST API untuk menambal data yang terlewat.
* **Konsekuensi Positif:**  
  Data selalu lengkap dan akurat tanpa perlu intervensi manual dari pengguna.
* **Risiko yang Muncul:**  
  `RISK-03` (Batas kuota panggilan REST API / rate limit dari penyedia data eksternal).

---

### ADR-04: Antarmuka CLI Ramah Agen AI dengan Format Envelope JSON dan Zero Interactivity
* **Status:** Diterima
* **Konteks:**  
  Sistem perlu dioperasikan tidak hanya oleh manusia via terminal interaktif, tetapi juga oleh Agen AI otonom (*Autonomous AI Agents*) secara headless. Prompt interaktif (seperti `[y/N]`) dan output tidak terstruktur menyebabkan agen macet (*hang indefinitely*) atau salah tafsir hasil.
* **Keputusan:**  
  Membangun CLI dengan format output envelope terstandarisasi (`{"success": true, "data": {...}, "error": null, "next_steps": [...]}`), flag non-interaktif (`--yes`, `--force`), kode keluar numerik terklasifikasi, dan dukungan *dry-run*.
* **Konsekuensi Positif:**  
  Agen AI dapat memanggil, memverifikasi kesehatan, mengonfigurasi parameter, dan memicu deep backfill sejak 2020 dengan keandalan 100%.
* **Risiko yang Muncul:**  
  `RISK-04` (Potensi pembatasan laju panggilan / rate limit saat penarikan data historis masif sejak tahun 2020).

---

### ADR-05: Arsitektur Penyelarasan Kalender Multi-Bursa & Normalisasi Valuasi FX
* **Status:** Diterima
* **Konteks:**  
  Penyusunan portofolio multi-aset (All-Weather) mencakup bursa dengan hari libur berbeda (NYSE, IDX, TSE, komoditas, dan kripto 24/7) serta denominasi mata uang beragam (USD, IDR). Ketidakselarasan tanggal dan kurs menyebabkan data missing gaps saat outer join dan bias return riil daya beli investor IDR.
* **Keputusan:**  
  Membangun modul `TradingCalendarAligner` dengan interpolasi deterministik Forward-Fill untuk menyamakan spine tanggal perdagangan, dan `FxNormalizer` untuk otomatis mengalikan atau membagi harga aset asing dengan time-series kurs FX acuan.
* **Konsekuensi Positif:**  
  Dataset panel multi-aset selalu kontinu tanpa nilai kosong (*zero-NaN gap*), dan return aset luar negeri terukur secara adil dalam mata uang basis investor.
* **Risiko yang Muncul:**  
  `RISK-06` (Asinkronitas kalender bursa) dan `RISK-07` (Distorsi nilai tukar).

---

### ADR-06: Mesin Analitika Portofolio & Risiko Vektorisasi Berbasis Rust Asli
* **Status:** Diterima
* **Konteks:**  
  Sebelumnya, pengujian hipotesis portofolio All-Weather dan kalkulasi matriks korelasi cross-asset bergantung pada skrip eksternal (Python dengan pustaka Polars/PyArrow). Hal ini menambah ketergantungan runtime luar dan memperlambat alur kerja otomatisasi agen AI.
* **Keputusan:**  
  Membangun modul analitika kuantitatif internal murni di Rust (`src/features/analytics/`) yang mencakup `PortfolioEngine`, `SyntheticBondAccumulator`, dan sub-perintah CLI `economy-aggregator analyze`.
* **Konsekuensi Positif:**  
  Kalkulasi CAGR, volatilitas tahunan, max drawdown, Sharpe ratio, dan matriks korelasi Pearson dapat dieksekusi secara instan (<2 detik) langsung dari Parquet lakehouse tanpa membutuhkan runtime Python eksternal.
* **Risiko yang Muncul:**  
  `DEBT-02` (Hutang analitik diselesaikan dan diverifikasi penuh dengan 100% tes otomatis).

---

### ADR-07: Kebijakan Zero-Synthetic Data dan Deteksi Anomali Statistik Non-Hardcode
* **Status:** Diterima
* **Konteks:**  
  Pada iterasi awal, kelas aset obligasi dimodelkan menggunakan akumulasi yield sintetis konstan ($1.072^t$). Ini menyebabkan bias artifisial berat karena runtun waktu obligasi memiliki volatilitas nol dan drawdown nol, yang secara palsu melipatgandakan Sharpe ratio portofolio. Selain itu, pembersihan anomali harga pada rute ingesti sempat menggunakan aturan hardcode ad-hoc (seperti `price < 5000.0` atau pemeriksaan string `USD_IDR`). Hal ini melanggar prinsip kebersihan kode dan tidak tahan uji jangka panjang.
* **Keputusan:**  
  1. Menetapkan **Invarian Zero-Synthetic Data** yang melarang keras penggunaan kurva yield konstan atau data fiktif dalam riset dan backtesting portofolio. Portofolio All-Weather wajib divalidasi dengan 100% instrumen pasar empiris riil (seperti ETF Obligasi Total Return: TLT, IEF, LQD, BND).
  2. Menggantikan seluruh ambang batas hardcoded dengan **Hampel Filter (Rolling Median Absolute Deviation / MAD)** non-parametrik adaptif ($k=7, 3.5\sigma$) yang bekerja secara agnostik terhadap seluruh kelas aset.
  3. Mengganti injeksi bobot prosedural dengan spesifikasi konfigurasi deklaratif `PortfolioSpec`.
* **Konsekuensi Positif:**  
  Hasil backtest memiliki integritas kuantitatif tinggi setara standar hedge fund institusional tanpa distorsi volatilitas palsu. Algoritma pembersihan tahan terhadap regresi dan tidak memerlukan tambal-sulam saat menambahkan instrumen baru.
* **Risiko yang Muncul:**  
  `RISK-08` (Diperlukannya pengunduhan data pasar empiris riil untuk instrumen ETF obligasi sebelum backtest dapat dieksekusi).

---

### ADR-08: Mitigasi Struktural Multi-Aset (Corporate Actions, Bitemporal PIT Macro, Futures Roll)
* **Status:** Diterima
* **Konteks:**  
  Setiap kelas aset keuangan memiliki anomali struktural unik:
  1. *Equities*: Stock split dan dividen tunai menimbulkan lonjakan/penurunan harga artifisial semalam (-75% pada split 1:4).
  2. *Macro*: Revisi statistik berkala (GDP, CPI, NFP) menimbulkan *Look-Ahead Bias* berbahaya jika backtest membaca angka final yang belum ada saat kejadian.
  3. *Commodities & Forex*: Pergantian kontrak berjangka (*futures roll*) menimbulkan lonjakan harga semu akibat contango/backwardation, dan sesi Minggu malam pada Valas 24/5 menimbulkan artefak missing dates.
* **Keputusan:**  
  1. Mengimplementasikan `CorporateActionAdjuster` untuk menghitung faktor penyesuaian kumulatif ke belakang (*backward adjustment*) pada saham dan ETF.
  2. Mengimplementasikan `PointInTimeEngine` dengan model bitemporal untuk mengunci kueri data makro strictly `as_of` tanggal simulasi.
  3. Mengimplementasikan `FuturesRollEngine` (Panama Ratio & Additive Difference) dan `ForexSessionNormalizer` untuk menyelaraskan continuous series komoditas dan grid 5D valas.
* **Konsekuensi Positif:**  
  Mencegah bias metodologis fatal dalam riset kuantitatif, backtesting portofolio, dan pemrosesan sinyal otomatis.
* **Risiko yang Muncul:**  
  `RISK-09`, `RISK-10`, `RISK-11` (Kebutuhan metadata historis corporate actions dan log rilis vintage makro yang lengkap).

---

### ADR-09: Prinsip Preservasi Data Mentah (Raw Data Fidelity) dan Pemisahan Tanggung Jawab Downstream
* **Status:** Diterima
* **Konteks:**  
  Sebelumnya sempat muncul kecenderungan untuk membuang harga anomali atau lonjakan harga secara langsung pada pintu gerbang ingesti (`yahoo_finance.rs` sempat menolak candle jika `is_price_outlier` bernilai true). Ini adalah anti-pattern yang berbahaya: pemecahan saham sah (misal 1:4) atau pergerakan harga komoditas negatif (WTI -$37/bbl pada 2020) berisiko terhapus permanen dari danau data, merusak integritas *ground truth* bursa.
* **Keputusan:**  
  1. Menetapkan **Prinsip Imutabilitas Data Mentah (Bronze Tier)**: Lapisan ingesti dan penyimpanan Parquet dilarang keras melakukan manipulasi *lossy*, pemotongan split, atau penghapusan harga anomali. Seluruh data mentah disimpan apa adanya ke disk.
  2. Menetapkan **Pemisahan Tanggung Jawab Downstream (Silver/Gold Tier)**: Seluruh pembersihan anomali statistik (*Hampel filter*), penyesuaian aksi korporasi (*Corporate Actions*), mitigasi *look-ahead bias* (*Point-in-Time*), dan perataan *futures roll* didelegasikan sebagai modul pengolahan downstream yang dieksekusi secara non-destruktif saat data dikonsumsi.
* **Konsekuensi Positif:**  
  Danau data selalu menyimpan rekaman historis yang utuh dan dapat diaudit ulang (*re-playable*). Berbagai jenis konsumen hilir bebas menerapkan metode kuantitatif yang paling sesuai dengan strategi masing-masing tanpa kehilangan data asli.
* **Risiko yang Muncul:**  
  `RISK-12` (Konsumen hilir wajib menyadari karakteristik data mentah dan secara eksplisit menerapkan pipeline pembersihan downstream).

---

### ADR-10: Standar Pemodelan Derivatif Non-Linier dan Pencegahan Bug Compounding Opsi
* **Status:** Diterima
* **Konteks:**  
  Pada pengujian portofolio tingkat lanjut (seperti Dragon Portfolio dan Cockroach Portfolio yang melibatkan alokasi *Long Volatility / Tail Risk*), pemodelan simplifikasi deret waktu linier memicu bug kritis: aset opsi menghasilkan return harian $+5\% \text{ s/d } +15\%$ terus-menerus selama pasar krisis ($drawdown > 15\%$), terakumulasi menjadi return fiktif $10^{32}$ dan return portofolio $46\%$ CAGR. Di pasar riil, opsi tunduk pada *theta decay* (-7% s/d -15% per tahun), pembatasan delta fisik ($\Delta \le 1.0$), dan kejatuhan premi (*volatility crush*) pasca-rebound.
* **Keputusan:**  
  1. Menetapkan standar analitika ketat: instrumen derivatif konveks dilarang keras dimodelkan sebagai runtun persentase return harian otonom yang di-*compound* tanpa batas.
  2. Mewajibkan kalibrasi ke indeks tolok ukur riil (seperti CBOE Eurekahedge Long Volatility Index `EHFI451`) atau model proteksi terikat (anggaran tetap tahunan $\approx 1-2\%$ yang hangus saat tenang dan dicairkan sebagai likuiditas tunai saat krisis).
* **Konsekuensi Positif:**  
  Menghilangkan ilusi performa "terlalu indah untuk menjadi kenyataan", menghasilkan estimasi risiko-imbal hasil yang objektif, akurat, dan sesuai dengan literatur keuangan institusional.
* **Risiko yang Muncul:**  
  `RISK-13` (Kebutuhan kalibrasi parameter volatilitas yang ketat berdasarkan data bursa derivatif).

---

### ADR-11: Pipeline Transformasi Silver Terpadu dan Engine Alokasi Rust Native
* **Status:** Diterima
* **Konteks:**  
  Saat ini terdapat kekosongan fungsional antara layer Bronze (file-file Parquet mentah) dan kebutuhan pemodelan portofolio tingkat lanjut (HRP, Threshold Rebalancing). Konsumen terpaksa menggunakan skrip Python eksternal ad-hoc yang lambat dan rentan bug pemodelan.
* **Keputusan:**  
  1. Membangun sub-perintah CLI native `economy-aggregator build-silver` untuk menghasilkan panel Parquet terpadu (sudah *calendar-aligned*, *Hampel-cleaned*, dan *FX-normalized*).
  2. Mengimplementasikan algoritma pembelajaran mesin *Hierarchical Risk Parity (HRP)* dan modul *Threshold Rebalancing* langsung di Rust (`src/features/analytics/`).
* **Konsekuensi Positif:**  
  Menghapus total ketergantungan runtime Python, mempercepat eksekusi analitika hingga 100x lipat di lingkungan Linux/ARM64, dan menjamin kepatuhan tipe kompilasi (*compile-time safety*).
* **Risiko yang Muncul:**  
  `RISK-14` (Kompleksitas algoritma pengelompokan hierarkis matriks jarak di Rust).

---

### ADR-12: Symbology Temporal (Point-in-Time Alias) dan Penyesuaian Aksi Korporasi Volume-Aware
* **Status:** Diterima
* **Konteks:**  
  Ticker saham di pasar keuangan bersifat fana (*transient*): perusahaan dapat melakukan rebranding (seperti `FB` $\to$ `META`), restrukturisasi merger, atau penggantian simbol pasca-reverse split. Pemetaan ticker statis menyebabkan data historis sebelum perubahan simbol terputus (*data fragmentation*) atau bias kesintasan (*survivorship bias*). Selain itu, pemecahan saham (*stock split*) tidak hanya membagi harga, tetapi juga mengalikan jumlah lembar saham yang beredar, sehingga penyesuaian volume historis ($V / \text{split\_factor}$) wajib dilakukan agar omzet perputaran (*turnover*) tetap konsisten.
* **Keputusan:**  
  1. Memperluas skema `InstrumentMapping` dengan dukungan identitas permanen (ISIN, OpenFIGI, PermID) dan daftar alias temporal (`TickerAlias { raw_symbol, vendor, valid_from, valid_to }`).
  2. Mengimplementasikan `InstrumentRegistry::resolve_at()` untuk resolusi simbol berbasis tanggal (*Point-in-Time Symbology*).
  3. Memperluas `CorporateActionAdjuster` dengan metode `adjust_ohlcv_backward()` yang secara otomatis menyesuaikan harga ($P \times \text{factor}$) sekaligus volume ($V / \text{split\_factor}$).
  4. Mengotomatisasi penarikan riwayat split dan dividen langsung dari Yahoo Finance via `YahooFinanceFetcher::fetch_daily_bars_with_events()`.
* **Konsekuensi Positif:**  
  Data historis emiten sebelum dan sesudah rebranding tertaut sempurna ke satu identitas kanonikal, dan analisis volume/likuiditas pra-split mencerminkan nilai ekuivalen pasca-split secara deterministik.
* **Risiko yang Muncul:**  
  `RISK-15` (Kebutuhan pemeliharaan tabel symbology master yang terus diperbarui seiring pengumuman aksi korporasi bursa).

---

### ADR-13: Rekonsiliasi Lilin Multi-Sumber dan Audit Integritas Data Lakehouse
* **Status:** Diterima
* **Konteks:**  
  Data deret lilin (OHLCV) yang telah tersimpan di Parquet lakehouse berisiko mengalami drift atau inkonsistensi terhadap data pasar langsung akibat:
  1. Revisi retroaktif vendor (misal Yahoo Finance memperbarui faktor dividen/split masa lalu secara diam-diam).
  2. Perbedaan konvensi bursa dan vendor (misal Dukascopy FX BID/ASK mid price vs Yahoo Last traded price vs Binance crypto exchange execution).
  3. Celah data (*gaps*) atau keterlambatan penarikan batch harian.
  4. Kerusakan atau korupsi file Parquet akibat proses batch yang terputus di tengah jalan.  
  Diperlukan mekanisme komparasi otomatis yang membandingkan lilin tersimpan di Parquet lakehouse dengan tarikan langsung (*live fetch*) maupun format vendor eksternal lainnya (Dukascopy, TradingView).
* **Keputusan:**  
  1. Membangun modul domain `ReconciliationEngine` di [`src/features/analytics/reconciliation.rs`](file:///root/projects/economy-aggregator/src/features/analytics/reconciliation.rs) yang mengomparasi candle-by-candle pada irisan jendela observasi bersama $[\max(T_1, T_3), \min(T_2, T_4)]$.
  2. Mengklasifikasikan diskrepansi secara deterministik: `PriceDrift` ($> \text{tolerance}$, default $0.5\%$), `VolumeDrift`, `MissingInLake`, dan `MissingInLive`.
  3. Menambahkan parser multi-sumber vendor: `DukascopyParser` (format array-of-arrays dan array-of-objects) dan `TradingViewParser` (spesifikasi UDF `/history`).
  4. Menyediakan pembaca Parquet danau data terpadu `LakeReader::read_candles()` yang otomatis membaca partisi, mendekompresi Arrow batch, dan mendeduplikasi data dengan prioritas *settled records*.
  5. Menambahkan sub-perintah CLI: `economy-aggregator reconcile --symbol <SYMBOL> --source <SOURCE> [--tolerance <PCT>] [--days <N>] [--json]`.
  6. Menjadikan mekanisme ini sebagai test suite otomatis (`tests/test_lake_reconciliation.rs`) dengan 8 pengujian komprehensif termasuk pengujian live lakehouse riil.
* **Konsekuensi Positif:**  
  Integritas lakehouse dapat diaudit secara berkala tanpa menyentuh atau memutasi data Bronze asli, memberikan bukti empiris bahwa data yang tersimpan valid, bebas revisi liar, dan terdeteksi dini jika ada celah data.
* **Risiko yang Muncul:**  
  `RISK-16` (Perbedaan konvensi jam penutupan bursa / timezone antar vendor yang dapat menimbulkan selisih mikro toleransi harga).

---

### ADR-14: Built-in High-Density Svelte Candle Inspection Terminal & Batasan Deterministik 400 Baris
* **Status:** Diterima
* **Konteks:**  
  Meskipun engine rekonsiliasi CLI dan test suite otomatis telah menjamin akurasi numerik candle lakehouse, analis dan operator tetap membutuhkan sarana konfirmasi visual instan untuk mengamati formasi lilin, wick, anomali volume, dan pola data tanpa harus membuka platform trading pihak ketiga yang berat atau berpotensi terikat lisensi komersial.
  Kebutuhan spesifik:
  1. Antarmuka bawaan (*built-in*) langsung dilayani oleh backend Rust Axum (`GET /`).
  2. Charting berkinerja tinggi berbasis `lightweight-charts`.
  3. Gaya antarmuka ala TradingView / Bloomberg Terminal dengan densitas tinggi (*high density*, obsidian dark `#08090d`).
  4. Aturan tanpa sudut melengkung (*zero rounded corners*, `border-radius: 0px` strictly everywhere).
  5. Tipografi 100% Google Sans (termasuk angka tabular `tnum`/`lnum`).
  6. Nol emotikon/emoji (*zero emoticons*), digantikan oleh vektor SVG miter/square monokromatik.
  7. Implementasi murni Svelte (`web/`).
  8. Batasan deterministik ketat: maksimal 400 baris per file di seluruh codebase frontend maupun backend.
* **Keputusan:**  
  1. Membangun modul frontend di `web/` menggunakan Svelte 4 dan Vite 5, membungkus TradingView `lightweight-charts` dengan histogram volume terintegrasi, legend pelacak crosshair interaktif, pemilih instrumen/ticker modal keyboard-first (`/` atau `Ctrl+K`), dan pemilih timeframe (`1m`, `5m`, `15m`, `1h`, `1d`, `1w`, `1M`).
  2. Menyediakan endpoint katalog `/v1/market/catalog` di backend Axum dan melayani bundel statis `web/dist` melalui `tower_http::services::ServeDir`.
  3. Mengadopsi `Icons.svelte` berbasis SVG terminal miter/square untuk meniadakan ketergantungan bundle eksternal dan menjamin tampilan bebas rounded corner.
  4. Menerapkan pengujian deterministik di `tests/test_sanity.rs` (`test_deterministic_file_line_limits_under_400`) yang secara rekursif memverifikasi bahwa tidak ada satupun file `.rs`, `.svelte`, `.ts`, `.js`, atau `.css` yang melampaui 400 baris kode.
* **Konsekuensi Positif:**  
  Operator mendapatkan visualisasi real-time yang tajam, sangat responsif, dan zero-friction langsung dari executable tunggal `economy-aggregator serve`, sembari menjaga modularitas kode tetap ramping, terdekomposisi rapi, dan mudah diaudit.
* **Risiko yang Muncul:**  
  `RISK-17` (Kebutuhan build Vite `bun run build` jika terjadi perubahan komponen UI di direktori `web/src/`).
