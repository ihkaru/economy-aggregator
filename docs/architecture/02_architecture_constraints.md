# 2. Architecture Constraints

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `329892f`
> - **Verified Timestamp**: `2026-10-30T15:33:05.959236+07:00`
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

## 2.2 Operational Constraints
- **Tanpa Dependency Server Tambahan:** Tidak membutuhkan database eksternal terpisah (seperti PostgreSQL, MySQL, Redis, atau Kafka). Seluruh penyimpanan berjalan lokal di folder file.
- **Portabilitas Sistem Operasi:** Berjalan sebagai satu file binary tunggal di Linux (termasuk Android/Termux aarch64), macOS, dan Windows.
- **Lisensi Dependensi:** Semua pustaka pihak ketiga wajib berlisensi bebas komersial (MIT, Apache-2.0, atau BSD). Dilarang keras menggunakan lisensi copyleft (GPL/AGPL).
