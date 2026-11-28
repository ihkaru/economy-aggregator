//! Pure-Rust Deterministic Liquidity Diagnostic Decision Tree Engine (Phase 347).
//! Traverses frozen epistemic rules (Tier 1 Accounting -> Tier 2 Replicated -> Tier 3 Regime ->
//! Tier 4 Gauge / Blind-Spot Escalation) with full source-to-leaf backtrack lineage.

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::core::diagnostic_types::{
    DiagnosticNode, DiagnosticTreeReport, EpistemicTier, NodeEvalStatus, SourceLineageEntry,
    UnansweredQuestion,
};
use crate::core::observation_types::sha256_hex;
use crate::features::analytics::liquidity_tree_branches::{
    evaluate_risk_off_branch, evaluate_risk_on_branch, fmt_opt_pct,
};

pub const TREE_PROTOCOL_VERSION: &str = "liquidity-tree-v1.0.0";

#[derive(Debug, Clone, Default)]
pub struct DiagnosticTreeMetrics {
    pub spy_return_pct: Option<f64>,
    pub btc_return_pct: Option<f64>,
    pub move_latest: Option<f64>,
    pub spy_delta_aum_usd: Option<f64>,
    pub spy_exact_flow_usd: Option<f64>,
    pub gld_exact_flow_usd: Option<f64>,
    pub bil_exact_flow_usd: Option<f64>,
    pub jnk_exact_flow_usd: Option<f64>,
    pub btc_etf_flow_usd: Option<f64>,
    pub delta_wrmfns_usd: Option<f64>,
    pub delta_wrmfns_pct: Option<f64>,
    pub delta_bank_deposits_usd: Option<f64>,
    pub delta_bank_deposits_pct: Option<f64>,
    pub delta_bank_credit_usd: Option<f64>,
    pub delta_m2_usd: Option<f64>,
    pub delta_walcl_minus_tga_usd: Option<f64>,
    pub delta_on_rrp_usd: Option<f64>,
    pub delta_stablecoin_usd: Option<f64>,
    pub delta_btc_oi_usd: Option<f64>,
    pub finra_margin_delta_pct: Option<f64>,
    pub finra_is_stale: bool,
    pub spillover_ex_rrp_z: Option<f64>,
    pub spillover_ex_rrp_bias_adj_pct: Option<f64>,
    pub spillover_wild_break_p: Option<f64>,
}

pub struct DiagnosticTreeInputs {
    pub as_of_date: NaiveDate,
    pub window_weeks: usize,
    pub metrics: DiagnosticTreeMetrics,
    pub source_lineage: Vec<SourceLineageEntry>,
}

fn fmt_opt_f(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.prec$}", x, prec = decimals),
        _ => "N/A".to_string(),
    }
}

/// Evaluates the deterministic diagnostic tree and returns a fully backtrackable report.
pub fn evaluate_liquidity_tree(inputs: DiagnosticTreeInputs) -> DiagnosticTreeReport {
    let m = &inputs.metrics;
    let as_of_str = inputs.as_of_date.format("%Y-%m-%d").to_string();
    let mut nodes: Vec<DiagnosticNode> = Vec::new();
    let mut active_path: Vec<String> = Vec::new();
    let mut blind_spots: Vec<UnansweredQuestion> = Vec::new();

    // Scale USD thresholds mildly by sqrt(window_weeks) so 1w and 4w remain comparable yet fixed.
    let scale = (inputs.window_weeks.max(1) as f64).sqrt();
    let flow_thr = 250_000_000.0 * scale;

    // -------------------------------------------------------------------------
    // Q0: Root Market Direction Regime (Tier 1 direct price observation)
    // -------------------------------------------------------------------------
    let spy_down = m.spy_return_pct.is_some_and(|r| r <= -2.0);
    let btc_down = m.btc_return_pct.is_some_and(|r| r <= -5.0);
    let is_risk_off = spy_down || btc_down;
    let root_regime = if spy_down && btc_down {
        "risk_off_broad_equities_and_crypto"
    } else if spy_down {
        "risk_off_equities"
    } else if btc_down {
        "risk_off_crypto"
    } else {
        "risk_on_or_neutral"
    };
    let q0_status = if m.spy_return_pct.is_none() && m.btc_return_pct.is_none() {
        NodeEvalStatus::InsufficientData
    } else if is_risk_off {
        NodeEvalStatus::True
    } else {
        NodeEvalStatus::False
    };
    active_path.push("Q0_MARKET_REGIME".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q0_MARKET_REGIME".to_string(),
        parent_id: None,
        question: "Are US equities (SPY) or Bitcoin (BTCUSDT) in a drawdown over the active window?".to_string(),
        epistemic_tier: EpistemicTier::Tier1Accounting,
        status: q0_status,
        branch_taken: if is_risk_off { "risk_off".to_string() } else { "risk_on_or_neutral".to_string() },
        predicate_formula: "spy_return_pct <= -2.0 OR btc_return_pct <= -5.0".to_string(),
        metrics: serde_json::json!({
            "spy_return_pct": m.spy_return_pct,
            "btc_return_pct": m.btc_return_pct,
            "window_weeks": inputs.window_weeks,
        }),
        summary: format!(
            "Window {}w ending {}: SPY={}, BTC={} -> branch '{}'.",
            inputs.window_weeks,
            as_of_str,
            fmt_opt_pct(m.spy_return_pct),
            fmt_opt_pct(m.btc_return_pct),
            root_regime
        ),
        input_series: vec!["candle:SPY:1d".to_string(), "candle:BTCUSDT:1d".to_string()],
        limitations: vec![
            "Yahoo Finance SPY 1d candles are upstream split-adjusted (not dividend-adjusted).".to_string(),
        ],
    });

    // Aggregate primary risk ETF flows (exact share creation + BTC Spot ETF USD)
    let risk_etf_flow = [m.spy_exact_flow_usd, m.jnk_exact_flow_usd, m.btc_etf_flow_usd]
        .into_iter()
        .flatten()
        .sum::<f64>();
    let has_risk_flow = m.spy_exact_flow_usd.is_some()
        || m.jnk_exact_flow_usd.is_some()
        || m.btc_etf_flow_usd.is_some();

    let (terminal_leaf_id, terminal_tier, terminal_verdict) = if is_risk_off {
        evaluate_risk_off_branch(
            m,
            flow_thr,
            risk_etf_flow,
            has_risk_flow,
            &mut nodes,
            &mut active_path,
            &mut blind_spots,
        )
    } else {
        evaluate_risk_on_branch(
            m,
            flow_thr,
            risk_etf_flow,
            has_risk_flow,
            &mut nodes,
            &mut active_path,
            &mut blind_spots,
        )
    };

    // -------------------------------------------------------------------------
    // Q6: Non-Causal Systemic Context Gauge (Tier 4 attached to every run)
    // -------------------------------------------------------------------------
    let elevated_context = m.spillover_ex_rrp_z.is_some_and(|z| z >= 2.0)
        || m.move_latest.is_some_and(|mv| mv > 110.0);
    active_path.push("Q6_SYSTEMIC_CONTEXT_GAUGE".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q6_SYSTEMIC_CONTEXT_GAUGE".to_string(),
        parent_id: Some(terminal_leaf_id.clone()),
        question: "Context gauge (non-causal): Is cross-compartment connectedness or bond volatility elevated?".to_string(),
        epistemic_tier: EpistemicTier::Tier4HypothesisGauge,
        status: if m.spillover_ex_rrp_z.is_none() && m.move_latest.is_none() {
            NodeEvalStatus::InsufficientData
        } else if elevated_context {
            NodeEvalStatus::True
        } else {
            NodeEvalStatus::False
        },
        branch_taken: if elevated_context { "elevated_co_movement_context".to_string() } else { "calm_context".to_string() },
        predicate_formula: "ex_rrp_circular_shift_z >= 2.0 OR ^MOVE > 110".to_string(),
        metrics: serde_json::json!({
            "move_latest": m.move_latest,
            "spillover_ex_rrp_z": m.spillover_ex_rrp_z,
            "spillover_ex_rrp_bias_adj_pct": m.spillover_ex_rrp_bias_adj_pct,
            "spillover_wild_break_p": m.spillover_wild_break_p,
        }),
        summary: format!(
            "Non-causal context: ^MOVE={}, ex-RRP circular-shift TCI bias_adj={}% (z={}, wild_break_p={}).",
            fmt_opt_f(m.move_latest, 1),
            fmt_opt_f(m.spillover_ex_rrp_bias_adj_pct, 2),
            fmt_opt_f(m.spillover_ex_rrp_z, 2),
            fmt_opt_f(m.spillover_wild_break_p, 3),
        ),
        input_series: vec!["candle:^MOVE:1d".to_string(), "derived:spillover:ex_rrp".to_string()],
        limitations: vec![
            "Tier 4 exploratory gauge only: circular-shift rejection reflects non-independence (including shared macro shocks), never directional causality.".to_string(),
        ],
    });

    // Append structural blind spot notice whenever monthly FINRA margin data has not printed for the active window.
    if m.finra_is_stale {
        blind_spots.push(UnansweredQuestion {
            triggered_by_node: "Q3_CREDIT_AND_MACRO_CONTRACTION".to_string(),
            question: "Did US equity margin debit balances contract during the current unprinted month?".to_string(),
            missing_data_gap: "FINRA Margin Debt is published monthly with ~21-day reporting lag (currently STALE_LAG for the active window).".to_string(),
            candidate_open_sources: vec!["FINRA next monthly release".to_string()],
            governance_rule: "Never extrapolate stale monthly margin figures onto current-week price moves.".to_string(),
        });
    }

    // Deterministic checksum over protocol, window, active path, and node JSON.
    let hash_payload = serde_json::json!({
        "protocol_version": TREE_PROTOCOL_VERSION,
        "as_of_date": &as_of_str,
        "window_weeks": inputs.window_weeks,
        "active_path": &active_path,
        "terminal_leaf_id": &terminal_leaf_id,
        "nodes": &nodes,
    });
    let checksum = sha256_hex(hash_payload.to_string().as_bytes());
    let run_id = format!("tree_{}_{}", as_of_str.replace('-', ""), &Uuid::new_v4().to_string()[..8]);

    DiagnosticTreeReport {
        run_id,
        protocol_version: TREE_PROTOCOL_VERSION.to_string(),
        evaluated_at_utc: Utc::now(),
        as_of_date: as_of_str,
        window_weeks: inputs.window_weeks,
        root_regime: root_regime.to_string(),
        terminal_leaf_id,
        terminal_epistemic_tier: terminal_tier,
        terminal_verdict,
        active_path,
        nodes,
        blind_spots,
        source_lineage: inputs.source_lineage,
        tree_checksum_sha256: checksum,
        persisted_path: None,
    }
}
