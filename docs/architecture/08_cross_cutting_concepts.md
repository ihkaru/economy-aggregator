# 8. Cross-Cutting Concepts

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `f1daafe`
> - **Verified Timestamp**: `2026-10-30T20:29:28+07:00`
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
