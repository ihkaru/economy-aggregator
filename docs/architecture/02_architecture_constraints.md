# 2. Architecture Constraints

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e773941`
> - **Verified Timestamp**: `2026-10-30T20:02:43+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 2.1 Technical Constraints & Performance Budgets

Agar sistem selalu cepat, tidak rakus RAM, dan tidak membebani prosesor mesin lokal, ditetapkan batasan teknis yang tegas:

| Kategori | Batasan / Budget | Alasan Teknis |
| :--- | :--- | :--- |
| **Bahasa & Runtime** | 100% Rust (Edisi 2021) | Menghilangkan beban runtime besar dan menghindari jeda pembersihan memori (*Garbage Collection pause*). |
| **Batas Memori RAM (Idle)** | $\le$ 50 MB | Aplikasi harus ramah untuk laptop atau server berspesifikasi hemat. |
| **Batas Memori RAM (Load)** | $\le$ 150 MB (saat 50.000 tick/detik) | Memori dibatasi dengan antrean terukur (*bounded buffer*), tidak boleh membengkak tanpa batas. |
| **Kapasitas Buffer Penampung** | Maksimal 50.000 record per buffer | Mencegah penumpukan data di RAM jika proses penulisan ke disk melambat. |
| **Alokasi Heap Hot-Path** | Zero-Allocation di jalur parsing tick | Menggunakan *borrowing* (`&str`) dan array tipe data tetap tanpa membuat alokasi string baru saat membaca WebSocket. |
| **Format File Disk** | Apache Parquet (`ZSTD` level 3) | Format tabel kolumnar terbuka; file terkompresi hemat tempat dan sangat cepat dibaca oleh Polars, DuckDB, dan Python. |
| **Ukuran Grup Baris (Row Group)**| 32.768 baris per blok Parquet | Ukuran seimbang agar pembacaan Parquet dapat memanfaatkan multi-core CPU secara maksimal. |
| **Pemisahan Thread CPU** | Thread Jaringan $\ne$ Thread Disk | Operasi penulisan disk yang berat dijalankan di thread terpisah (`spawn_blocking`) agar tidak mengganggu penerimaan data jaringan WebSocket. |
| **Standar Antarmuka CLI Agen**| Format Envelope JSON & Zero-Interactivity | Setiap perintah CLI wajib mendukung `--json`, kode keluar deterministik, dan bendera `--yes` / `--force` tanpa memblokir input terminal (`stdin`). |
| **Hierarki Kredensial & Rahasia** | Infisical Universal Auth & ENV | Kunci API vendor (Binance, FRED, Alpaca) tidak boleh disimpan di git repository; wajib diinjeksi via Infisical dev vault atau environment variable. |

## 2.2 Operational Constraints
- **Tanpa Dependency Server Tambahan:** Tidak membutuhkan database eksternal terpisah (seperti PostgreSQL, MySQL, Redis, atau Kafka). Seluruh penyimpanan berjalan lokal di folder file.
- **Portabilitas Sistem Operasi:** Berjalan sebagai satu file binary tunggal di Linux (termasuk Android/Termux aarch64), macOS, dan Windows.
- **Lisensi Dependensi:** Semua pustaka pihak ketiga wajib berlisensi bebas komersial (MIT, Apache-2.0, atau BSD). Dilarang keras menggunakan lisensi copyleft (GPL/AGPL).
- **One-Config-Away Bootstrap:** Seluruh pipeline (crypto, equities, commodities, macro) dapat langsung berjalan hanya dengan satu file konfigurasi deklaratif `economy-aggregator.toml`.

## 2.3 Quantitative & Backtest Data Integrity Constraints
- **Invarian Zero Synthetic Data:** Modul analitika dan simulasi portofolio dilarang keras merekayasa time-series menggunakan kurva compounding konstan (seperti formula yield eksponensial datar). Semua evaluasi wajib menggunakan 100% instrumen pasar empiris riil (seperti ETF Obligasi Total Return: TLT, IEF, LQD) atau yield observasi pasar nyata untuk mencegah distorsi nol volatilitas dan Sharpe ratio palsu.
- **Deteksi Anomali Non-Parametrik (No Hardcoded Price Bounds):** Filter pembersihan data pada jalur analitika dilarang keras menggunakan nilai batas harga statis atau nama ticker tertentu (misal `price < 5000.0` atau `symbol == "USD_IDR"`). Pembersihan wajib menggunakan metode statistik non-parametrik adaptif seperti Hampel Filter berbasis rolling Median Absolute Deviation (MAD).
- **Prinsip Imutabilitas Data Mentah (Raw Data Fidelity to Source):** Lapisan penyimpanan danau data (*lakehouse Bronze tier*) dilarang keras melakukan manipulasi *lossy*, pemotongan split, atau penghapusan harga anomali pada saat ingesti. Data mentah bursa wajib disimpan apa adanya; seluruh pembersihan dan penyesuaian wajib didelegasikan ke pengolah data hilir (*downstream consumption / Silver-Gold tier*).
