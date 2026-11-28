//! Autonomous Continuous Analytics Supervisor (Phase 355).
//! Monitors lake watermarks, runs checksum-deduplicated 1w/4w diagnostic trees, emits event-sourced
//! regime transitions, updates frozen-protocol Prospective OOS scorecards, and runs non-destructive
//! Parquet bin-packing compaction every 24 hours.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use chrono::{NaiveDate, Utc};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::core::continuous_types::{
    ContinuousCycleOutcome, ContinuousSupervisorStatus, WindowCheckpoint,
};
use crate::features::analytics::{
    detect_regime_transition, evaluate_liquidity_tree, evaluate_prospective_scorecard,
};
use crate::features::storage::{ContinuousAnalyticsStore, DiagnosticTreeLedger};
use crate::runtime::{build_prospective_inputs, build_tree_inputs, compute_lake_watermark_hash};

pub struct ContinuousAnalyticsSupervisor {
    lake_dir: PathBuf,
    tracked_windows: Vec<usize>,
    check_interval_secs: u64,
    status: Arc<RwLock<ContinuousSupervisorStatus>>,
    run_mutex: Arc<Mutex<()>>,
}

impl ContinuousAnalyticsSupervisor {
    pub fn new(lake_dir: PathBuf) -> Self {
        let check_interval_secs = std::env::var("CONTINUOUS_ANALYTICS_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| v.max(60))
            .unwrap_or(900);
        let tracked_windows = vec![1, 4];
        let store = ContinuousAnalyticsStore::new(&lake_dir);
        let cp = store.load_checkpoint();
        Self {
            lake_dir,
            tracked_windows: tracked_windows.clone(),
            check_interval_secs,
            status: Arc::new(RwLock::new(ContinuousSupervisorStatus {
                is_running: false,
                check_interval_secs,
                tracked_windows,
                last_error: None,
                checkpoint: cp,
            })),
            run_mutex: Arc::new(Mutex::new(())),
        }
    }

    pub fn status(&self) -> ContinuousSupervisorStatus {
        self.status.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Spawns background continuous analytics loop (idempotent boot pass + periodic cadence).
    pub fn start(self: &Arc<Self>) {
        let sup = Arc::clone(self);
        tokio::spawn(async move {
            if let Ok(mut st) = sup.status.write() {
                st.is_running = true;
            }
            info!(
                "🧠 Continuous Analytics Supervisor started (interval={}s, windows={:?})",
                sup.check_interval_secs, sup.tracked_windows
            );
            // Allow boot ingestion catch-up a brief head start before first pass.
            sleep(Duration::from_secs(20)).await;

            loop {
                match sup.run_cycle(None, false, true).await {
                    Ok(out) => {
                        if !out.persisted_run_ids.is_empty() || !out.emitted_transition_ids.is_empty() {
                            info!(
                                "📊 Continuous analytics pass {}: persisted_runs={}, transitions={}, oos_persisted={}, compacted={}",
                                out.as_of_date,
                                out.persisted_run_ids.len(),
                                out.emitted_transition_ids.len(),
                                out.prospective_oos_persisted,
                                out.compacted_files
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Continuous analytics pass error: {}", e);
                        if let Ok(mut st) = sup.status.write() {
                            st.last_error = Some(e);
                        }
                    }
                }
                sleep(Duration::from_secs(sup.check_interval_secs)).await;
            }
        });
    }

    /// Executes one serialized, idempotent continuous analytics cycle.
    pub async fn run_cycle(
        &self,
        as_of_override: Option<NaiveDate>,
        force: bool,
        include_spillover: bool,
    ) -> Result<ContinuousCycleOutcome, String> {
        let _guard = self.run_mutex.lock().await;
        let lake_dir = self.lake_dir.clone();
        let windows = self.tracked_windows.clone();
        let status_arc = Arc::clone(&self.status);

        tokio::task::spawn_blocking(move || {
            let now = Utc::now();
            let as_of = as_of_override.unwrap_or_else(|| now.date_naive());
            let as_of_str = as_of.format("%Y-%m-%d").to_string();

            let store = ContinuousAnalyticsStore::new(&lake_dir);
            let tree_ledger = DiagnosticTreeLedger::new(&lake_dir);
            let mut cp = store.load_checkpoint();
            let watermark_hash = compute_lake_watermark_hash(&lake_dir);

            let all_windows_current = windows.iter().all(|w| {
                cp.windows
                    .get(w)
                    .is_some_and(|wc| wc.last_as_of_date == as_of_str)
            });

            if !force
                && cp.last_data_watermark_hash.as_deref() == Some(watermark_hash.as_str())
                && all_windows_current
            {
                cp.total_cycles_executed += 1;
                cp.updated_at_utc = now;
                let _ = store.save_checkpoint(&cp);
                if let Ok(mut st) = status_arc.write() {
                    st.checkpoint = cp;
                    st.last_error = None;
                }
                return Ok(ContinuousCycleOutcome {
                    executed_at_utc: now,
                    as_of_date: as_of_str,
                    data_watermark_hash: watermark_hash,
                    skipped_unchanged_watermark: true,
                    persisted_run_ids: Vec::new(),
                    emitted_transition_ids: Vec::new(),
                    prospective_oos_persisted: false,
                    compacted_files: 0,
                });
            }

            let mut persisted_run_ids = Vec::new();
            let mut emitted_transition_ids = Vec::new();

            for &w in &windows {
                let inputs = build_tree_inputs(&lake_dir, as_of, w, include_spillover);
                let report = evaluate_liquidity_tree(inputs);
                let prev = tree_ledger.latest_run_for_window(w).ok().flatten();
                let should_persist = force
                    || prev.as_ref().is_none_or(|p| {
                        p.as_of_date != report.as_of_date
                            || p.tree_checksum_sha256 != report.tree_checksum_sha256
                            || p.protocol_version != report.protocol_version
                    });

                if should_persist {
                    tree_ledger.persist_run(&report)?;
                    persisted_run_ids.push(report.run_id.clone());
                    cp.total_runs_persisted += 1;

                    if let Some(ev) = detect_regime_transition(prev.as_ref(), &report) {
                        store.persist_transition(&ev)?;
                        emitted_transition_ids.push(ev.transition_id);
                        cp.total_transitions_emitted += 1;
                    }

                    cp.windows.insert(
                        w,
                        WindowCheckpoint {
                            window_weeks: w,
                            last_run_id: report.run_id,
                            last_as_of_date: report.as_of_date,
                            last_evaluated_at_utc: report.evaluated_at_utc,
                            last_protocol_version: report.protocol_version,
                            last_root_regime: report.root_regime,
                            last_terminal_leaf_id: report.terminal_leaf_id,
                            last_checksum_sha256: report.tree_checksum_sha256,
                        },
                    );
                } else if let Some(p) = prev {
                    cp.windows.entry(w).or_insert(WindowCheckpoint {
                        window_weeks: w,
                        last_run_id: p.run_id,
                        last_as_of_date: p.as_of_date,
                        last_evaluated_at_utc: p.evaluated_at_utc,
                        last_protocol_version: p.protocol_version,
                        last_root_regime: p.root_regime,
                        last_terminal_leaf_id: p.terminal_leaf_id,
                        last_checksum_sha256: p.tree_checksum_sha256,
                    });
                }
            }

            // Evaluate frozen Prospective OOS Scorecard (persist only on new week or force)
            let oos_inputs = build_prospective_inputs(&lake_dir, as_of, include_spillover);
            let scorecard = evaluate_prospective_scorecard(&oos_inputs);
            let mut oos_persisted = false;
            let week_changed = cp.last_oos_week_scored != scorecard.latest_panel_week
                || cp.last_oos_week_scored.is_none();
            if force || week_changed {
                store.persist_scorecard(&scorecard)?;
                cp.last_oos_week_scored = scorecard.latest_panel_week.clone().or_else(|| Some(as_of_str.clone()));
                oos_persisted = true;
            }

            // Periodic 24-hour non-destructive bin-packing compaction
            let due_compact = cp
                .last_compaction_utc
                .is_none_or(|t| now.signed_duration_since(t).num_hours() >= 24);
            let mut compacted_files = 0usize;
            if due_compact {
                compacted_files += tree_ledger.compact_year_partitions(8).unwrap_or(0);
                compacted_files += store.compact_transition_partitions(8).unwrap_or(0);
                cp.last_compaction_utc = Some(now);
            }

            cp.last_data_watermark_hash = Some(watermark_hash.clone());
            cp.total_cycles_executed += 1;
            cp.updated_at_utc = now;
            store.save_checkpoint(&cp)?;

            if let Ok(mut st) = status_arc.write() {
                st.checkpoint = cp;
                st.last_error = None;
            }

            Ok(ContinuousCycleOutcome {
                executed_at_utc: now,
                as_of_date: as_of_str,
                data_watermark_hash: watermark_hash,
                skipped_unchanged_watermark: false,
                persisted_run_ids,
                emitted_transition_ids,
                prospective_oos_persisted: oos_persisted,
                compacted_files,
            })
        })
        .await
        .map_err(|e| format!("continuous supervisor join error: {}", e))?
    }
}
