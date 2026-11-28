//! Deterministic Risk-Off and Risk-On/Neutral Branch Evaluators for the Liquidity Diagnostic Tree (Phase 347).

use crate::core::diagnostic_types::{
    DiagnosticNode, EpistemicTier, NodeEvalStatus, UnansweredQuestion,
};
use crate::features::analytics::liquidity_tree::DiagnosticTreeMetrics;

pub(crate) fn fmt_opt_b(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:+.2}B", x / 1e9),
        _ => "N/A".to_string(),
    }
}

pub(crate) fn fmt_opt_pct(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:+.2}%", x),
        _ => "N/A".to_string(),
    }
}

pub(crate) fn evaluate_risk_off_branch(
    m: &DiagnosticTreeMetrics,
    flow_thr: f64,
    risk_etf_flow: f64,
    has_risk_flow: bool,
    nodes: &mut Vec<DiagnosticNode>,
    active_path: &mut Vec<String>,
    blind_spots: &mut Vec<UnansweredQuestion>,
) -> (String, EpistemicTier, String) {
    // Q1: Accounting Revaluation vs Primary ETF Unit Redemption (Tier 1)
    let primary_outflow = has_risk_flow && risk_etf_flow < -flow_thr;
    active_path.push("Q1_ETF_REVAL_VS_PRIMARY".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q1_ETF_REVAL_VS_PRIMARY".to_string(),
        parent_id: Some("Q0_MARKET_REGIME".to_string()),
        question: "Did ETF AUM decline via actual primary share redemptions, or mostly price revaluation?".to_string(),
        epistemic_tier: EpistemicTier::Tier1Accounting,
        status: if !has_risk_flow {
            NodeEvalStatus::InsufficientData
        } else if primary_outflow {
            NodeEvalStatus::True
        } else {
            NodeEvalStatus::False
        },
        branch_taken: if primary_outflow {
            "primary_redemption_confirmed".to_string()
        } else {
            "revaluation_dominant".to_string()
        },
        predicate_formula: format!("(SPY_exact + JNK_exact + BTC_ETF_usd) < -{:.2}B", flow_thr / 1e9),
        metrics: serde_json::json!({
            "spy_delta_aum_usd": m.spy_delta_aum_usd,
            "spy_exact_flow_usd": m.spy_exact_flow_usd,
            "jnk_exact_flow_usd": m.jnk_exact_flow_usd,
            "btc_etf_flow_usd": m.btc_etf_flow_usd,
            "combined_primary_risk_flow_usd": risk_etf_flow,
        }),
        summary: format!(
            "Combined primary ETF risk flow={:+.2}B USD (SPY ΔAUM={} vs exact={}; JNK={}; BTC ETF={}).",
            risk_etf_flow / 1e9,
            fmt_opt_b(m.spy_delta_aum_usd),
            fmt_opt_b(m.spy_exact_flow_usd),
            fmt_opt_b(m.jnk_exact_flow_usd),
            fmt_opt_b(m.btc_etf_flow_usd),
        ),
        input_series: vec![
            "etf:SPY:aum_usd".to_string(),
            "etf:SPY:shares_outstanding".to_string(),
            "etf:JNK:shares_outstanding".to_string(),
            "etf:BTC_US:bgeometrics_flow_btc".to_string(),
        ],
        limitations: vec![
            "Covers primary AP creations/redemptions in tracked ETFs; secondary-market turnover without unit changes is zero by accounting identity.".to_string(),
        ],
    });

    // Q2: Defensive Cash Parking Tanks (BIL + WRMFNS + Bank Deposits)
    let bil_in = m.bil_exact_flow_usd.is_some_and(|v| v > flow_thr);
    let mmf_in = m.delta_wrmfns_pct.is_some_and(|p| p >= 0.15);
    let cash_parking_up = bil_in || mmf_in;
    let gld_out = m.gld_exact_flow_usd.is_some_and(|v| v < -0.6 * flow_thr);
    active_path.push("Q2_DEFENSIVE_CASH_PARKING".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q2_DEFENSIVE_CASH_PARKING".to_string(),
        parent_id: Some("Q1_ETF_REVAL_VS_PRIMARY".to_string()),
        question: "Are observed short-term cash parking tanks (BIL T-Bill, WRMFNS Retail MMF, Fed H.8 Bank Deposits) absorbing inflows?".to_string(),
        epistemic_tier: if mmf_in { EpistemicTier::Tier2Replicated } else { EpistemicTier::Tier3RegimeDescriptive },
        status: if m.bil_exact_flow_usd.is_none() && m.delta_wrmfns_pct.is_none() {
            NodeEvalStatus::InsufficientData
        } else if cash_parking_up {
            NodeEvalStatus::True
        } else {
            NodeEvalStatus::False
        },
        branch_taken: if cash_parking_up { "cash_tanks_absorbing".to_string() } else { "cash_tanks_flat_or_out".to_string() },
        predicate_formula: format!("BIL_exact > +{:.2}B OR dWRMFNS_pct >= +0.15%", flow_thr / 1e9),
        metrics: serde_json::json!({
            "bil_exact_flow_usd": m.bil_exact_flow_usd,
            "delta_wrmfns_usd": m.delta_wrmfns_usd,
            "delta_wrmfns_pct": m.delta_wrmfns_pct,
            "delta_bank_deposits_usd": m.delta_bank_deposits_usd,
            "delta_bank_deposits_pct": m.delta_bank_deposits_pct,
            "gld_exact_flow_usd": m.gld_exact_flow_usd,
            "dash_for_cash_gld_out_and_bil_in": gld_out && bil_in,
        }),
        summary: format!(
            "BIL={}, Retail MMF={} ({}), Bank Deposits={} ({}), GLD={}.",
            fmt_opt_b(m.bil_exact_flow_usd),
            fmt_opt_b(m.delta_wrmfns_usd),
            fmt_opt_pct(m.delta_wrmfns_pct),
            fmt_opt_b(m.delta_bank_deposits_usd),
            fmt_opt_pct(m.delta_bank_deposits_pct),
            fmt_opt_b(m.gld_exact_flow_usd),
        ),
        input_series: vec![
            "etf:BIL:shares_outstanding".to_string(),
            "macro:US:FRED_WRMFNS_USD".to_string(),
            "macro:US:FRED_BANK_DEPOSITS_USD".to_string(),
            "etf:GLD:shares_outstanding".to_string(),
        ],
        limitations: vec![
            "Institutional weekly MMF (WIMFNS) was discontinued by the Fed in Feb 2021; WRMFNS covers retail MMF only.".to_string(),
            "Fed H.8 Bank Deposits are upstream seasonally adjusted (SA) and reflect aggregate double-entry banking balances.".to_string(),
        ],
    });

    // Q3: Credit Contraction / Deleveraging & Macro Balance-Sheet Drain
    let oi_down = m.delta_btc_oi_usd.is_some_and(|v| v < -flow_thr);
    let finra_down = !m.finra_is_stale && m.finra_margin_delta_pct.is_some_and(|p| p <= -1.0);
    let macro_drain = m.delta_bank_credit_usd.is_some_and(|v| v < -10e9)
        || m.delta_walcl_minus_tga_usd.is_some_and(|v| v < -25e9);
    let credit_contracting = oi_down || finra_down || macro_drain;
    active_path.push("Q3_CREDIT_AND_MACRO_CONTRACTION".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q3_CREDIT_AND_MACRO_CONTRACTION".to_string(),
        parent_id: Some("Q2_DEFENSIVE_CASH_PARKING".to_string()),
        question: "Is observable leverage or macro credit/balance-sheet liquidity contracting?".to_string(),
        epistemic_tier: if finra_down { EpistemicTier::Tier2Replicated } else { EpistemicTier::Tier1Accounting },
        status: if m.finra_is_stale && !credit_contracting {
            NodeEvalStatus::StaleLag
        } else if credit_contracting {
            NodeEvalStatus::True
        } else {
            NodeEvalStatus::False
        },
        branch_taken: if credit_contracting {
            "credit_or_macro_contracting".to_string()
        } else {
            "no_observable_contraction".to_string()
        },
        predicate_formula: "dBTC_OI < -thr OR (fresh FINRA <= -1%) OR dBankCredit < -10B OR d(WALCL-TGA) < -25B".to_string(),
        metrics: serde_json::json!({
            "delta_btc_oi_usd": m.delta_btc_oi_usd,
            "finra_margin_delta_pct": m.finra_margin_delta_pct,
            "finra_is_stale": m.finra_is_stale,
            "delta_bank_credit_usd": m.delta_bank_credit_usd,
            "delta_walcl_minus_tga_usd": m.delta_walcl_minus_tga_usd,
            "delta_m2_usd": m.delta_m2_usd,
        }),
        summary: format!(
            "dBTC_OI={}, FINRA_margin={} (stale={}), dBankCredit={}, d(WALCL-TGA)={}.",
            fmt_opt_b(m.delta_btc_oi_usd),
            fmt_opt_pct(m.finra_margin_delta_pct),
            m.finra_is_stale,
            fmt_opt_b(m.delta_bank_credit_usd),
            fmt_opt_b(m.delta_walcl_minus_tga_usd),
        ),
        input_series: vec![
            "deriv:BTC:bgeometrics_oi_usd".to_string(),
            "credit:US:FINRA_MARGIN_DEBT_USD".to_string(),
            "macro:US:FRED_TOTBKCR_USD".to_string(),
            "macro:US:FRED_WALCL_USD".to_string(),
            "macro:US:TGA_BALANCE_USD".to_string(),
        ],
        limitations: vec![
            "FINRA Margin Debt is loan principal (not equity market value), but monthly release lags ~21 days and does not separate forced margin calls from voluntary paydowns.".to_string(),
        ],
    });

    if primary_outflow && cash_parking_up {
        (
            "LEAF_DEFENSIVE_CASH_ROTATION".to_string(),
            if mmf_in { EpistemicTier::Tier2Replicated } else { EpistemicTier::Tier3RegimeDescriptive },
            "Risk-off drawdown accompanied by primary ETF redemptions and inflows into observed cash tanks (BIL / Retail MMF); consistent with defensive cash rotation.".to_string(),
        )
    } else if credit_contracting {
        (
            "LEAF_CREDIT_CONTRACTION_AND_DELEVERAGING".to_string(),
            EpistemicTier::Tier2Replicated,
            "Risk-off drawdown coincides with observable leverage/credit or balance-sheet contraction (BTC OI / FINRA margin / bank credit / WALCL-TGA).".to_string(),
        )
    } else if !primary_outflow {
        (
            "LEAF_REVALUATION_DOMINANT_NO_EXODUS".to_string(),
            EpistemicTier::Tier1Accounting,
            "Price drawdown is predominantly mark-to-market revaluation in tracked ETFs without material primary unit redemption or observable credit contraction.".to_string(),
        )
    } else {
        blind_spots.push(UnansweredQuestion {
            triggered_by_node: "LEAF_BLIND_SPOT_UNOBSERVED_DESTINATION".to_string(),
            question: "Primary ETF risk outflows occurred while BIL, Retail MMF, Bank Deposits, and observable credit stayed flat — where did those funds settle?".to_string(),
            missing_data_gap: "Institutional Money Market Funds (weekly WIMFNS discontinued), non-SSGA ETF/mutual fund destinations, exchange-internal stablecoin reserves, and non-US capital flows are unobserved.".to_string(),
            candidate_open_sources: vec![
                "SEC Form N-MFP / OFR MMF Monitor (monthly institutional MMF breakdown)".to_string(),
                "ICI weekly MMF & mutual fund aggregate reports".to_string(),
                "Foreign sovereign/equity flow feeds (e.g. TIC Treasury monthly, DJPPR SBN / IDX foreign flows)".to_string(),
            ],
            governance_rule: "Pre-register any new hypothesis before ingesting new candidate series to avoid forking paths.".to_string(),
        });
        (
            "LEAF_BLIND_SPOT_UNOBSERVED_DESTINATION".to_string(),
            EpistemicTier::BlindSpot,
            "BLIND SPOT: Tracked risk ETFs showed primary redemptions, but observed cash parking tanks and credit gauges did not absorb them; destination lies outside current sensor coverage.".to_string(),
        )
    }
}

pub(crate) fn evaluate_risk_on_branch(
    m: &DiagnosticTreeMetrics,
    flow_thr: f64,
    risk_etf_flow: f64,
    has_risk_flow: bool,
    nodes: &mut Vec<DiagnosticNode>,
    active_path: &mut Vec<String>,
    _blind_spots: &mut Vec<UnansweredQuestion>,
) -> (String, EpistemicTier, String) {
    // Q4: Quality of Risk-On / Neutral Window (Primary Unit Creation vs Pure Revaluation)
    let primary_inflow = has_risk_flow && risk_etf_flow > flow_thr;
    let liq_expanding = m.delta_stablecoin_usd.is_some_and(|v| v > flow_thr)
        || m.delta_m2_usd.is_some_and(|v| v > 15e9)
        || m.delta_bank_credit_usd.is_some_and(|v| v > 10e9);
    active_path.push("Q4_RALLY_QUALITY_REVAL_VS_INFLOW".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q4_RALLY_QUALITY_REVAL_VS_INFLOW".to_string(),
        parent_id: Some("Q0_MARKET_REGIME".to_string()),
        question: "Is market strength/stability backed by primary ETF share creation or monetary/stablecoin expansion, or mostly price revaluation?".to_string(),
        epistemic_tier: EpistemicTier::Tier1Accounting,
        status: if !has_risk_flow {
            NodeEvalStatus::InsufficientData
        } else if primary_inflow || liq_expanding {
            NodeEvalStatus::True
        } else {
            NodeEvalStatus::False
        },
        branch_taken: if primary_inflow || liq_expanding {
            "primary_inflow_or_expansion".to_string()
        } else {
            "revaluation_dominant".to_string()
        },
        predicate_formula: format!(
            "risk_etf_flow > +{:.2}B OR dStablecoin > +{:.2}B OR dM2 > +15B OR dBankCredit > +10B",
            flow_thr / 1e9,
            flow_thr / 1e9
        ),
        metrics: serde_json::json!({
            "combined_primary_risk_flow_usd": risk_etf_flow,
            "spy_exact_flow_usd": m.spy_exact_flow_usd,
            "jnk_exact_flow_usd": m.jnk_exact_flow_usd,
            "btc_etf_flow_usd": m.btc_etf_flow_usd,
            "delta_stablecoin_usd": m.delta_stablecoin_usd,
            "delta_m2_usd": m.delta_m2_usd,
            "delta_bank_credit_usd": m.delta_bank_credit_usd,
        }),
        summary: format!(
            "Primary risk ETF flow={:+.2}B, dStablecoin={}, dM2={}, dBankCredit={}.",
            risk_etf_flow / 1e9,
            fmt_opt_b(m.delta_stablecoin_usd),
            fmt_opt_b(m.delta_m2_usd),
            fmt_opt_b(m.delta_bank_credit_usd),
        ),
        input_series: vec![
            "etf:SPY:shares_outstanding".to_string(),
            "etf:JNK:shares_outstanding".to_string(),
            "etf:BTC_US:bgeometrics_flow_btc".to_string(),
            "stablecoin:ALL:mcap_usd".to_string(),
            "macro:US:FRED_WM2NS_USD".to_string(),
        ],
        limitations: vec![
            "Descriptive accounting check only; does NOT predict future 4-week rally continuation (Q9 predictive rule rejected out-of-sample).".to_string(),
        ],
    });

    // Q5: Cash Parking Retention vs Rotation Out of Cash
    let cash_draining = m.bil_exact_flow_usd.is_some_and(|v| v < -flow_thr)
        || m.delta_wrmfns_pct.is_some_and(|p| p < -0.15)
        || m.delta_on_rrp_usd.is_some_and(|v| v < -25e9);
    active_path.push("Q5_CASH_PARKING_RETENTION".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q5_CASH_PARKING_RETENTION".to_string(),
        parent_id: Some("Q4_RALLY_QUALITY_REVAL_VS_INFLOW".to_string()),
        question: "Is cash leaving defensive parking tanks (BIL, Retail MMF, ON RRP) or remaining parked?".to_string(),
        epistemic_tier: EpistemicTier::Tier1Accounting,
        status: if cash_draining { NodeEvalStatus::True } else { NodeEvalStatus::False },
        branch_taken: if cash_draining { "cash_leaving_parking".to_string() } else { "cash_remaining_parked".to_string() },
        predicate_formula: format!("BIL_exact < -{:.2}B OR dWRMFNS_pct < -0.15% OR dON_RRP < -25B", flow_thr / 1e9),
        metrics: serde_json::json!({
            "bil_exact_flow_usd": m.bil_exact_flow_usd,
            "delta_wrmfns_usd": m.delta_wrmfns_usd,
            "delta_wrmfns_pct": m.delta_wrmfns_pct,
            "delta_on_rrp_usd": m.delta_on_rrp_usd,
            "delta_bank_deposits_usd": m.delta_bank_deposits_usd,
        }),
        summary: format!(
            "BIL={}, WRMFNS={} ({}), dON_RRP={}, dBankDeposits={}.",
            fmt_opt_b(m.bil_exact_flow_usd),
            fmt_opt_b(m.delta_wrmfns_usd),
            fmt_opt_pct(m.delta_wrmfns_pct),
            fmt_opt_b(m.delta_on_rrp_usd),
            fmt_opt_b(m.delta_bank_deposits_usd),
        ),
        input_series: vec![
            "etf:BIL:shares_outstanding".to_string(),
            "macro:US:FRED_WRMFNS_USD".to_string(),
            "macro:US:FRED_RRPONTSYD_USD".to_string(),
            "macro:US:FRED_BANK_DEPOSITS_USD".to_string(),
        ],
        limitations: vec![
            "ON RRP drawdowns often reflect Treasury bill supply absorption by MMFs rather than direct retail equity buying.".to_string(),
        ],
    });

    if !(primary_inflow || liq_expanding) {
        (
            "LEAF_REVALUATION_DRIVEN_RALLY_LOW_PRIMARY_INFLOW".to_string(),
            EpistemicTier::Tier1Accounting,
            "Risk-on/neutral prices are driven predominantly by price revaluation without material primary ETF creation or monetary expansion.".to_string(),
        )
    } else if cash_draining {
        (
            "LEAF_ROTATION_OUT_OF_CASH_TO_RISK".to_string(),
            EpistemicTier::Tier1Accounting,
            "Primary inflows/expansion coincide with outflows from defensive cash tanks (BIL / Retail MMF / ON RRP); consistent with cash rotating toward risk assets.".to_string(),
        )
    } else {
        (
            "LEAF_PARALLEL_EXPANSION_CASH_STILL_PARKED".to_string(),
            EpistemicTier::Tier1Accounting,
            "Primary risk inflows/expansion occur while cash remains parked or growing in BIL/MMF/Bank Deposits (elastic monetary/credit expansion, not zero-sum cash drawdown).".to_string(),
        )
    }
}
