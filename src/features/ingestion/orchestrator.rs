//! Autonomous Ingestion Orchestrator managing background gap catch-up and live feeds for containers.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::core::config::LakehouseConfig;
use crate::core::subscription::{SubscriptionItem, SubscriptionManager};
use crate::features::ingestion::catch_up::{CatchUpEngine, CatchUpSummary};

/// Evaluates whether a subscription is due to run based on its configured `poll_interval_secs`.
pub fn is_subscription_due(
    sub: &SubscriptionItem,
    last_ran: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    let interval_secs = sub.poll_interval_secs.unwrap_or(900).max(60) as i64;
    match last_ran {
        None => true,
        Some(prev) => now.signed_duration_since(prev).num_seconds() >= interval_secs,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorStatus {
    pub is_running: bool,
    pub boot_timestamp_utc: String,
    pub last_catch_up_utc: Option<String>,
    pub total_candles_ingested_session: usize,
    pub active_subscriptions_count: usize,
    pub active_streams: Vec<String>,
    pub active_pollers: Vec<String>,
}

pub struct IngestionOrchestrator {
    config: LakehouseConfig,
    sub_manager: Arc<SubscriptionManager>,
    status: Arc<RwLock<OrchestratorStatus>>,
}

impl IngestionOrchestrator {
    pub fn new(config: LakehouseConfig, sub_manager: Arc<SubscriptionManager>) -> Self {
        let status = Arc::new(RwLock::new(OrchestratorStatus {
            is_running: false,
            boot_timestamp_utc: Utc::now().to_rfc3339(),
            last_catch_up_utc: None,
            total_candles_ingested_session: 0,
            active_subscriptions_count: 0,
            active_streams: Vec::new(),
            active_pollers: Vec::new(),
        }));
        Self {
            config,
            sub_manager,
            status,
        }
    }

    pub fn status(&self) -> OrchestratorStatus {
        self.status.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Spawns autonomous ingestion background daemon (Catch-Up then Live Loop).
    pub fn start(&self) {
        let config = self.config.clone();
        let sub_manager = self.sub_manager.clone();
        let status_arc = self.status.clone();

        tokio::spawn(async move {
            {
                let mut st = status_arc.write().unwrap_or_else(|e| e.into_inner());
                st.is_running = true;
            }
            info!("🛡️ Autonomous Ingestion Orchestrator booting up in background");

            // 0. Startup compaction cleanup (merges any micro-chunk sprawl >= 4 files before catch-up)
            let boot_lake = config.lake_dir.clone();
            let _ = tokio::task::spawn_blocking(move || {
                crate::features::storage::LakeCompactor::default().compact_all(&boot_lake, 4)
            })
            .await;

            // 1. Initial Gap Catch-Up Run
            let mut last_ran: HashMap<String, DateTime<Utc>> = HashMap::new();
            run_gap_catch_up(&config, &sub_manager, &status_arc).await;
            let boot_done = Utc::now();
            if let Ok(subs) = sub_manager.list_active() {
                for s in subs {
                    last_ran.insert(format!("{}:{}", s.source, s.symbol), boot_done);
                }
            }

            // 2. Per-Subscription Cadence Supervisor & Auto-Compaction Loop
            let mut tick_counter: u64 = 0;
            loop {
                sleep(Duration::from_secs(60)).await;
                tick_counter += 1;

                // Reload active subscriptions dynamically (allows live config additions)
                let active_subs = match sub_manager.list_active() {
                    Ok(subs) => subs,
                    Err(e) => {
                        warn!("Failed reloading subscriptions: {}", e);
                        continue;
                    }
                };

                {
                    if let Ok(mut st) = status_arc.write() {
                        st.active_subscriptions_count = active_subs.len();
                        st.active_pollers = active_subs.iter().map(|s| format!("{}:{}", s.source, s.symbol)).collect();
                    }
                }

                // Trigger ONLY subscriptions whose configured `poll_interval_secs` has elapsed
                let now = Utc::now();
                let due_subs: Vec<SubscriptionItem> = active_subs
                    .into_iter()
                    .filter(|s| {
                        let key = format!("{}:{}", s.source, s.symbol);
                        is_subscription_due(s, last_ran.get(&key).copied(), now)
                    })
                    .collect();
                if !due_subs.is_empty() {
                    let engine = CatchUpEngine::new(config.clone());
                    let mut total_new = 0usize;
                    for sub in &due_subs {
                        let summaries = engine.catch_up_subscription(sub, false).await;
                        total_new += summaries.iter().map(|s| s.candles_ingested).sum::<usize>();
                        last_ran.insert(format!("{}:{}", sub.source, sub.symbol), Utc::now());
                    }
                    if let Ok(mut st) = status_arc.write() {
                        st.last_catch_up_utc = Some(Utc::now().to_rfc3339());
                        st.total_candles_ingested_session += total_new;
                    }
                    if total_new > 0 {
                        info!("✅ Cadence sync finished across {} due subscription(s): +{} new rows", due_subs.len(), total_new);
                    }
                }

                // Every 6 hours (360 ticks): Compact small Parquet micro-chunks off the async worker
                if tick_counter % 360 == 0 {
                    let lake_dir = config.lake_dir.clone();
                    match tokio::task::spawn_blocking(move || {
                        crate::features::storage::LakeCompactor::default().compact_all(&lake_dir, 2)
                    })
                    .await
                    {
                        Ok(Ok(results)) if !results.is_empty() => {
                            let merged: usize = results.iter().map(|r| r.files_compacted).sum();
                            info!("🗜️ Scheduled auto-compaction merged {} micro-chunks across {} partitions", merged, results.len());
                        }
                        Ok(Err(e)) => warn!("Scheduled auto-compaction error: {}", e),
                        Err(e) => warn!("Scheduled auto-compaction join error: {}", e),
                        _ => {}
                    }
                }
            }
        });
    }

    /// Triggers immediate on-demand catch-up across all active subscriptions.
    pub async fn trigger_catch_up(&self) -> usize {
        let summaries = self.trigger_catch_up_filtered(None, None, false).await;
        summaries.iter().map(|s| s.candles_ingested).sum()
    }

    /// Triggers targeted or full on-demand catch-up with optional symbol/source filter and force refresh.
    pub async fn trigger_catch_up_filtered(
        &self,
        symbol_filter: Option<&str>,
        source_filter: Option<&str>,
        force: bool,
    ) -> Vec<CatchUpSummary> {
        run_gap_catch_up_filtered(
            &self.config,
            &self.sub_manager,
            &self.status,
            symbol_filter,
            source_filter,
            force,
        )
        .await
    }
}

async fn run_gap_catch_up(
    config: &LakehouseConfig,
    sub_manager: &SubscriptionManager,
    status_arc: &Arc<RwLock<OrchestratorStatus>>,
) -> usize {
    let summaries =
        run_gap_catch_up_filtered(config, sub_manager, status_arc, None, None, false).await;
    summaries.iter().map(|s| s.candles_ingested).sum()
}

async fn run_gap_catch_up_filtered(
    config: &LakehouseConfig,
    sub_manager: &SubscriptionManager,
    status_arc: &Arc<RwLock<OrchestratorStatus>>,
    symbol_filter: Option<&str>,
    source_filter: Option<&str>,
    force: bool,
) -> Vec<CatchUpSummary> {
    let active_subs = match sub_manager.list_active() {
        Ok(s) => s,
        Err(e) => {
            error!("Orchestrator failed listing subscriptions: {}", e);
            return Vec::new();
        }
    };

    let filtered: Vec<_> = active_subs
        .into_iter()
        .filter(|s| {
            if let Some(sym) = symbol_filter.filter(|v| !v.trim().is_empty()) {
                if !s.symbol.eq_ignore_ascii_case(sym.trim()) {
                    return false;
                }
            }
            if let Some(src) = source_filter.filter(|v| !v.trim().is_empty()) {
                if !s.source.eq_ignore_ascii_case(src.trim()) {
                    return false;
                }
            }
            true
        })
        .collect();

    info!(
        "🔄 Orchestrator starting gap catch-up across {} matching subscription(s) (force={})",
        filtered.len(),
        force
    );

    let engine = CatchUpEngine::new(config.clone());
    let mut all_summaries = Vec::new();
    let mut total_new = 0;

    for sub in &filtered {
        let summaries = engine.catch_up_subscription(sub, force).await;
        for s in &summaries {
            total_new += s.candles_ingested;
        }
        all_summaries.extend(summaries);
    }

    if let Ok(mut st) = status_arc.write() {
        st.last_catch_up_utc = Some(Utc::now().to_rfc3339());
        st.total_candles_ingested_session += total_new;
    }

    info!(
        "✅ Orchestrator catch-up phase finished. Added {} new records/candles to lake",
        total_new
    );
    all_summaries
}
