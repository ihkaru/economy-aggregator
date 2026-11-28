//! Pure Functional Regime Transition & Blind-Spot Diff Engine (Phase 352).
//! Compares consecutive DiagnosticTreeReports without I/O to emit immutable event-sourced transitions.

use std::collections::{BTreeMap, BTreeSet};
use chrono::Utc;
use uuid::Uuid;

use crate::core::continuous_types::{
    MetricShiftEntry, RegimeTransitionEvent, TransitionKind, CONTINUOUS_SCHEMA_VERSION,
};
use crate::core::diagnostic_types::DiagnosticTreeReport;

/// Detects material state transitions between a previous and current diagnostic tree snapshot.
/// Returns `None` when the snapshots are byte-for-byte identical in checksum, date, and protocol.
pub fn detect_regime_transition(
    prev: Option<&DiagnosticTreeReport>,
    curr: &DiagnosticTreeReport,
) -> Option<RegimeTransitionEvent> {
    let tid = format!(
        "tr_{}_{}",
        curr.as_of_date.replace('-', ""),
        &Uuid::new_v4().to_string()[..8]
    );

    let Some(p) = prev else {
        return Some(RegimeTransitionEvent {
            schema_version: CONTINUOUS_SCHEMA_VERSION,
            transition_id: tid,
            detected_at_utc: Utc::now(),
            as_of_date: curr.as_of_date.clone(),
            window_weeks: curr.window_weeks,
            protocol_version: curr.protocol_version.clone(),
            transition_kind: TransitionKind::InitialBaseline,
            secondary_kinds: Vec::new(),
            from_run_id: None,
            to_run_id: curr.run_id.clone(),
            from_root_regime: None,
            to_root_regime: curr.root_regime.clone(),
            from_leaf_id: None,
            to_leaf_id: curr.terminal_leaf_id.clone(),
            from_epistemic_tier: None,
            to_epistemic_tier: curr.terminal_epistemic_tier.as_str().to_string(),
            from_checksum_sha256: None,
            to_checksum_sha256: curr.tree_checksum_sha256.clone(),
            opened_blind_spots: blind_spot_keys(curr).into_iter().collect(),
            resolved_blind_spots: Vec::new(),
            newly_freshened_sources: Vec::new(),
            newly_stale_sources: curr
                .source_lineage
                .iter()
                .filter(|s| s.is_stale)
                .map(|s| s.series_id.clone())
                .collect(),
            metric_shifts: Vec::new(),
            summary: format!(
                "Initial {}w baseline established on {} -> {} ({}).",
                curr.window_weeks, curr.as_of_date, curr.terminal_leaf_id, curr.root_regime
            ),
        });
    };

    if p.tree_checksum_sha256 == curr.tree_checksum_sha256
        && p.as_of_date == curr.as_of_date
        && p.protocol_version == curr.protocol_version
    {
        return None;
    }

    let prev_bs = blind_spot_keys(p);
    let curr_bs = blind_spot_keys(curr);
    let opened_blind_spots: Vec<String> = curr_bs.difference(&prev_bs).cloned().collect();
    let resolved_blind_spots: Vec<String> = prev_bs.difference(&curr_bs).cloned().collect();

    let prev_stale: BTreeMap<&str, bool> = p
        .source_lineage
        .iter()
        .map(|s| (s.series_id.as_str(), s.is_stale))
        .collect();
    let mut newly_freshened_sources = Vec::new();
    let mut newly_stale_sources = Vec::new();
    for s in &curr.source_lineage {
        if let Some(&was_stale) = prev_stale.get(s.series_id.as_str()) {
            if was_stale && !s.is_stale {
                newly_freshened_sources.push(s.series_id.clone());
            } else if !was_stale && s.is_stale {
                newly_stale_sources.push(s.series_id.clone());
            }
        }
    }

    let metric_shifts = compute_metric_shifts(p, curr);

    let mut kinds = Vec::new();
    if p.terminal_leaf_id != curr.terminal_leaf_id {
        kinds.push(TransitionKind::LeafChanged);
    }
    if p.root_regime != curr.root_regime {
        kinds.push(TransitionKind::RootRegimeChanged);
    }
    if p.protocol_version != curr.protocol_version {
        kinds.push(TransitionKind::ProtocolUpgraded);
    }
    if !opened_blind_spots.is_empty() {
        kinds.push(TransitionKind::BlindSpotOpened);
    }
    if !resolved_blind_spots.is_empty() {
        kinds.push(TransitionKind::BlindSpotResolved);
    }
    if !newly_freshened_sources.is_empty() {
        kinds.push(TransitionKind::StaleSourceFreshened);
    }
    if kinds.is_empty() {
        kinds.push(TransitionKind::MetricsUpdatedSameLeaf);
    }

    let primary = kinds[0];
    let secondary = kinds[1..].to_vec();
    let summary = build_transition_summary(
        p,
        curr,
        primary,
        &opened_blind_spots,
        &resolved_blind_spots,
        &newly_freshened_sources,
        metric_shifts.len(),
    );

    Some(RegimeTransitionEvent {
        schema_version: CONTINUOUS_SCHEMA_VERSION,
        transition_id: tid,
        detected_at_utc: Utc::now(),
        as_of_date: curr.as_of_date.clone(),
        window_weeks: curr.window_weeks,
        protocol_version: curr.protocol_version.clone(),
        transition_kind: primary,
        secondary_kinds: secondary,
        from_run_id: Some(p.run_id.clone()),
        to_run_id: curr.run_id.clone(),
        from_root_regime: Some(p.root_regime.clone()),
        to_root_regime: curr.root_regime.clone(),
        from_leaf_id: Some(p.terminal_leaf_id.clone()),
        to_leaf_id: curr.terminal_leaf_id.clone(),
        from_epistemic_tier: Some(p.terminal_epistemic_tier.as_str().to_string()),
        to_epistemic_tier: curr.terminal_epistemic_tier.as_str().to_string(),
        from_checksum_sha256: Some(p.tree_checksum_sha256.clone()),
        to_checksum_sha256: curr.tree_checksum_sha256.clone(),
        opened_blind_spots,
        resolved_blind_spots,
        newly_freshened_sources,
        newly_stale_sources,
        metric_shifts,
        summary,
    })
}

fn blind_spot_keys(rep: &DiagnosticTreeReport) -> BTreeSet<String> {
    rep.blind_spots
        .iter()
        .map(|b| format!("{}:{}", b.triggered_by_node, b.question))
        .collect()
}

fn compute_metric_shifts(
    prev: &DiagnosticTreeReport,
    curr: &DiagnosticTreeReport,
) -> Vec<MetricShiftEntry> {
    let prev_nodes: BTreeMap<&str, &serde_json::Value> = prev
        .nodes
        .iter()
        .map(|n| (n.node_id.as_str(), &n.metrics))
        .collect();
    let mut shifts = Vec::new();
    for node in &curr.nodes {
        let Some(curr_obj) = node.metrics.as_object() else { continue };
        let prev_obj = prev_nodes.get(node.node_id.as_str()).and_then(|v| v.as_object());
        for (k, cv) in curr_obj {
            let curr_num = cv.as_f64().filter(|x| x.is_finite());
            let prev_num = prev_obj.and_then(|o| o.get(k)).and_then(|v| v.as_f64()).filter(|x| x.is_finite());
            let changed = match (prev_num, curr_num) {
                (Some(a), Some(b)) => (b - a).abs() > 1e-6,
                (None, Some(_)) | (Some(_), None) => true,
                (None, None) => false,
            };
            if changed {
                shifts.push(MetricShiftEntry {
                    node_id: node.node_id.clone(),
                    metric_key: k.clone(),
                    previous_value: prev_num,
                    current_value: curr_num,
                    delta: prev_num.zip(curr_num).map(|(a, b)| b - a),
                });
            }
        }
    }
    shifts
}

fn build_transition_summary(
    prev: &DiagnosticTreeReport,
    curr: &DiagnosticTreeReport,
    primary: TransitionKind,
    opened: &[String],
    resolved: &[String],
    freshened: &[String],
    metric_shift_count: usize,
) -> String {
    match primary {
        TransitionKind::LeafChanged => format!(
            "{}w leaf shifted {} -> {} (root: {} -> {}, {} metric deltas).",
            curr.window_weeks,
            prev.terminal_leaf_id,
            curr.terminal_leaf_id,
            prev.root_regime,
            curr.root_regime,
            metric_shift_count
        ),
        TransitionKind::RootRegimeChanged => format!(
            "{}w root regime shifted {} -> {} (leaf remained {}).",
            curr.window_weeks, prev.root_regime, curr.root_regime, curr.terminal_leaf_id
        ),
        TransitionKind::BlindSpotOpened => format!(
            "{}w opened {} new blind spot(s) under {}.",
            curr.window_weeks, opened.len(), curr.terminal_leaf_id
        ),
        TransitionKind::BlindSpotResolved => format!(
            "{}w resolved {} blind spot(s) under {}.",
            curr.window_weeks, resolved.len(), curr.terminal_leaf_id
        ),
        TransitionKind::StaleSourceFreshened => format!(
            "{}w stale source(s) freshened: {} (leaf {}).",
            curr.window_weeks,
            freshened.join(", "),
            curr.terminal_leaf_id
        ),
        TransitionKind::ProtocolUpgraded => format!(
            "{}w diagnostic protocol upgraded {} -> {}.",
            curr.window_weeks, prev.protocol_version, curr.protocol_version
        ),
        _ => format!(
            "{}w metrics updated ({} deltas) while remaining in {}.",
            curr.window_weeks, metric_shift_count, curr.terminal_leaf_id
        ),
    }
}
