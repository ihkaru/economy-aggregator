# 10. Quality Requirements

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `936d34c`
> - **Verified Timestamp**: `2026-10-30T20:19:40+07:00`
> - **Tracked Paths**: `src/`, `Cargo.toml`
> - **Drift Status**: `verified`

## 10.1 Quality Tree
Kebutuhan kualitas sistem dipetakan langsung ke skenario pengujian yang dapat diverifikasi secara deterministik:

```mermaid
graph LR
    Q[Kebutuhan Mutu Utama]
    Q --> P[#performance: Kecepatan & Latensi]
    Q --> E[#efficiency: Hemat RAM & CPU]
    Q --> R[#reliability: Keandalan Data]
    Q --> M[#maintainability: Kemudahan Perawatan]
    Q --> A[#agent_readiness: Otomasi Agen AI]
    Q --> F[#fidelity: Integritas Data & Zero Synthetic]

    P --> P1["QS-01: Latensi parsing tick < 100 µs"]
    P --> P2["QS-02: Waktu query REST candle 1 bulan < 10 ms"]
    E --> E1["QS-03: Konsumsi RAM idle <= 50 MB"]
    E --> E2["QS-04: Konsumsi RAM beban tinggi <= 150 MB"]
    R --> R1["QS-05: Penanganan recon & deduplikasi 100% akurat"]
    M --> M1["QS-06: 100% lulus static lint & zero memory leak"]
    A --> A1["QS-07: Output CLI 100% valid JSON envelope & exit code deterministik"]
    F --> F1["QS-08: 100% instrumen empiris riil & filter anomali non-parametrik"]
    F --> F2["QS-09: Preservasi data mentah as-is (Bronze tier immutability)"]
```

## 10.2 Quality Scenarios

| Skenario | Kategori Mutu | Stimulus / Kondisi Uji | Respons & Toleransi Terukur |
| :--- | :--- | :--- | :--- |
| **QS-01** | #performance | 50.000 pesan tick masuk secara bersamaan melalui WebSocket. | Sistem memvalidasi dan memproses tiap tick dalam waktu rata-rata di bawah 100 mikrodetik. |
| **QS-02** | #performance | Klien meminta 43.200 baris candle (1 bulan data 1-menit) via REST API. | Endpoint memberikan response lengkap dalam waktu di bawah 10 milidetik berkat pemangkasan partisi folder. |
| **QS-03** | #efficiency | Sistem berjalan tanpa beban transaksi aktif selama 24 jam. | Total memori RAM yang digunakan tidak melebihi 50 MB secara konsisten. |
| **QS-04** | #efficiency | Sistem menerima arus deras 50.000 tick/detik secara berkelanjutan. | Kapasitas antrean memori tertahan di angka aman, total RAM tidak melebihi 150 MB. |
| **QS-05** | #reliability | Koneksi WebSocket terputus selama 60 detik lalu tersambung kembali. | Detektor gap mendeteksi data yang hilang, mengunduh via REST, dan menyatukannya tanpa duplikasi record. |
| **QS-06** | #maintainability | Menjalankan seluruh test suite dan linter otomatis Rust (`cargo clippy`, `cargo test`). | 100% test lulus, nol peringatan keamanan (*zero warnings*), dan bebas dari kebocoran memori. |
| **QS-07** | #agent_readiness | Perintah CLI dijalankan via subprocess oleh Agen AI dengan parameter `--json`. | Output 100% parseable JSON envelope tanpa kode escape ANSI dan exit code akurat sesuai status eksekusi. |
| **QS-08** | #fidelity | Menjalankan evaluasi portofolio multi-dekade via `economy-aggregator analyze`. | 100% data obligasi menggunakan ETF empiris pasar riil (TLT, IEF, LQD), seluruh glitch harga disaring via Hampel Filter (Rolling MAD) pada downstream analytics tanpa hardcode threshold. |
| **QS-09** | #fidelity | Ingesti data pasar mentah (termasuk harga negatif, lonjakan ekstrem, split saham) ke data lake. | 100% data mentah tersimpan apa adanya (*as-is*) di Bronze layer tanpa mutasi atau dropping prematur; seluruh transformasi dijalankan downstream. |
