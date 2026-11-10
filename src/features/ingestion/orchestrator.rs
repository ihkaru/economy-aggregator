//! Autonomous Ingestion Orchestrator managing background gap catch-up and live feeds for containers.

use std::sync::{Arc, RwLock};
use std::time::Duration;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::core::config::LakehouseConfig;
use crate::core::subscription::SubscriptionManager;
use crate::features::ingestion::catch_up::CatchUpEngine;

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
        self.status.read().unwrap().clone()
    }

    /// Spawns autonomous ingestion background daemon (Catch-Up then Live Loop).
    pub fn start(&self) {
        let config = self.config.clone();
        let sub_manager = self.sub_manager.clone();
        let status_arc = self.status.clone();

        tokio::spawn(async move {
            {
                let mut st = status_arc.write().unwrap();
                st.is_running = true;
            }
            info!("🛡️ Autonomous Ingestion Orchestrator booting up in background");

            // 1. Initial Gap Catch-Up Run
            run_gap_catch_up(&config, &sub_manager, &status_arc).await;

            // 2. Periodic Polling & Live Stream Supervisor Loop
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

                // Update status counters
                {
                    if let Ok(mut st) = status_arc.write() {
                        st.active_subscriptions_count = active_subs.len();
                        st.active_pollers = active_subs.iter().map(|s| s.canonical.clone()).collect();
                    }
                }

                // Every 15 minutes (15 ticks): Trigger incremental catch-up for scheduled items
                if tick_counter % 15 == 0 {
                    run_gap_catch_up(&config, &sub_manager, &status_arc).await;
                }
            }
        });
    }

    /// Triggers immediate on-demand catch-up across all active subscriptions.
    pub async fn trigger_catch_up(&self) -> usize {
        run_gap_catch_up(&self.config, &self.sub_manager, &self.status).await
    }
}

async fn run_gap_catch_up(
    config: &LakehouseConfig,
    sub_manager: &SubscriptionManager,
    status_arc: &Arc<RwLock<OrchestratorStatus>>,
) -> usize {
    let active_subs = match sub_manager.list_active() {
        Ok(s) => s,
        Err(e) => {
            error!("Orchestrator failed listing subscriptions: {}", e);
            return 0;
        }
    };

    info!(
        "🔄 Orchestrator starting gap catch-up across {} active subscriptions",
        active_subs.len()
    );

    let engine = CatchUpEngine::new(config.clone());
    let mut total_new = 0;

    for sub in &active_subs {
        let summaries = engine.catch_up_subscription(sub, false).await;
        for s in summaries {
            total_new += s.candles_ingested;
        }
    }

    if let Ok(mut st) = status_arc.write() {
        st.last_catch_up_utc = Some(Utc::now().to_rfc3339());
        st.total_candles_ingested_session += total_new;
    }

    info!(
        "✅ Orchestrator catch-up phase finished. Added {} new records/candles to lake",
        total_new
    );
    total_new
}
