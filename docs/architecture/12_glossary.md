# 12. Glossary

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e773941`
> - **Verified Timestamp**: `2026-10-30T20:02:43+07:00`
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
| **Agent-First CLI** | Antarmuka baris perintah yang dirancang untuk diurai secara deterministik oleh agen kecerdasan buatan (AI) menggunakan format envelope JSON tanpa input interaktif. |
| **Deep Backfill** | Proses penarikan data historis jangka panjang (sejak tahun 2020) secara terkelompok (*chunked pagination*) dengan checkpoint untuk mencegah pengunduhan ulang. |
| **One-Config-Away** | Paradigma di mana seluruh pipeline sistem dapat diaktifkan dan dikonfigurasi melalui satu file konfigurasi deklaratif tanpa setup manual berulang. |
| **Hampel Filter (Rolling MAD)** | Metode statistik non-parametrik adaptif berbasis Median Absolute Deviation (MAD) untuk menyaring anomali harga (glitch/fat-finger) tanpa asumsi distribusi normal dan tanpa hardcode threshold. |
| **Zero-Synthetic Data Mandate** | Invarian arsitektur yang melarang penggunaan kurva bunga majemuk konstan ($1.072^t$) atau runtun waktu fiktif dalam riset kuantitatif, mewajibkan penggunaan 100% instrumen pasar empiris riil. |
| **Forward-Fill Alignment** | Strategi penyelarasan tanggal perdagangan lintas bursa multi-negara di mana hari libur salah satu bursa diisi dengan harga penutupan terakhir yang diketahui (*forward-fill*, return harian = 0%). |
| **Bond Total Return ETF** | Reksa dana yang diperdagangkan di bursa (seperti TLT, IEF, LQD) yang merefleksikan harga pasar obligasi riil termasuk reinvestasi kupon dan sensitivitas durasi-konveksitas terhadap perubahan suku bunga acuan. |
| **CAGR & Max Drawdown** | *Compound Annual Growth Rate* (laju pertumbuhan tahunan majemuk) dan *Maximum Drawdown* (penurunan nilai puncak-ke-lembah terdalam) sebagai metrik standar valuasi risiko portofolio. |
| **Sharpe Ratio** | Ukuran imbal hasil yang disesuaikan dengan risiko (*risk-adjusted return*), dihitung dari kelebihan return di atas risk-free rate dibagi dengan volatilitas tahunan. |
| **Raw Data Fidelity (Bronze Layer)** | Prinsip bahwa data lakehouse harus menyimpan data pasar mentah apa adanya tanpa filter destruktif, menjaga jejak sejarah riil seperti harga negatif, split, atau lonjakan ekstrem. |
| **Medallion Architecture (Bronze-Silver-Gold)** | Pemisahan lapisan data antara Bronze (data mentah immutabel), Silver (data terbersihkan/terstandarisasi/adjusted di memori/view), dan Gold (fitur model & metrik analitika agregasi). |
| **Corporate Actions Adjustment** | Penyesuaian matematis runtun harga historis ke belakang (*backward adjustment*) untuk menghilangkan lompatan artifisial akibat pemecahan saham atau pembagian dividen tunai. |
| **Point-in-Time (Bi-temporal Macro)** | Model waktu ganda (*Observation Date* vs *Knowledge Release Date*) yang mencegah Look-Ahead Bias pada rilis makroekonomi dengan mengunci pembacaan data strictly as-of tanggal simulasi. |
| **Futures Roll Smoothing** | Rekonsiliasi perbedaan harga antar-kontrak berjangka (Contango/Backwardation) saat pergantian bulan kontrak menggunakan metode rasio Panama atau selisih kumulatif. |
