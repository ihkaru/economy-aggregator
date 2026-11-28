//! Core Data Contracts for Continuous Analytics, Regime Transitions, Atomic Checkpoints,
//! and Frozen-Protocol Prospective OOS Scorecards (Milestone 18).

use std::collections::BTreeMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const CONTINUOUS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    InitialBaseline,
    LeafChanged,
    RootRegimeChanged,
    BlindSpotOpened,
    BlindSpotResolved,
    StaleSourceFreshened,
    ProtocolUpgraded,
    MetricsUpdatedSameLeaf,
}

impl TransitionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InitialBaseline => "initial_baseline",
            Self::LeafChanged => "leaf_changed",
            Self::RootRegimeChanged => "root_regime_changed",
            Self::BlindSpotOpened => "blind_spot_opened",
            Self::BlindSpotResolved => "blind_spot_resolved",
            Self::StaleSourceFreshened => "stale_source_freshened",
            Self::ProtocolUpgraded => "protocol_upgraded",
            Self::MetricsUpdatedSameLeaf => "metrics_updated_same_leaf",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricShiftEntry {
    pub node_id: String,
    pub metric_key: String,
    pub previous_value: Option<f64>,
    pub current_value: Option<f64>,
    pub delta: Option<f64>,
}

/// Event-sourced record emitted whenever a continuous diagnostic pass changes state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeTransitionEvent {
    pub schema_version: u32,
    pub transition_id: String,
    pub detected_at_utc: DateTime<Utc>,
    pub as_of_date: String,
    pub window_weeks: usize,
    pub protocol_version: String,
    pub transition_kind: TransitionKind,
    pub secondary_kinds: Vec<TransitionKind>,
    pub from_run_id: Option<String>,
    pub to_run_id: String,
    pub from_root_regime: Option<String>,
    pub to_root_regime: String,
    pub from_leaf_id: Option<String>,
    pub to_leaf_id: String,
    pub from_epistemic_tier: Option<String>,
    pub to_epistemic_tier: String,
    pub from_checksum_sha256: Option<String>,
    pub to_checksum_sha256: String,
    pub opened_blind_spots: Vec<String>,
    pub resolved_blind_spots: Vec<String>,
    pub newly_freshened_sources: Vec<String>,
    pub newly_stale_sources: Vec<String>,
    pub metric_shifts: Vec<MetricShiftEntry>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowCheckpoint {
    pub window_weeks: usize,
    pub last_run_id: String,
    pub last_as_of_date: String,
    pub last_evaluated_at_utc: DateTime<Utc>,
    pub last_protocol_version: String,
    pub last_root_regime: String,
    pub last_terminal_leaf_id: String,
    pub last_checksum_sha256: String,
}

/// Atomic CQRS consumer checkpoint persisted to `lake/diagnostics/checkpoints/continuous_analytics_state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousAnalyticsCheckpoint {
    pub schema_version: u32,
    pub updated_at_utc: DateTime<Utc>,
    pub last_data_watermark_hash: Option<String>,
    pub total_cycles_executed: u64,
    pub total_runs_persisted: u64,
    pub total_transitions_emitted: u64,
    pub last_oos_week_scored: Option<String>,
    pub last_compaction_utc: Option<DateTime<Utc>>,
    pub windows: BTreeMap<usize, WindowCheckpoint>,
}

impl Default for ContinuousAnalyticsCheckpoint {
    fn default() -> Self {
        Self {
            schema_version: CONTINUOUS_SCHEMA_VERSION,
            updated_at_utc: Utc::now(),
            last_data_watermark_hash: None,
            total_cycles_executed: 0,
            total_runs_persisted: 0,
            total_transitions_emitted: 0,
            last_oos_week_scored: None,
            last_compaction_utc: None,
            windows: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousCycleOutcome {
    pub executed_at_utc: DateTime<Utc>,
    pub as_of_date: String,
    pub data_watermark_hash: String,
    pub skipped_unchanged_watermark: bool,
    pub persisted_run_ids: Vec<String>,
    pub emitted_transition_ids: Vec<String>,
    pub prospective_oos_persisted: bool,
    pub compacted_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousSupervisorStatus {
    pub is_running: bool,
    pub check_interval_secs: u64,
    pub tracked_windows: Vec<usize>,
    pub last_error: Option<String>,
    pub checkpoint: ContinuousAnalyticsCheckpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProspectiveRuleTracker {
    pub rule_id: String,
    pub family: String,
    pub rule_class: String,
    pub readout_status: String,
    pub target_readout_weeks: usize,
    pub oos_observations_collected: usize,
    pub trigger_count: usize,
    pub hit_count: Option<usize>,
    pub running_metric_label: String,
    pub running_metric_value: Option<f64>,
    pub comparator_value: Option<f64>,
    pub progress_pct: f64,
    pub governance_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProspectiveScorecardReport {
    pub schema_version: u32,
    pub scorecard_id: String,
    pub evaluated_at_utc: DateTime<Utc>,
    pub as_of_date: String,
    pub protocol_version: String,
    pub protocol_anchor_tag: String,
    pub protocol_sha256: String,
    pub burn_in_end_week: String,
    pub oos_start_week: String,
    pub primary_readout_date_utc: String,
    pub secondary_readout_date_utc: String,
    pub latest_panel_week: Option<String>,
    pub oos_weeks_collected: usize,
    pub primary_weeks_remaining: usize,
    pub secondary_weeks_remaining: usize,
    pub optional_stopping_forbidden: bool,
    pub overall_readout_status: String,
    pub primary_rules_m2: Vec<ProspectiveRuleTracker>,
    pub secondary_rules_m5: Vec<ProspectiveRuleTracker>,
    pub scorecard_checksum_sha256: String,
    pub persisted_path: Option<String>,
}
