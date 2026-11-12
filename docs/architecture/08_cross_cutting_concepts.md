# 8. Cross-Cutting Concepts

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `09e95c7`
> - **Verified Timestamp**: `2026-11-12T06:28:32+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 8.1 Pola Penanganan Error (Robust Error Handling)
- Semua fungsi yang dapat gagal mengembalikan tipe standar Rust `Result<T, AppError>`.
- Dilarang keras menggunakan `.unwrap()` atau `.expect()` pada alur data live untuk mencegah aplikasi mati mendadak (*panic*).
- Jika ada satu baris pesan bursa yang gagal diparsing, baris tersebut dicatat sebagai log peringatan, sementara aliran data tetap berjalan normal tanpa gangguan.

## 8.2 Logging & Observabilitas Berperforma Tinggi
- Menggunakan pustaka resmi Rust `tracing` dan `tracing-subscriber`.
- Format log terstruktur (*structured logging*) yang efisien tanpa alokasi string boros.
- Level log dapat diatur dinamis (`RUST_LOG=info` atau `RUST_LOG=debug`).

## 8.3 Keamanan Data & Pembersihan Input (Input Sanitization & Data Hygiene)
- **Pembersihan Simbol Ticker:** Karakter berbahaya pada simbol (seperti `..`, `/`, `\`) dibersihkan sebelum dijadikan nama folder partisi agar tidak terjadi *directory traversal*.
- **Sanitasi Ingesti Non-Destruktif (Bronze Tier):** Gerbang ingesti hanya menolak payload bursa yang rusak secara matematis (*NaN* atau *Infinity*) dan payload bursa malformed. Gerbang ingesti dilarang membuang transaksi pasar riil secara sepihak (termasuk lonjakan split 4:1 atau harga negatif komoditas). Seluruh rekaman mentah disimpan utuh apa adanya ke Parquet lakehouse.
- **Pembersihan Anomali Statistik Downstream (Hampel Filter & Rolling MAD):** Pada tahap konsumsi analitika downstream, pembersihan outlier runtun waktu dilakukan secara non-parametrik menggunakan formula:
  $$|x_t - M_t| > n \times 1.4826 \times \text{MAD}_t$$
  dengan sliding window simetris $2k + 1 = 15$ titik ($k=7$) dan ambang deviasi $n = 3.5\sigma$. Pendekatan ini menyaring anomali fat-finger tanpa merusak rekaman asli di danau data.
- **Pemisahan Tanggung Jawab Downstream (Downstream Decoupling):** Transformasi aksi korporasi (`CorporateActionAdjuster`), eliminasi look-ahead bias (`PointInTimeEngine`), dan perataan futures (`FuturesRollEngine`) dieksekusi secara non-destruktif di lapisan downstream sesuai kebutuhan pemodelan spesifik.
- **Kebijakan Zero-Synthetic Data:** Jalur analitika portofolio menolak kurva compounding sintetis konstan. Semua estimasi risiko wajib menggunakan data empiris historis riil.

## 8.4 Standar Pemanfaatan Data Downstream & Arsitektur Medallion
Data diatur dalam paradigma Medallion dengan kontrak ketat:
1. **Bronze Tier (Raw Lakehouse):**
   - Imutabel, *append-only*, apa adanya (*as-is*).
   - Dilarang memodifikasi harga pasar riil (termasuk komoditas berharga negatif atau stock split).
2. **Silver Tier (Cleaned & Harmonized Panel):**
   - Downstream wajib melakukan penyelarasan kalender (`TradingCalendarAligner`) dengan forward-fill deterministik sebelum menghitung kovariansi.
   - Pembersihan anomali statistik dilakukan via Hampel Filter non-parametrik (bukan menghapus baris di Bronze).
   - Valuasi multi-mata uang dinormalisasi ke mata uang basis (misal IDR) menggunakan kurs FX harian (`FxNormalizer`).
3. **Gold Tier (Analytical Features & Models):**
   - Bobot portofolio dinamis (HRP, Risk Parity) dan rebalancing berkala dieksekusi di atas panel Silver.

## 8.5 Standar Pemodelan Instrumen Konveks & Asimetris (Anti-Compounding Glitch)
Belajar dari evaluasi empiris simulasi portofolio, pemodelan instrumen derivatif/opsi tunduk pada invarian berikut:
- **Larangan Compounding Harian Otonom:** Instrumen opsi (*Long Volatility / Tail Risk*) dilarang keras dimodelkan sebagai runtun harga linier dengan persentase return harian yang di-*compound* terus-menerus ($R_t$).
- **Saturasi Delta & Volatility Crush:** Nilai opsi dibatasi oleh batas delta fisik ($\Delta \le 1.0$), lonjakan volatilitas wajib menyertakan peluruhan premi (*theta burn* harian), serta fenomena *volatility crush* pasca-rebound ekuitas.
- **Pencairan Proteksi (Asymmetric Cash Injection):** Alokasi tail-risk di dunia nyata berfungsi sebagai injeksi likuiditas tunai saat krisis, bukan instrumen yang di-reinvestasikan ke opsi baru dengan volume berlipat ganda.

## 8.6 Model Konkurensi & Manajemen Memori
- Menggunakan antrean terkendali `tokio::sync::mpsc::channel(50_000)` dengan kapasitas tetap (*bounded channel*).
- Jika disk mengalami perlambatan sesaat, antrean akan menahan laju penerimaan (*backpressure*) secara elegan daripada membiarkan RAM meluap kehabisan memori.
- Operasi tulis file Parquet yang bersifat sinkron dijalankan menggunakan `tokio::task::spawn_blocking` agar tidak memperlambat thread asinkron jaringan.

## 8.7 Standar CLI Ramah AI-Agent (Agent-First Envelope & Protocol)
- **Envelope JSON Standar:** Seluruh perintah mode `--json` mengembalikan payload baku:
  ```json
  {
    "success": true,
    "error": null,
    "data": { ... },
    "next_steps": ["economy-aggregator backfill status", "economy-aggregator lake stats"]
  }
  ```
- **Zero Interactivity:** Perintah mutasi tidak pernah memblokir `stdin`. Penggunaan otomatis wajib mendukung opsi `--yes`, `--force`, dan `--non-interactive`.
- **Kode Keluar Deterministik:** `0` (Sukses), `1` (Argumen/Konfigurasi Salah), `2` (Rate Limit/Network Error), `3` (Storage I/O Error), `4` (Integritas Data Gagal).

## 8.8 Manajemen Kunci Rahasia & Konfigurasi Bertingkat (Infisical & ENV Hierarchy)
- Mengadopsi hierarki resolusi 3-tier:
  1. *Command-line Flags* (Prioritas tertinggi, misal `--start-year 2020`).
  2. *Injeksi Rahasia Mesin* via Infisical Universal Auth (`infisical run -- dev`) dan Environment Variables (`AGGREGATOR_*`).
  3. *File Konfigurasi Deklaratif* (`economy-aggregator.toml`) untuk kemudahan setup "one config away".
- Token rahasia atau API key vendor tidak pernah disimpan ke git commit atau dicetak mentah di log terminal (wajib dimaskir: `sk_***`).

## 8.9 Rekonsiliasi Lilin Multi-Sumber & Audit Integritas Data (Candle Reconciliation & Drift Auditing)
- **Tujuan:** Memverifikasi secara matematis bahwa lilin pasar (OHLCV) yang tersimpan di Parquet lakehouse tetap valid, tidak mengalami revisi retroaktif liar oleh vendor, dan bebas dari celah waktu (*gaps*).
- **Prinsip Jendela Observasi Bersama (Mutual Observation Window):**
  Untuk mencegah penalti keliru (*false positive*) pada rentang historis di luar cakupan tarikan, komparasi dilakukan ketat pada irisan rentang waktu:
  $$[\max(T_{lake\_start}, T_{live\_start}), \min(T_{lake\_end}, T_{live\_end})]$$
- **Taksonomi Diskrepansi:**
  1. `PriceDrift`: Selisih harga penutupan $|\text{Close}_{lake} - \text{Close}_{live}| / \text{Close}_{lake}$ melebihi ambang toleransi $\epsilon$ (default $\pm 0.5\%$).
  2. `VolumeDrift`: Selisih volume antar bursa atau tipe transaksi.
  3. `MissingInLake`: Bar perdagangan ada pada live vendor tetapi hilang dari danau data dalam rentang observasi.
  4. `MissingInLive`: Bar perdagangan ada di danau data tetapi tidak disediakan oleh vendor live pada rentang kueri.
- **Dukungan Multi-Sumber:** Mengakomodasi format vendor eksternal bursa, termasuk Binance klines, Yahoo Finance daily bars, Dukascopy FX/CFD JSON (format array dan objek), serta TradingView UDF response format (`s: "ok"`, `t`, `o`, `h`, `l`, `c`, `v`).
- **Audit Non-Destruktif:** Pemeriksaan rekonsiliasi bersifat murni *read-only* tanpa pernah memutasi data Bronze mentah di disk.

## 8.10 Visual Candle Inspection Terminal & Batasan Deterministik 400 Baris
- **Tujuan:** Menyediakan antarmuka visual bawaan (*built-in zero-dependency UI*) yang ringan, berdensitas tinggi, dan dapat diakses instan oleh analis/operator pasar untuk memverifikasi bentuk lilin secara visual berdampingan dengan tes rekonsiliasi otomatis.
- **Invarian Desain Terminal (TradingView & Bloomberg Aesthetics):**
  1. *Zero Rounded Corners:* Seluruh elemen UI wajib menggunakan sudut siku tajam (`border-radius: 0px` strictly everywhere, `.rounded-none`).
  2. *Obsidian Dark Theme:* Palet warna gelap obsidian `#08090d` dengan kontras batas 1px (`#1f2430` / `#2d3446`).
  3. *Tipografi 100% Google Sans:* Seluruh teks dan angka menggunakan Google Sans dengan fitur OpenType tabular numbers (`font-variant-numeric: tabular-nums lining-nums; font-feature-settings: 'tnum', 'lnum'`).
  4. *Zero Emotikon / Emojis:* Dilarang keras menggunakan emoji di manapun pada antarmuka. Seluruh ikonografi menggunakan SVG vektor square/miter murni via `Icons.svelte`.
  5. *Interaksi Keyboard-First:* Pemilihan instrumen mendukung pencarian instan via modal keyboard shortcut (`/` atau `Ctrl+K`), navigasi panah, dan pemilih timeframe instan (`1m`, `5m`, `15m`, `1h`, `1d`, `1w`, `1M`).
- **Batasan Deterministik 400 Baris (Strict Line Limit):**
  Untuk mencegah *file bloat* dan pelanggaran *Single Responsibility Principle* (SRP), seluruh berkas kode baik frontend (`.svelte`, `.ts`, `.js`, `.css`) maupun backend (`.rs`) diwajibkan memiliki panjang maksimal $\le 400$ baris kode. Invarian ini diverifikasi secara deterministik oleh test suite otomatis `tests/test_sanity.rs` (`test_deterministic_file_line_limits_under_400`).

## 8.11 Big Data Serving Guardrails, In-Memory TTL Cache & HTTP Compression
- **Memory Guardrails (Capped Limits):**
  Untuk mematuhi budget memori $\le 150\text{ MB}$ dan mencegah alokasi berlebih pada serialisasi `serde_json`, seluruh endpoint data time-series (seperti `GET /v1/market/candles`) menerapkan batas maksimum keras:
  $$\text{capped\_limit} = \min(\text{query.limit}, 5000)$$
  Jika klien tidak mencantumkan parameter `limit`, sistem menggunakan nilai default 300 bar.
- **Partition Pruning & Short-Circuit Reading:**
  Query candles memotong folder partisi tahun/bulan sebelum membuka file (`effective_start_date` dari parameter `from` / `from_timestamp`). Ketika `start_date` tidak dispesifikasikan tetapi `limit` ada, reader menerapkan lazy short-circuit truncation:
  $$\text{max\_partitions\_needed} = \lfloor\text{limit} / 15\rfloor + 3$$
- **In-Memory TTL Caching (60s):**
  Untuk mencegah *disk thrashing* dan kelelahan I/O file descriptor pada ribuan file Parquet (seperti 4.100+ partisi), endpoint katalog (`GET /v1/market/catalog`) dan inspeksi partisi (`GET /v1/storage/partitions`) dilengkapi cache in-memory berbasis `RwLock` dengan TTL 60 detik.
- **HTTP Content-Encoding Compression:**
  Router Axum mengadopsi layer kompresi transparan `tower_http::compression::CompressionLayer` yang mendukung algoritma `gzip` dan `zstd` berdasarkan header request klien `Accept-Encoding`, menghemat bandwidth transmisi data besar hingga 70-85%.
- **Lakehouse Historical Archival Access:**
  Menyediakan akses offline-first langsung ke danau Parquet untuk kalender makro (`LakeMacroReader`) dan pasar prediksi (`LakePredictionReader`) dengan dukungan pencarian substring (`q`) dan filter status/tanggal.
