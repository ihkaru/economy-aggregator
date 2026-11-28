//! Pure-Rust Deterministic Liquidity Diagnostic Decision Tree Engine (`liquidity-tree-v1.1.0`).
//! Enforces strict epistemic discipline:
//! - Pure accounting & co-movement wording (zero unobserved motives such as "dip-buying",
//!   "profit-taking", or "defensive" without controlling for short-term yields).
//! - Price-neutral derivatives leverage (BTC OI in contracts + 8h perpetual funding rate).
//! - Explicit H.8 / FINRA / WRMFNS / COT staleness disclosure so unprinted weeks are never extrapolated.

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

pub const TREE_PROTOCOL_VERSION: &str = "liquidity-tree-v1.1.0";

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
    /// Nominal USD OI change (affected by BTC price revaluation; kept strictly for transparency).
    pub delta_btc_oi_usd: Option<f64>,
    /// Price-neutral BTC perpetual OI change in contracts (Bybit V5).
    pub delta_btc_oi_contracts: Option<f64>,
    pub delta_btc_oi_contracts_pct: Option<f64>,
    /// Perpetual 8h funding rate in basis points (window average & latest print).
    pub btc_funding_rate_8h_avg_bps: Option<f64>,
    pub btc_funding_rate_latest_bps: Option<f64>,
    /// Short-term interest rate context (needed to separate yield/carry from cash hoarding).
    pub sofr_latest_pct: Option<f64>,
    pub dtb3_latest_pct: Option<f64>,
    /// Daily/weekly credit & stress indicators bridging Fed H.8 reporting lag.
    pub hy_oas_latest_pct: Option<f64>,
    pub delta_hy_oas_bps: Option<f64>,
    pub stlfsi4_latest: Option<f64>,
    /// CFTC Disaggregated Gold Managed Money Net positioning delta (contracts).
    pub cot_gold_managed_money_delta: Option<f64>,
    pub cot_gold_is_stale: bool,
    /// True only if Fed H.8 latest date covers the latest observed equity candle date.
    pub h8_covers_latest_market_date: bool,
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

/// Evaluates the deterministic diagnostic tree (`v1.1.0`) and returns a fully backtrackable report.
pub fn evaluate_liquidity_tree(inputs: DiagnosticTreeInputs) -> DiagnosticTreeReport {
    let m = &inputs.metrics;
    let as_of_str = inputs.as_of_date.format("%Y-%m-%d").to_string();
    let mut nodes: Vec<DiagnosticNode> = Vec::new();
    let mut active_path: Vec<String> = Vec::new();
    let mut blind_spots: Vec<UnansweredQuestion> = Vec::new();

    let scale = (inputs.window_weeks.max(1) as f64).sqrt();
    let flow_thr = 250_000_000.0 * scale;

    // -------------------------------------------------------------------------
    // Q0: Window Price Direction Snapshot (Tier 1 direct price observation)
    // -------------------------------------------------------------------------
    let spy_down = m.spy_return_pct.is_some_and(|r| r <= -2.0);
    let btc_down = m.btc_return_pct.is_some_and(|r| r <= -5.0);
    let is_drawdown = spy_down || btc_down;
    let root_regime = if spy_down && btc_down {
        "drawdown_equities_and_crypto"
    } else if spy_down {
        "drawdown_equities"
    } else if btc_down {
        "drawdown_crypto"
    } else {
        "non_drawdown_window"
    };
    let q0_status = if m.spy_return_pct.is_none() && m.btc_return_pct.is_none() {
        NodeEvalStatus::InsufficientData
    } else if is_drawdown {
        NodeEvalStatus::True
    } else {
        NodeEvalStatus::False
    };
    active_path.push("Q0_MARKET_REGIME".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q0_MARKET_REGIME".to_string(),
        parent_id: None,
        question: "Did SPY (<= -2%) or BTCUSDT (<= -5%) post a price drawdown over the active window?".to_string(),
        epistemic_tier: EpistemicTier::Tier1Accounting,
        status: q0_status,
        branch_taken: if is_drawdown { "drawdown".to_string() } else { "non_drawdown".to_string() },
        predicate_formula: "spy_return_pct <= -2.0 OR btc_return_pct <= -5.0".to_string(),
        metrics: serde_json::json!({
            "spy_return_pct": m.spy_return_pct,
            "btc_return_pct": m.btc_return_pct,
            "window_weeks": inputs.window_weeks,
        }),
        summary: format!(
            "Single-window snapshot ({}w ending {}): SPY={}, BTC={} -> branch '{}'.",
            inputs.window_weeks,
            as_of_str,
            fmt_opt_pct(m.spy_return_pct),
            fmt_opt_pct(m.btc_return_pct),
            root_regime
        ),
        input_series: vec!["candle:SPY:1d".to_string(), "candle:BTCUSDT:1d".to_string()],
        limitations: vec![
            "Descriptive snapshot of a single rolling window, not evidence of a persistent regime.".to_string(),
            "Yahoo Finance SPY 1d candles are upstream split-adjusted (dividends excluded).".to_string(),
        ],
    });

    let risk_etf_flow = [m.spy_exact_flow_usd, m.jnk_exact_flow_usd, m.btc_etf_flow_usd]
        .into_iter()
        .flatten()
        .sum::<f64>();
    let has_risk_flow = m.spy_exact_flow_usd.is_some()
        || m.jnk_exact_flow_usd.is_some()
        || m.btc_etf_flow_usd.is_some();

    let (terminal_leaf_id, terminal_tier, terminal_verdict) = if is_drawdown {
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
    // Q6: Non-Causal Systemic & Credit Context Gauge (Tier 4 attached to every run)
    // -------------------------------------------------------------------------
    let elevated_context = m.spillover_ex_rrp_z.is_some_and(|z| z >= 2.0)
        || m.move_latest.is_some_and(|mv| mv > 110.0)
        || m.stlfsi4_latest.is_some_and(|s| s >= 0.5);
    active_path.push("Q6_SYSTEMIC_CONTEXT_GAUGE".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q6_SYSTEMIC_CONTEXT_GAUGE".to_string(),
        parent_id: Some(terminal_leaf_id.clone()),
        question: "Context gauge (non-causal): What do bond volatility, HY spreads, STLFSI4, and ex-RRP connectedness show?".to_string(),
        epistemic_tier: EpistemicTier::Tier4HypothesisGauge,
        status: if m.spillover_ex_rrp_z.is_none() && m.move_latest.is_none() && m.hy_oas_latest_pct.is_none() {
            NodeEvalStatus::InsufficientData
        } else if elevated_context {
            NodeEvalStatus::True
        } else {
            NodeEvalStatus::False
        },
        branch_taken: if elevated_context { "elevated_co_movement_context".to_string() } else { "calm_context".to_string() },
        predicate_formula: "ex_rrp_circular_shift_z >= 2.0 OR ^MOVE > 110 OR STLFSI4 >= 0.5".to_string(),
        metrics: serde_json::json!({
            "move_latest": m.move_latest,
            "hy_oas_latest_pct": m.hy_oas_latest_pct,
            "delta_hy_oas_bps": m.delta_hy_oas_bps,
            "stlfsi4_latest": m.stlfsi4_latest,
            "spillover_ex_rrp_z": m.spillover_ex_rrp_z,
            "spillover_ex_rrp_bias_adj_pct": m.spillover_ex_rrp_bias_adj_pct,
            "spillover_wild_break_p": m.spillover_wild_break_p,
        }),
        summary: format!(
            "Non-causal context: ^MOVE={}, HY_OAS={}% (d={}bps), STLFSI4={}, ex-RRP circular-shift TCI bias_adj={}% (z={}, wild_break_p={}).",
            fmt_opt_f(m.move_latest, 1),
            fmt_opt_f(m.hy_oas_latest_pct, 2),
            fmt_opt_f(m.delta_hy_oas_bps, 0),
            fmt_opt_f(m.stlfsi4_latest, 2),
            fmt_opt_f(m.spillover_ex_rrp_bias_adj_pct, 2),
            fmt_opt_f(m.spillover_ex_rrp_z, 2),
            fmt_opt_f(m.spillover_wild_break_p, 3),
        ),
        input_series: vec![
            "candle:^MOVE:1d".to_string(),
            "macro:US:FRED_HY_OAS_PCT".to_string(),
            "macro:US:FRED_STLFSI4_IDX".to_string(),
            "derived:spillover:ex_rrp".to_string(),
        ],
        limitations: vec![
            "Tier 4 exploratory context only: circular-shift rejection reflects non-independence (including shared macro shocks), never directional causality or regime proof.".to_string(),
        ],
    });

    if m.finra_is_stale {
        blind_spots.push(UnansweredQuestion {
            triggered_by_node: "Q3_CREDIT_AND_MACRO_CONTRACTION".to_string(),
            question: "Did US equity margin debit balances contract during the current unprinted month?".to_string(),
            missing_data_gap: "FINRA Margin Debt is published monthly with ~21-day reporting lag (STALE_LAG for active window).".to_string(),
            candidate_open_sources: vec!["FINRA next monthly release".to_string()],
            governance_rule: "Never extrapolate stale monthly margin figures onto current-week price moves.".to_string(),
        });
    }
    if !m.h8_covers_latest_market_date {
        blind_spots.push(UnansweredQuestion {
            triggered_by_node: "Q3_CREDIT_AND_MACRO_CONTRACTION".to_string(),
            question: "How did US commercial bank deposits and credit respond to the most recent week's market move?".to_string(),
            missing_data_gap: "Federal Reserve H.8 (TOTBKCR & DPSACBW027SBOG) lags by ~9-10 days and has not yet printed for the latest market date.".to_string(),
            candidate_open_sources: vec!["Next Friday Federal Reserve H.8 release via FRED".to_string()],
            governance_rule: "State only 'no banking stress observed in released data so far'; never assert zero banking impact for unprinted weeks.".to_string(),
        });
    }

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
