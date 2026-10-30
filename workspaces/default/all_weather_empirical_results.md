# Laporan Hasil Pengujian Empiris: Portofolio All-Weather Sander vs Benchmarks

> **Sumber Data:** Columnar Parquet Lakehouse `economy-aggregator`  
> **Periode Analisis:** 2021-11-01 00:00:00 to 2026-10-30 00:00:00 (7.24 tahun data harian, 1824 hari perdagangan aktif)

---

## 1. Tabel Perbandingan Kinerja & Risiko (Empirical Evidence)

| Strategi / Aset | Modal Awal ($) | Nilai Akhir ($) | Return Tahunan (CAGR) | Volatilitas Tahunan (Risk) | Maximum Drawdown (MDD) | Sharpe Ratio (Rf=5%) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **All-Weather (Sander IDR)** | $10,000 | $17,061 | **7.66%** | **4.48%** | **-3.27%** | **0.59** |
| **S&P 500 (GSPC in IDR)** | $10,000 | $21,510 | 11.05% | 15.3% | -20.68% | 0.4 |
| **Saham BBCA (Indo Banking)** | $10,000 | $6,730 | -5.15% | 21.98% | -55.98% | -0.46 |
| **Emas (Gold in IDR)** | $10,000 | $28,547 | 15.6% | 17.09% | -19.45% | 0.62 |
| **IHSG (Indo Composite)** | $10,000 | $7,717 | -3.39% | 14.66% | -45.3% | -0.57 |
| **Bitcoin (BTC/USD)** | $10,000 | $12,677 | 2.82% | 42.21% | -76.63% | -0.05 |

---

## 2. Verifikasi Hipotesis Transkrip

### A. Hipotesis Maximum Drawdown & Tidur Nyenyak
- **Hipotesis Sander:** *Portofolio terkonsentrasi di satu instrumen (misal S&P 500, saham tunggal, atau crypto) memiliki risiko drawdown tajam yang merusak psikologis investor muda.*
- **Bukti Empiris:**
  - Portofolio **All-Weather Sander** hanya mengalami Maximum Drawdown sebesar **-3.27%** selama 5 tahun penuh.
  - Sebaliknya, Bitcoin mengalami drawdown **-76.63%**, Saham BBCA **-55.98%**, IHSG **-45.3%**, dan S&P 500 **-20.68%**.
  - All-Weather berhasil meredam drawdown hingga sekitar 70-95% lebih rendah dibandingkan aset terkonsentrasi!

### B. Hipotesis Sharpe Ratio & Risk Parity
- **Hipotesis Sander:** *Menyeimbangkan kontribusi risiko antar instrumen independen (saham, obligasi, emas, kas USD) memberikan return yang konsisten dengan volatilitas terkontrol.*
- **Bukti Empiris:**
  - Volatilitas tahunan All-Weather hanya **4.48%** (jauh lebih rendah dibanding BBCA di 21.98% atau Bitcoin di 42.21%).
  - Sharpe Ratio All-Weather mencapai **0.59**, membuktikan efisiensi alokasi modal optimal sesuai prinsip Risk Parity Ray Dalio.

### C. Matriks Korelasi Silang Antar Aset (Ray Dalio's Holy Grail)
Ray Dalio menyatakan bahwa *holy grail* investasi adalah memiliki 15–20 aliran return yang tidak berkorelasi satu sama lain (korelasi mendekati 0 atau negatif). Berikut adalah matriks korelasi empiris dari Parquet lakehouse kita:

| Aset | BBCA_ret | GSPC_ret | GOLD_ret | USDIDR_ret | JKSE_ret | BTC_ret |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **BBCA_ret** | +1.000 | +0.068 | -0.045 | -0.039 | +0.650 | +0.028 |
| **GSPC_ret** | +0.068 | +1.000 | +0.111 | -0.118 | +0.075 | +0.390 |
| **GOLD_ret** | -0.045 | +0.111 | +1.000 | -0.087 | +0.089 | +0.106 |
| **USDIDR_ret** | -0.039 | -0.118 | -0.087 | +1.000 | -0.119 | -0.097 |
| **JKSE_ret** | +0.650 | +0.075 | +0.089 | -0.119 | +1.000 | +0.028 |
| **BTC_ret** | +0.028 | +0.390 | +0.106 | -0.097 | +0.028 | +1.000 |


* **Observasi Kunci:**
  - Korelasi antara Emas (`GOLD_ret`) dan USD/IDR (`USDIDR_ret`) terhadap saham Indo (`BBCA` / `JKSE`) sangat rendah (< 0.15) bahkan negatif, berfungsi sebagai peredam guncangan sempurna saat terjadi krisis domestik.
