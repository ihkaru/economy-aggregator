//! Unified Zero-Key Liquidity, Flow & Positioning Sync Orchestrator (Phase 321).

use std::path::Path;
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::core::config::LakehouseConfig;
use crate::core::observation_types::MetricObservation;
use crate::features::ingestion::crypto_flows::{
    BGeometricsFetcher, CoinMetricsFetcher, DefiLlamaFetcher, ExchangeOiFetcher,
    GithubCryptoMirrorFetcher,
};
use crate::features::ingestion::flows::{
    CftcCotFetcher, FinraMarginFetcher, OccOiFetcher, SsgaXlsxFetcher, StockAnalysisFetcher,
};
use crate::features::ingestion::liquidity::{
    DbnomicsFedFetcher, NyFedFetcher, OfrFetcher, TreasuryFetcher,
};
use crate::features::storage::{ObservationWriter, PartitionResolver};

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
    let mode = target.trim().to_ascii_lowercase();
    let mut tasks = Vec::new();

    if matches!(mode.as_str(), "all" | "macro" | "liquidity_macro") {
        run_macro_tasks(&lake_dir, &batch_id, &mut tasks).await;
    }
    if matches!(mode.as_str(), "all" | "tradfi" | "tradfi_flows") {
        run_tradfi_tasks(&lake_dir, &batch_id, &mut tasks).await;
    }
    if matches!(mode.as_str(), "all" | "crypto" | "crypto_flows") {
        run_crypto_tasks(&lake_dir, &batch_id, &mut tasks).await;
    }

    let total_rows_written = tasks.iter().map(|t| t.rows_written).sum();
    ObservationSyncReport {
        batch_id,
        target: target.to_string(),
        total_rows_written,
        tasks,
    }
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
                ObservationWriter::new(PartitionResolver::new(dir)).write_observations(&rows)
            })
            .await;
            match write_res {
                Ok(Ok(_)) => tasks.push(SourceSyncStatus {
                    task: task.to_string(),
                    source_used: used,
                    rows_fetched: fetched,
                    rows_written: fetched,
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

async fn run_macro_tasks(lake_dir: &Path, batch_id: &str, tasks: &mut Vec<SourceSyncStatus>) {
    let tr = TreasuryFetcher::default();
    persist_and_record(lake_dir, "treasury_tga", "treasury_fiscaldata", false, tr.fetch_tga_balance(500, batch_id).await, tasks).await;
    let cur_year = Utc::now().year();
    for y in [cur_year - 1, cur_year] {
        persist_and_record(lake_dir, &format!("treasury_tbill_3m_{}", y), "treasury_tbill_csv", false, tr.fetch_tbill_3m_year(y, batch_id).await, tasks).await;
    }

    let nyf = NyFedFetcher::default();
    persist_and_record(lake_dir, "nyfed_on_rrp", "nyfed_api", false, nyf.fetch_on_rrp(1825, batch_id).await, tasks).await;

    let ofr = OfrFetcher::default();
    persist_and_record(lake_dir, "ofr_mmf_assets", "ofr_stfm", false, ofr.fetch_mmf_total_assets(batch_id).await, tasks).await;

    let fed = DbnomicsFedFetcher::default();
    persist_and_record(lake_dir, "dbnomics_h8_bank_credit", "dbnomics_fed", false, fed.fetch_h8_bank_credit(batch_id).await, tasks).await;
    persist_and_record(lake_dir, "dbnomics_h41_fed_assets", "dbnomics_fed", false, fed.fetch_h41_assets_backfill(batch_id).await, tasks).await;
    persist_and_record(lake_dir, "fed_h41_current_html", "fed_h41_html", false, fed.fetch_latest_fed_h41_html(batch_id).await, tasks).await;
}

async fn run_tradfi_tasks(lake_dir: &Path, batch_id: &str, tasks: &mut Vec<SourceSyncStatus>) {
    let ssga = SsgaXlsxFetcher::default();
    persist_and_record(lake_dir, "ssga_spy_xlsx", "ssga_xlsx", false, ssga.fetch_spy_history(batch_id).await, tasks).await;
    persist_and_record(lake_dir, "ssga_gld_xlsx", "ssga_xlsx", false, ssga.fetch_gld_history(batch_id).await, tasks).await;

    let cftc = CftcCotFetcher::default();
    persist_and_record(lake_dir, "cftc_gold_cot", "cftc_cot", false, cftc.fetch_gold_cot(520, batch_id).await, tasks).await;
    persist_and_record(
        lake_dir,
        "cftc_sp500_cot",
        "cftc_cot",
        false,
        cftc.fetch_equity_index_cot("13874A", "cot:SP500:open_interest_contracts", "CME_SP500", 520, batch_id).await,
        tasks,
    )
    .await;

    let sa = StockAnalysisFetcher::default();
    for (sym, comp) in [("SPY", "equities"), ("QQQ", "equities"), ("IBIT", "crypto"), ("IAU", "gold")] {
        persist_and_record(lake_dir, &format!("stockanalysis_{}", sym), "stockanalysis", false, sa.fetch_etf_snapshot(sym, Some(comp), batch_id).await, tasks).await;
    }

    let occ = OccOiFetcher::default();
    persist_and_record(lake_dir, "occ_options_oi", "occ_csv", false, occ.fetch_recent_months(3, batch_id).await, tasks).await;

    let finra = FinraMarginFetcher::default();
    persist_and_record(lake_dir, "finra_margin_debt", "finra_xlsx", false, finra.fetch_margin_debt(batch_id).await, tasks).await;
}

async fn run_crypto_tasks(lake_dir: &Path, batch_id: &str, tasks: &mut Vec<SourceSyncStatus>) {
    let dl = DefiLlamaFetcher::default();
    persist_and_record(lake_dir, "defillama_all", "defillama", false, dl.fetch_total_stablecoin_mcap(batch_id).await, tasks).await;
    persist_and_record(lake_dir, "defillama_usdt", "defillama", false, dl.fetch_asset_stablecoin_mcap(1, "USDT", batch_id).await, tasks).await;
    persist_and_record(lake_dir, "defillama_usdc", "defillama", false, dl.fetch_asset_stablecoin_mcap(2, "USDC", batch_id).await, tasks).await;

    let bg = BGeometricsFetcher::default();
    persist_and_record(lake_dir, "bgeometrics_etf_flow_btc", "bgeometrics", false, bg.fetch_btc_etf_flows(batch_id).await, tasks).await;

    let gh = GithubCryptoMirrorFetcher::default();
    let cm = CoinMetricsFetcher::default();
    persist_and_record(
        lake_dir,
        "coinmetrics_btc_netflow_usd",
        "coinmetrics_community",
        false,
        cm.fetch_btc_exchange_netflow(Some("2018-01-01"), 5000, batch_id).await,
        tasks,
    )
    .await;
    persist_and_record(
        lake_dir,
        "github_btc_netflow_btc",
        "github_ercindedeoglu",
        false,
        gh.fetch_series("btc_exchange_netflow.json", "onchain:BTC:github_netflow_btc", "onchain_flow", Some("crypto"), "BTC", "exchange_netflow_btc", "btc", batch_id).await,
        tasks,
    )
    .await;
    persist_and_record(
        lake_dir,
        "bgeometrics_btc_oi_usd",
        "bgeometrics",
        false,
        bg.fetch_futures_open_interest(batch_id).await,
        tasks,
    )
    .await;
    persist_and_record(
        lake_dir,
        "github_btc_oi_usd",
        "github_ercindedeoglu",
        false,
        gh.fetch_series("btc_open_interest.json", "deriv:BTC:github_oi_usd", "derivatives_oi", Some("deleveraging"), "BTC", "open_interest_usd", "usd", batch_id).await,
        tasks,
    )
    .await;

    let ex = ExchangeOiFetcher::default();
    persist_and_record(lake_dir, "binance_btc_oi_30d", "binance_futures", false, ex.fetch_binance_btc_oi(30, batch_id).await, tasks).await;
    persist_and_record(lake_dir, "okx_btc_oi", "okx_rubik", false, ex.fetch_okx_btc_oi(batch_id).await, tasks).await;
    persist_and_record(lake_dir, "bybit_btc_oi", "bybit_v5", false, ex.fetch_bybit_btc_oi(200, batch_id).await, tasks).await;
    persist_and_record(lake_dir, "deribit_btc_oi", "deribit", false, ex.fetch_deribit_btc_oi_snapshot(batch_id).await, tasks).await;
}
