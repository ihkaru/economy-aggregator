//! Domain Models for Deterministic Liquidity Diagnostic Tree & Persistent Backtrack Lineage (Phase 347).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::core::observation_types::DataProcessingLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicTier {
    /// Accounting identity or direct measurement (revaluation vs exact unit creation, raw stock levels).
    Tier1Accounting,
    /// Replicated across 30 years & unseen 1997-2015 holdout (WRMFNS 3-wk post-crash, FINRA pro-cyclicality).
    Tier2Replicated,
    /// Modern-regime empirical co-movement (2016-2026 BIL/JNK/BTC ETF; not yet replicated across eras).
    Tier3RegimeDescriptive,
    /// Exploratory / non-causal hypothesis gauge (MOVE->BIL, reduced-form VAR circular-shift TCI).
    Tier4HypothesisGauge,
    /// Explicit admission that observed sensors cannot account for capital movement.
    BlindSpot,
}

impl EpistemicTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tier1Accounting => "tier1_accounting",
            Self::Tier2Replicated => "tier2_replicated",
            Self::Tier3RegimeDescriptive => "tier3_regime_descriptive",
            Self::Tier4HypothesisGauge => "tier4_hypothesis_gauge",
            Self::BlindSpot => "blind_spot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeEvalStatus {
    True,
    False,
    StaleLag,
    InsufficientData,
}

/// Complete source-level backtrack record linking any diagnostic node back to raw upstream origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLineageEntry {
    pub series_id: String,
    pub source: String,
    pub source_series_id: String,
    pub source_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_checksum: Option<String>,
    pub batch_id: String,
    pub data_layer: DataProcessingLayer,
    pub raw_value_preserved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adjustment_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_limitation: Option<String>,
    pub latest_observation_utc: Option<String>,
    pub stale_lag_days: Option<i64>,
    pub is_stale: bool,
    pub used_by_nodes: Vec<String>,
}

/// One deterministic question node evaluated during diagnostic traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticNode {
    pub node_id: String,
    pub parent_id: Option<String>,
    pub question: String,
    pub epistemic_tier: EpistemicTier,
    pub status: NodeEvalStatus,
    pub branch_taken: String,
    pub predicate_formula: String,
    pub metrics: serde_json::Value,
    pub summary: String,
    pub input_series: Vec<String>,
    pub limitations: Vec<String>,
}

/// Structured follow-up question emitted when a tree hits a blind spot or stale data gap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnansweredQuestion {
    pub triggered_by_node: String,
    pub question: String,
    pub missing_data_gap: String,
    pub candidate_open_sources: Vec<String>,
    pub governance_rule: String,
}

/// Complete deterministic diagnostic traversal report (persisted losslessly in Parquet ledger).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticTreeReport {
    pub run_id: String,
    pub protocol_version: String,
    pub evaluated_at_utc: DateTime<Utc>,
    pub as_of_date: String,
    pub window_weeks: usize,
    pub root_regime: String,
    pub terminal_leaf_id: String,
    pub terminal_epistemic_tier: EpistemicTier,
    pub terminal_verdict: String,
    pub active_path: Vec<String>,
    pub nodes: Vec<DiagnosticNode>,
    pub blind_spots: Vec<UnansweredQuestion>,
    pub source_lineage: Vec<SourceLineageEntry>,
    pub tree_checksum_sha256: String,
    pub persisted_path: Option<String>,
}

/// Compact summary returned by history listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticTreeRunSummary {
    pub run_id: String,
    pub evaluated_at_utc: DateTime<Utc>,
    pub as_of_date: String,
    pub window_weeks: usize,
    pub protocol_version: String,
    pub root_regime: String,
    pub terminal_leaf_id: String,
    pub terminal_epistemic_tier: String,
    pub blind_spot_count: usize,
    pub stale_source_count: usize,
    pub tree_checksum_sha256: String,
}
