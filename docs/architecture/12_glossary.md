# 12. Glossary

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `329892f`
> - **Verified Timestamp**: `2026-10-30T15:33:05.959236+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

| Istilah | Penjelasan Sederhana Tanpa Jargon |
| :--- | :--- |
| **Candle / OHLCV** | Ringkasan pergerakan harga dalam kurun waktu tertentu, mencakup Open (buka), High (tertinggi), Low (terendah), Close (tutup), dan Volume (jumlah unit ditransaksikan). |
| **Tick** | Catatan satu transaksi tunggal yang baru saja terjadi di bursa (berisi harga, jumlah, dan waktu). |
| **Micro-Batching** | Menampung data sebentar di memori komputer sebelum ditulis sekaligus ke harddisk agar harddisk tidak lambat. |
| **Apache Parquet** | Format file khusus penyimpanan data tabel yang menyimpan informasi per kolom (bukan per baris), sehingga file sangat kecil dan sangat cepat dibaca. |
| **Hive Partitioning** | Cara mengatur file data ke dalam folder bertingkat (misal `tahun=2026/bulan=10/`) agar komputer tidak perlu mencari ke seluruh file saat mencari tanggal tertentu. |
| **Gap Healing** | Kemampuan sistem untuk secara otomatis mendeteksi dan mengambil kembali data yang sempat hilang saat koneksi internet terputus. |
| **Natural Key** | Gabungan informasi unik (seperti simbol + timeframe + waktu) yang menjamin tidak akan ada data ganda di dalam penyimpanan. |
| **Zero-Copy** | Teknik pemrosesan data di mana komputer langsung membaca data dari lokasi memori aslinya tanpa perlu menduplikat atau menyalin ulang data tersebut ke tempat lain. |
| **Settled vs Provisional** | Data *provisional* adalah data sementara hasil tangkapan langsung saat menit berjalan; data *settled* adalah data resmi final yang sudah diaudit oleh bursa setelah periode waktu selesai. |
| **Backpressure** | Mekanisme rem otomatis saat aliran data masuk lebih cepat daripada kemampuan komputer menyimpannya ke disk, mencegah memori RAM meluap habis. |
