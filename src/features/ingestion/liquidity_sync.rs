//! Watermark-Aware Incremental Liquidity, Flow & Positioning Sync Orchestrator (Phase 321 + 334 + 338).

use std::collections::HashMap;
use std::path::Path;
use chrono::{DateTime, Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::core::config::LakehouseConfig;
use crate::core::observation_types::{MetricObservation, ObservationFrequency};
use crate::features::ingestion::crypto_flows::{
    BGeometricsFetcher, CoinMetricsFetcher, DefiLlamaFetcher, ExchangeOiFetcher,
    GithubCryptoMirrorFetcher,
};
use crate::features::ingestion::flows::{
    CftcCotFetcher, FinraMarginFetcher, OccOiFetcher, SsgaXlsxFetcher, StockAnalysisFetcher,
};
use crate::features::ingestion::liquidity::{
    DbnomicsFedFetcher, FredFetcher, NyFedFetcher, OfrFetcher, TreasuryFetcher,
    CANONICAL_FRED_SPECS,
};
use crate::features::storage::{
    ObservationReader, ObservationWriter, PartitionResolver, SeriesWatermark,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSyncStatus {
    pub task: String,
    pub source_used: String,
    pub rows_fetched: usize,
    pub rows_written: usize,
    pub fallback_triggered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSyncReport {
    pub batch_id: String,
    pub target: String,
    pub total_rows_written: usize,
    pub tasks: Vec<SourceSyncStatus>,
}

pub async fn sync_observations(config: &LakehouseConfig, target: &str) -> ObservationSyncReport {
    let batch_id = format!(
        "obs_{}_{}",
        Utc::now().format("%Y%m%dT%H%M%SZ"),
        &Uuid::new_v4().to_string()[..6]
    );
    let lake_dir = config.lake_dir.clone();
    let wm_dir = lake_dir.clone();
    let watermarks = tokio::task::spawn_blocking(move || {
        ObservationReader::new(PartitionResolver::new(wm_dir)).scan_watermarks()
    })
    .await
    .unwrap_or_default();

    let mode = target.trim().to_ascii_lowercase();
    let mut tasks = Vec::new();
    let now = Utc::now();

    if matches!(mode.as_str(), "all" | "macro" | "liquidity_macro") {
        run_macro_tasks(&lake_dir, &batch_id, &watermarks, now, &mut tasks).await;
    }
    if matches!(mode.as_str(), "all" | "tradfi" | "tradfi_flows") {
        run_tradfi_tasks(&lake_dir, &batch_id, &watermarks, now, &mut tasks).await;
    }
    if matches!(mode.as_str(), "all" | "crypto" | "crypto_flows") {
        run_crypto_tasks(&lake_dir, &batch_id, &watermarks, now, &mut tasks).await;
    }

    let total_rows_written = tasks.iter().map(|t| t.rows_written).sum();
    ObservationSyncReport {
        batch_id,
        target: target.to_string(),
        total_rows_written,
        tasks,
    }
}

/// Returns true if a series was already synced recently enough relative to its publication cadence.
pub fn is_series_fresh(
    wm: Option<&SeriesWatermark>,
    freq: ObservationFrequency,
    now: DateTime<Utc>,
) -> bool {
    let Some(w) = wm else { return false };
    let since_sync = now.signed_duration_since(w.latest_known_at_utc);
    let since_obs = now.signed_duration_since(w.max_ts_utc);
    match freq {
        ObservationFrequency::Monthly | ObservationFrequency::Quarterly => {
            since_sync < Duration::hours(24) && since_obs < Duration::days(60)
        }
        ObservationFrequency::Weekly => {
            since_sync < Duration::hours(12) && since_obs < Duration::days(14)
        }
        ObservationFrequency::Daily | ObservationFrequency::Irregular => {
            since_sync < Duration::hours(4)
        }
    }
}

fn push_skipped(tasks: &mut Vec<SourceSyncStatus>, task: &str, source: &str) {
    tasks.push(SourceSyncStatus {
        task: task.to_string(),
        source_used: source.to_string(),
        rows_fetched: 0,
        rows_written: 0,
        fallback_triggered: false,
        error: None,
    });
}

async fn persist_and_record(
    lake_dir: &Path,
    task: &str,
    primary_source: &str,
    fallback_triggered: bool,
    res: Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>>,
    tasks: &mut Vec<SourceSyncStatus>,
) {
    match res {
        Ok(rows) => {
            let fetched = rows.len();
            let used = rows
                .first()
                .map(|r| r.provenance.source.clone())
                .unwrap_or_else(|| primary_source.to_string());
            let dir = lake_dir.to_path_buf();
            let write_res = tokio::task::spawn_blocking(move || {
                ObservationWriter::new(PartitionResolver::new(dir)).write_incremental(&rows)
            })
            .await;
            match write_res {
                Ok(Ok(written)) => tasks.push(SourceSyncStatus {
                    task: task.to_string(),
                    source_used: used,
                    rows_fetched: fetched,
                    rows_written: written,
                    fallback_triggered,
                    error: None,
                }),
                Ok(Err(e)) => push_err(tasks, task, &used, fetched, fallback_triggered, e.to_string()),
                Err(e) => push_err(tasks, task, &used, fetched, fallback_triggered, e.to_string()),
            }
        }
        Err(e) => push_err(tasks, task, primary_source, 0, fallback_triggered, e.to_string()),
    }
}

fn push_err(
    tasks: &mut Vec<SourceSyncStatus>,
    task: &str,
    source: &str,
    fetched: usize,
    fallback: bool,
    err: String,
) {
    warn!("⚠️ Observation task '{}' issue: {}", task, err);
    tasks.push(SourceSyncStatus {
        task: task.to_string(),
        source_used: source.to_string(),
        rows_fetched: fetched,
        rows_written: 0,
        fallback_triggered: fallback,
        error: Some(err),
    });
}

async fn run_macro_tasks(
    lake_dir: &Path,
    batch_id: &str,
    wms: &HashMap<String, SeriesWatermark>,
    now: DateTime<Utc>,
    tasks: &mut Vec<SourceSyncStatus>,
) {
    let tr = TreasuryFetcher::default();
    let tga_wm = wms.get("macro:US:TGA_BALANCE_USD");
    if is_series_fresh(tga_wm, ObservationFrequency::Daily, now) {
        push_skipped(tasks, "treasury_tga", "treasury_fiscaldata");
    } else {
        let start = tga_wm
            .filter(|w| w.min_year <= 2016)
            .map(|w| (w.max_ts_utc - Duration::days(14)).format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "2016-01-01".to_string());
        persist_and_record(lake_dir, "treasury_tga", "treasury_fiscaldata", false, tr.fetch_tga_balance_since(&start, 10_000, batch_id).await, tasks).await;
    }

    let cur_year = now.year();
    let tb_wm = wms.get("macro:US:TBILL_3M_PCT");
    if is_series_fresh(tb_wm, ObservationFrequency::Daily, now) {
        push_skipped(tasks, "treasury_tbill_3m", "treasury_tbill_csv");
    } else {
        let start_y = if tb_wm.is_some_and(|w| w.min_year <= cur_year - 9) { cur_year } else { cur_year - 10 };
        for y in start_y..=cur_year {
            persist_and_record(lake_dir, &format!("treasury_tbill_3m_{}", y), "treasury_tbill_csv", false, tr.fetch_tbill_3m_year(y, batch_id).await, tasks).await;
        }
    }

    if is_series_fresh(wms.get("macro:US:ON_RRP_USD"), ObservationFrequency::Daily, now) {
        push_skipped(tasks, "nyfed_on_rrp", "nyfed_api");
    } else {
        let days = if wms.get("macro:US:ON_RRP_USD").is_some_and(|w| w.year_partitions >= 4) { 30 } else { 1825 };
        persist_and_record(lake_dir, "nyfed_on_rrp", "nyfed_api", false, NyFedFetcher::default().fetch_on_rrp(days, batch_id).await, tasks).await;
    }

    if is_series_fresh(wms.get("macro:US:MMF_ASSETS_USD"), ObservationFrequency::Monthly, now) {
        push_skipped(tasks, "ofr_mmf_assets", "ofr_stfm");
    } else {
        persist_and_record(lake_dir, "ofr_mmf_assets", "ofr_stfm", false, OfrFetcher::default().fetch_mmf_total_assets(batch_id).await, tasks).await;
    }

    let fed = DbnomicsFedFetcher::default();
    if is_series_fresh(wms.get("macro:US:BANK_CREDIT_USD"), ObservationFrequency::Weekly, now) {
        push_skipped(tasks, "dbnomics_h8_bank_credit", "dbnomics_fed");
    } else {
        persist_and_record(lake_dir, "dbnomics_h8_bank_credit", "dbnomics_fed", false, fed.fetch_h8_bank_credit(batch_id).await, tasks).await;
    }
    if is_series_fresh(wms.get("macro:US:FED_H41_RESERVE_FACTORS_USD"), ObservationFrequency::Weekly, now) {
        push_skipped(tasks, "dbnomics_h41_fed_assets", "dbnomics_fed");
    } else {
        persist_and_record(lake_dir, "dbnomics_h41_fed_assets", "dbnomics_fed", false, fed.fetch_h41_assets_backfill(batch_id).await, tasks).await;
    }
    if is_series_fresh(wms.get("macro:US:FED_TOTAL_ASSETS_USD"), ObservationFrequency::Weekly, now) {
        push_skipped(tasks, "fed_h41_current_html", "fed_h41_html");
    } else {
        persist_and_record(lake_dir, "fed_h41_current_html", "fed_h41_html", false, fed.fetch_latest_fed_h41_html(batch_id).await, tasks).await;
    }

    if let Some(fred_cli) = FredFetcher::from_env() {
        for spec in CANONICAL_FRED_SPECS {
            let task_name = format!("fred_{}", spec.fred_id.to_ascii_lowercase());
            let wm = wms.get(spec.series_id);
            if is_series_fresh(wm, spec.frequency, now) {
                push_skipped(tasks, &task_name, "fred_api");
                continue;
            }
            let start = wm
                .filter(|w| w.year_partitions >= 3)
                .map(|w| (w.max_ts_utc - Duration::days(45)).format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "2016-01-01".to_string());
            persist_and_record(lake_dir, &task_name, "fred_api", false, fred_cli.fetch_series(spec, &start, batch_id).await, tasks).await;
        }
    } else {
        tasks.push(SourceSyncStatus {
            task: "fred_rest_api".to_string(),
            source_used: "fred_api".to_string(),
            rows_fetched: 0,
            rows_written: 0,
            fallback_triggered: false,
            error: Some("Skipped: FRED_API_KEY env var not configured".to_string()),
        });
    }
}

async fn run_tradfi_tasks(
    lake_dir: &Path,
    batch_id: &str,
    wms: &HashMap<String, SeriesWatermark>,
    now: DateTime<Utc>,
    tasks: &mut Vec<SourceSyncStatus>,
) {
    let ssga = SsgaXlsxFetcher::default();
    for (sid, task, kind) in [
        ("etf:SPY:aum_usd", "ssga_spy_xlsx", "SPY"),
        ("etf:GLD:aum_usd", "ssga_gld_xlsx", "GLD"),
        ("etf:BIL:aum_usd", "ssga_bil_xlsx", "BIL"),
        ("etf:JNK:aum_usd", "ssga_jnk_xlsx", "JNK"),
        ("etf:SPTL:aum_usd", "ssga_sptl_xlsx", "SPTL"),
    ] {
        if is_series_fresh(wms.get(sid), ObservationFrequency::Daily, now) {
            push_skipped(tasks, task, "ssga_xlsx");
        } else {
            let res = match kind {
                "SPY" => ssga.fetch_spy_history(batch_id).await,
                "GLD" => ssga.fetch_gld_history(batch_id).await,
                "BIL" => ssga.fetch_bil_history(batch_id).await,
                "JNK" => ssga.fetch_jnk_history(batch_id).await,
                _ => ssga.fetch_sptl_history(batch_id).await,
            };
            persist_and_record(lake_dir, task, "ssga_xlsx", false, res, tasks).await;
        }
    }

    let cftc = CftcCotFetcher::default();
    if is_series_fresh(wms.get("cot:GOLD:managed_money_net"), ObservationFrequency::Weekly, now) {
        push_skipped(tasks, "cftc_gold_cot", "cftc_cot");
    } else {
        persist_and_record(lake_dir, "cftc_gold_cot", "cftc_cot", false, cftc.fetch_gold_cot(520, batch_id).await, tasks).await;
    }
    if is_series_fresh(wms.get("cot:SP500:open_interest_contracts"), ObservationFrequency::Weekly, now) {
        push_skipped(tasks, "cftc_sp500_cot", "cftc_cot");
    } else {
        persist_and_record(lake_dir, "cftc_sp500_cot", "cftc_cot", false, cftc.fetch_equity_index_cot("13874A", "cot:SP500:open_interest_contracts", "CME_SP500", 520, batch_id).await, tasks).await;
    }

    let sa = StockAnalysisFetcher::default();
    for (sym, comp) in [("SPY", "equities"), ("QQQ", "equities"), ("IBIT", "crypto"), ("IAU", "gold")] {
        let sid = format!("etf:{}:stockanalysis_shares_outstanding", sym);
        if is_series_fresh(wms.get(&sid), ObservationFrequency::Daily, now) {
            push_skipped(tasks, &format!("stockanalysis_{}", sym), "stockanalysis");
        } else {
            persist_and_record(lake_dir, &format!("stockanalysis_{}", sym), "stockanalysis", false, sa.fetch_etf_snapshot(sym, Some(comp), batch_id).await, tasks).await;
        }
    }

    let occ_wm = wms.get("deriv:EQ_OPT:open_interest_contracts");
    if is_series_fresh(occ_wm, ObservationFrequency::Daily, now) {
        push_skipped(tasks, "occ_options_oi", "occ_csv");
    } else {
        let months = if occ_wm.is_some_and(|w| w.year_partitions >= 2) { 2 } else { 24 };
        persist_and_record(lake_dir, "occ_options_oi", "occ_csv", false, OccOiFetcher::default().fetch_recent_months(months, batch_id).await, tasks).await;
    }

    if is_series_fresh(wms.get("credit:US:FINRA_MARGIN_DEBT_USD"), ObservationFrequency::Monthly, now) {
        push_skipped(tasks, "finra_margin_debt", "finra_xlsx");
    } else {
        persist_and_record(lake_dir, "finra_margin_debt", "finra_xlsx", false, FinraMarginFetcher::default().fetch_margin_debt(batch_id).await, tasks).await;
    }
}

async fn run_crypto_tasks(
    lake_dir: &Path,
    batch_id: &str,
    wms: &HashMap<String, SeriesWatermark>,
    now: DateTime<Utc>,
    tasks: &mut Vec<SourceSyncStatus>,
) {
    let dl = DefiLlamaFetcher::default();
    if is_series_fresh(wms.get("stablecoin:ALL:mcap_usd"), ObservationFrequency::Daily, now) {
        push_skipped(tasks, "defillama_stablecoins", "defillama");
    } else {
        persist_and_record(lake_dir, "defillama_all", "defillama", false, dl.fetch_total_stablecoin_mcap(batch_id).await, tasks).await;
        persist_and_record(lake_dir, "defillama_usdt", "defillama", false, dl.fetch_asset_stablecoin_mcap(1, "USDT", batch_id).await, tasks).await;
        persist_and_record(lake_dir, "defillama_usdc", "defillama", false, dl.fetch_asset_stablecoin_mcap(2, "USDC", batch_id).await, tasks).await;
    }

    let bg = BGeometricsFetcher::default();
    if is_series_fresh(wms.get("etf:BTC_US:bgeometrics_flow_btc"), ObservationFrequency::Daily, now) {
        push_skipped(tasks, "bgeometrics_etf_flow_btc", "bgeometrics");
    } else {
        persist_and_record(lake_dir, "bgeometrics_etf_flow_btc", "bgeometrics", false, bg.fetch_btc_etf_flows(batch_id).await, tasks).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    }
    if is_series_fresh(wms.get("deriv:BTC:bgeometrics_oi_usd"), ObservationFrequency::Daily, now) {
        push_skipped(tasks, "bgeometrics_btc_oi_usd", "bgeometrics");
    } else {
        persist_and_record(lake_dir, "bgeometrics_btc_oi_usd", "bgeometrics", false, bg.fetch_futures_open_interest(batch_id).await, tasks).await;
    }

    let cm_wm = wms.get("onchain:BTC:coinmetrics_netflow_usd");
    if is_series_fresh(cm_wm, ObservationFrequency::Daily, now) {
        push_skipped(tasks, "coinmetrics_btc_netflow_usd", "coinmetrics_community");
    } else {
        let start = cm_wm
            .filter(|w| w.min_year <= 2018)
            .map(|w| (w.max_ts_utc - Duration::days(7)).format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "2018-01-01".to_string());
        persist_and_record(lake_dir, "coinmetrics_btc_netflow_usd", "coinmetrics_community", false, CoinMetricsFetcher::default().fetch_btc_exchange_netflow(Some(&start), 10_000, batch_id).await, tasks).await;
    }

    let gh = GithubCryptoMirrorFetcher::default();
    if is_series_fresh(wms.get("onchain:BTC:github_netflow_btc"), ObservationFrequency::Daily, now) {
        push_skipped(tasks, "github_crypto_mirrors", "github_ercindedeoglu");
    } else {
        persist_and_record(lake_dir, "github_btc_netflow_btc", "github_ercindedeoglu", false, gh.fetch_series("btc_exchange_netflow.json", "onchain:BTC:github_netflow_btc", "onchain_flow", Some("crypto"), "BTC", "exchange_netflow_btc", "btc", batch_id).await, tasks).await;
        persist_and_record(lake_dir, "github_btc_oi_usd", "github_ercindedeoglu", false, gh.fetch_series("btc_open_interest.json", "deriv:BTC:github_oi_usd", "derivatives_oi", Some("deleveraging"), "BTC", "open_interest_usd", "usd", batch_id).await, tasks).await;
    }

    let ex = ExchangeOiFetcher::default();
    if is_series_fresh(wms.get("deriv:BINANCE_BTC:open_interest_usd"), ObservationFrequency::Daily, now) {
        push_skipped(tasks, "exchange_oi_batch", "exchanges");
    } else {
        persist_and_record(lake_dir, "binance_btc_oi_30d", "binance_futures", false, ex.fetch_binance_btc_oi(30, batch_id).await, tasks).await;
        persist_and_record(lake_dir, "okx_btc_oi", "okx_rubik", false, ex.fetch_okx_btc_oi(batch_id).await, tasks).await;
        persist_and_record(lake_dir, "bybit_btc_oi", "bybit_v5", false, ex.fetch_bybit_btc_oi(200, batch_id).await, tasks).await;
        persist_and_record(lake_dir, "deribit_btc_oi", "deribit", false, ex.fetch_deribit_btc_oi_snapshot(batch_id).await, tasks).await;
    }
}
