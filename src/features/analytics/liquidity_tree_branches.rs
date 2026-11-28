//! Neutral Accounting Branch Evaluators for `liquidity-tree-v1.1.0` (Phase 359).
//! Eliminates unobserved motive language ("dip-buying", "profit-taking", "defensive"),
//! removes pseudo-OOS `dash_for_cash_gld_out_and_bil_in` flag, enforces price-neutral BTC OI
//! (contracts + 8h funding rate), and attaches SOFR/DTB3 yield & HY-OAS/COT context.

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

fn fmt_opt_f(v: Option<f64>, prec: usize) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.p$}", x, p = prec),
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
    // Q1: Accounting Revaluation vs Primary ETF Unit Creations/Redemptions (Tier 1)
    let primary_outflow = has_risk_flow && risk_etf_flow < -flow_thr;
    let spy_inflow = m.spy_exact_flow_usd.is_some_and(|v| v > flow_thr);
    let btc_etf_outflow = m.btc_etf_flow_usd.is_some_and(|v| v < -flow_thr);
    let gld_outflow = m.gld_exact_flow_usd.is_some_and(|v| v < -0.6 * flow_thr);
    active_path.push("Q1_ETF_REVAL_VS_PRIMARY".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q1_ETF_REVAL_VS_PRIMARY".to_string(),
        parent_id: Some("Q0_MARKET_REGIME".to_string()),
        question: "Did tracked risk ETF AUM decline via net primary unit redemptions, or price revaluation?".to_string(),
        epistemic_tier: EpistemicTier::Tier1Accounting,
        status: if !has_risk_flow { NodeEvalStatus::InsufficientData } else if primary_outflow { NodeEvalStatus::True } else { NodeEvalStatus::False },
        branch_taken: if primary_outflow { "net_primary_redemption" } else if btc_etf_outflow && spy_inflow { "divergent_btc_outflow_spy_inflow" } else { "revaluation_dominant" }.to_string(),
        predicate_formula: format!("(SPY_exact + JNK_exact + BTC_ETF_usd) < -{:.2}B", flow_thr / 1e9),
        metrics: serde_json::json!({
            "spy_delta_aum_usd": m.spy_delta_aum_usd,
            "spy_exact_flow_usd": m.spy_exact_flow_usd,
            "jnk_exact_flow_usd": m.jnk_exact_flow_usd,
            "btc_etf_flow_usd": m.btc_etf_flow_usd,
            "combined_primary_risk_flow_usd": risk_etf_flow,
        }),
        summary: format!(
            "Net primary ETF risk flow={:+.2}B USD (SPY ΔAUM={} vs primary units={}; JNK={}; BTC Spot ETF={}).",
            risk_etf_flow / 1e9,
            fmt_opt_b(m.spy_delta_aum_usd),
            fmt_opt_b(m.spy_exact_flow_usd),
            fmt_opt_b(m.jnk_exact_flow_usd),
            fmt_opt_b(m.btc_etf_flow_usd),
        ),
        input_series: ["etf:SPY:aum_usd", "etf:SPY:shares_outstanding", "etf:JNK:shares_outstanding", "etf:BTC_US:bgeometrics_flow_btc"].into_iter().map(str::to_string).collect(),
        limitations: vec![
            "Accounting identity for primary AP creations/redemptions only; does NOT identify buyer identity (institutional vs retail vs options/dealer hedging or rebalancing) or motive.".to_string(),
        ],
    });

    // Q2: Short-Term Cash-Like Instruments & Gold ETF Flows (Neutral Accounting + Yield Context)
    let bil_in = m.bil_exact_flow_usd.is_some_and(|v| v > flow_thr);
    let mmf_in = m.delta_wrmfns_pct.is_some_and(|p| p >= 0.15);
    let cash_tanks_up = bil_in || mmf_in;
    active_path.push("Q2_DEFENSIVE_CASH_PARKING".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q2_DEFENSIVE_CASH_PARKING".to_string(),
        parent_id: Some("Q1_ETF_REVAL_VS_PRIMARY".to_string()),
        question: "Did short-term cash instruments (BIL T-Bill, WRMFNS Retail MMF, H.8 Bank Deposits) record net inflows, and what is the yield context?".to_string(),
        epistemic_tier: if mmf_in { EpistemicTier::Tier2Replicated } else { EpistemicTier::Tier1Accounting },
        status: if m.bil_exact_flow_usd.is_none() && m.delta_wrmfns_pct.is_none() { NodeEvalStatus::InsufficientData } else if cash_tanks_up { NodeEvalStatus::True } else { NodeEvalStatus::False },
        branch_taken: if cash_tanks_up { "cash_instruments_inflow" } else { "cash_instruments_flat_or_out" }.to_string(),
        predicate_formula: format!("BIL_exact > +{:.2}B OR dWRMFNS_pct >= +0.15%", flow_thr / 1e9),
        metrics: serde_json::json!({
            "bil_exact_flow_usd": m.bil_exact_flow_usd,
            "dtb3_latest_pct": m.dtb3_latest_pct,
            "sofr_latest_pct": m.sofr_latest_pct,
            "delta_wrmfns_usd": m.delta_wrmfns_usd,
            "delta_wrmfns_pct": m.delta_wrmfns_pct,
            "delta_bank_deposits_usd": m.delta_bank_deposits_usd,
            "delta_bank_deposits_pct": m.delta_bank_deposits_pct,
            "gld_exact_flow_usd": m.gld_exact_flow_usd,
            "cot_gold_managed_money_delta": m.cot_gold_managed_money_delta,
            "cot_gold_is_stale": m.cot_gold_is_stale,
        }),
        summary: format!(
            "BIL={} (DTB3={}%, SOFR={}%), Retail MMF={} ({}), H.8 Bank Deposits={} ({}), GLD={} (COT Gold MM Δ={} contracts, stale={}).",
            fmt_opt_b(m.bil_exact_flow_usd),
            fmt_opt_f(m.dtb3_latest_pct, 2),
            fmt_opt_f(m.sofr_latest_pct, 2),
            fmt_opt_b(m.delta_wrmfns_usd),
            fmt_opt_pct(m.delta_wrmfns_pct),
            fmt_opt_b(m.delta_bank_deposits_usd),
            fmt_opt_pct(m.delta_bank_deposits_pct),
            fmt_opt_b(m.gld_exact_flow_usd),
            fmt_opt_f(m.cot_gold_managed_money_delta, 0),
            m.cot_gold_is_stale,
        ),
        input_series: ["etf:BIL:shares_outstanding", "macro:US:FRED_DTB3_PCT", "macro:US:FRED_SOFR_PCT", "macro:US:FRED_WRMFNS_USD", "macro:US:FRED_BANK_DEPOSITS_USD", "etf:GLD:shares_outstanding", "cot:GOLD:managed_money_net"].into_iter().map(str::to_string).collect(),
        limitations: vec![
            "With elevated short-term rates (DTB3/SOFR), BIL inflows cannot distinguish precautionary cash holding from yield/carry allocation.".to_string(),
            "GLD tracks US financial gold ETF units only (excludes central bank and Asian physical demand); outflows do not prove seller motive (e.g. profit-taking).".to_string(),
            "Institutional weekly MMF (WIMFNS) was discontinued in Feb 2021; H.8 Bank Deposits are upstream SA with ~9-10 day lag.".to_string(),
        ],
    });

    // Q3: Price-Neutral Leverage (Contracts + Funding), Margin & Credit Conditions
    let crypto_contracts_down = m.delta_btc_oi_contracts_pct.is_some_and(|p| p <= -5.0);
    let finra_down = !m.finra_is_stale && m.finra_margin_delta_pct.is_some_and(|p| p <= -1.0);
    let hy_widening = m.delta_hy_oas_bps.is_some_and(|b| b >= 50.0);
    let macro_drain = m.delta_bank_credit_usd.is_some_and(|v| v < -10e9)
        || m.delta_walcl_minus_tga_usd.is_some_and(|v| v < -25e9)
        || hy_widening;
    let any_contraction = crypto_contracts_down || finra_down || macro_drain;
    active_path.push("Q3_CREDIT_AND_MACRO_CONTRACTION".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q3_CREDIT_AND_MACRO_CONTRACTION".to_string(),
        parent_id: Some("Q2_DEFENSIVE_CASH_PARKING".to_string()),
        question: "Did price-neutral derivative contracts, equity margin debt, HY credit spreads, or released bank/Fed liquidity tighten?".to_string(),
        epistemic_tier: if finra_down { EpistemicTier::Tier2Replicated } else { EpistemicTier::Tier1Accounting },
        status: if m.finra_is_stale && !any_contraction { NodeEvalStatus::StaleLag } else if any_contraction { NodeEvalStatus::True } else { NodeEvalStatus::False },
        branch_taken: if finra_down || macro_drain { "macro_credit_or_margin_tightening" } else if crypto_contracts_down { "crypto_contract_contraction_only" } else { "no_contract_or_released_credit_contraction" }.to_string(),
        predicate_formula: "dBybit_BTC_OI_contracts_pct <= -5% OR (fresh FINRA <= -1%) OR dHY_OAS >= +50bps OR dBankCredit < -10B OR d(WALCL-TGA) < -25B".to_string(),
        metrics: serde_json::json!({
            "delta_btc_oi_contracts": m.delta_btc_oi_contracts,
            "delta_btc_oi_contracts_pct": m.delta_btc_oi_contracts_pct,
            "delta_btc_oi_usd_nominal_price_affected": m.delta_btc_oi_usd,
            "btc_funding_rate_8h_avg_bps": m.btc_funding_rate_8h_avg_bps,
            "btc_funding_rate_latest_bps": m.btc_funding_rate_latest_bps,
            "delta_stablecoin_usd": m.delta_stablecoin_usd,
            "hy_oas_latest_pct": m.hy_oas_latest_pct,
            "delta_hy_oas_bps": m.delta_hy_oas_bps,
            "finra_margin_delta_pct": m.finra_margin_delta_pct,
            "finra_is_stale": m.finra_is_stale,
            "h8_covers_latest_market_date": m.h8_covers_latest_market_date,
            "delta_bank_credit_usd": m.delta_bank_credit_usd,
            "delta_walcl_minus_tga_usd": m.delta_walcl_minus_tga_usd,
        }),
        summary: format!(
            "Bybit BTC OI={} contracts ({}, nominal USD OI={}), BTC funding_8h_avg={}bps, HY_OAS={}% (d={}bps), FINRA={} (stale={}), released H.8 BankCredit={} (covers_latest={}), d(WALCL-TGA)={}.",
            fmt_opt_f(m.delta_btc_oi_contracts, 0),
            fmt_opt_pct(m.delta_btc_oi_contracts_pct),
            fmt_opt_b(m.delta_btc_oi_usd),
            fmt_opt_f(m.btc_funding_rate_8h_avg_bps, 2),
            fmt_opt_f(m.hy_oas_latest_pct, 2),
            fmt_opt_f(m.delta_hy_oas_bps, 0),
            fmt_opt_pct(m.finra_margin_delta_pct),
            m.finra_is_stale,
            fmt_opt_b(m.delta_bank_credit_usd),
            m.h8_covers_latest_market_date,
            fmt_opt_b(m.delta_walcl_minus_tga_usd),
        ),
        input_series: ["deriv:BYBIT_BTC:open_interest_contracts", "deriv:BINANCE_BTC:funding_rate_8h", "deriv:BTC:bgeometrics_oi_usd", "macro:US:FRED_HY_OAS_PCT", "credit:US:FINRA_MARGIN_DEBT_USD", "macro:US:FRED_TOTBKCR_USD", "macro:US:FRED_WALCL_USD", "macro:US:TGA_BALANCE_USD"].into_iter().map(str::to_string).collect(),
        limitations: vec![
            "Nominal USD OI drops mechanically with BTC price; contract-denominated OI (Bybit) and 8h funding rates are used to avoid price-revaluation bias.".to_string(),
            "Fed H.8 and FINRA lag market prices; absence of contraction in released data does NOT prove absence of stress during unprinted weeks.".to_string(),
        ],
    });

    if finra_down || macro_drain {
        let tier = if finra_down { EpistemicTier::Tier2Replicated } else { EpistemicTier::Tier1Accounting };
        (
            "LEAF_MACRO_CREDIT_OR_MARGIN_TIGHTENING".to_string(),
            tier,
            "Window drawdown coincides with observable credit/margin tightening in released series (fresh FINRA / HY OAS widening / bank credit / WALCL-TGA).".to_string(),
        )
    } else if crypto_contracts_down {
        (
            "LEAF_CRYPTO_CONTRACT_DELEVERAGING".to_string(),
            EpistemicTier::Tier1Accounting,
            "Contract-denominated BTC perpetual OI contracted during the window without broad released macro credit contraction.".to_string(),
        )
    } else if primary_outflow && cash_tanks_up {
        (
            "LEAF_PRIMARY_OUTFLOW_WITH_CASH_INFLOWS".to_string(),
            if mmf_in { EpistemicTier::Tier2Replicated } else { EpistemicTier::Tier1Accounting },
            "Net primary risk ETF redemptions occurred alongside inflows to short-term cash instruments (BIL / Retail MMF); yield/carry vs precautionary motive is not separated by flows alone.".to_string(),
        )
    } else if btc_etf_outflow && spy_inflow {
        (
            "LEAF_CROSS_COMPARTMENT_DIVERGENCE_CRYPTO_OUT_EQUITY_IN".to_string(),
            EpistemicTier::Tier1Accounting,
            format!(
                "Single-window snapshot: primary outflows in US Spot BTC ETF ({}{}) occurred alongside net primary unit creation in SPY ({}) and inflows in BIL ({}); no contract-denominated BTC OI contraction or released H.8 credit drop observed so far.",
                fmt_opt_b(m.btc_etf_flow_usd),
                if gld_outflow { format!(" & GLD {}", fmt_opt_b(m.gld_exact_flow_usd)) } else { String::new() },
                fmt_opt_b(m.spy_exact_flow_usd),
                fmt_opt_b(m.bil_exact_flow_usd),
            ),
        )
    } else if !primary_outflow {
        (
            "LEAF_REVALUATION_DOMINANT_NO_PRIMARY_EXODUS".to_string(),
            EpistemicTier::Tier1Accounting,
            "Tracked ETF AUM change over this window is predominantly mark-to-market price revaluation without net aggregate primary unit redemption or released credit contraction.".to_string(),
        )
    } else {
        blind_spots.push(UnansweredQuestion {
            triggered_by_node: "LEAF_BLIND_SPOT_UNOBSERVED_DESTINATION".to_string(),
            question: "Net primary risk ETF redemptions occurred while BIL, Retail MMF, and released credit sensors showed no offsetting move — where did those funds settle?".to_string(),
            missing_data_gap: "Institutional weekly MMF (WIMFNS discontinued), non-SSGA ETF/mutual funds, exchange-internal stablecoin reserves, and non-US capital flows are unobserved.".to_string(),
            candidate_open_sources: vec![
                "SEC Form N-MFP / OFR MMF Monitor (monthly institutional MMF breakdown)".to_string(),
                "ICI weekly MMF & mutual fund aggregate reports".to_string(),
                "TIC Treasury monthly & regional foreign equity/bond flow feeds".to_string(),
            ],
            governance_rule: "Pre-register any hypothesis before ingesting new series to prevent forking paths.".to_string(),
        });
        (
            "LEAF_BLIND_SPOT_UNOBSERVED_DESTINATION".to_string(),
            EpistemicTier::BlindSpot,
            "BLIND SPOT: Tracked risk ETFs showed net primary redemptions, but observed cash instruments and released credit sensors did not offset them.".to_string(),
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
    // Q4: Primary ETF Unit Creation vs Pure Revaluation (Tier 1 Accounting)
    let primary_inflow = has_risk_flow && risk_etf_flow > flow_thr;
    let liq_expanding = m.delta_stablecoin_usd.is_some_and(|v| v > flow_thr)
        || m.delta_m2_usd.is_some_and(|v| v > 15e9)
        || m.delta_bank_credit_usd.is_some_and(|v| v > 10e9);
    active_path.push("Q4_RALLY_QUALITY_REVAL_VS_INFLOW".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q4_RALLY_QUALITY_REVAL_VS_INFLOW".to_string(),
        parent_id: Some("Q0_MARKET_REGIME".to_string()),
        question: "Did the non-drawdown window coincide with net primary ETF unit creation or monetary/stablecoin expansion, or mostly price revaluation?".to_string(),
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
            "Net primary ETF risk flow={:+.2}B (SPY={}, JNK={}, BTC ETF={}), dStablecoin={}, dM2={}, dBankCredit={}.",
            risk_etf_flow / 1e9,
            fmt_opt_b(m.spy_exact_flow_usd),
            fmt_opt_b(m.jnk_exact_flow_usd),
            fmt_opt_b(m.btc_etf_flow_usd),
            fmt_opt_b(m.delta_stablecoin_usd),
            fmt_opt_b(m.delta_m2_usd),
            fmt_opt_b(m.delta_bank_credit_usd),
        ),
        input_series: ["etf:SPY:shares_outstanding", "etf:JNK:shares_outstanding", "etf:BTC_US:bgeometrics_flow_btc", "stablecoin:ALL:mcap_usd", "macro:US:FRED_WM2NS_USD"].into_iter().map(str::to_string).collect(),
        limitations: vec![
            "Descriptive accounting check only; does NOT identify buyer motive or predict future 4-week continuation (Q9 rejected out-of-sample).".to_string(),
            "Stablecoin market cap decline is consistent with net redemptions, though single-aggregator coverage cannot rule out off-coverage issuer shifts.".to_string(),
        ],
    });

    // Q5: Short-Term Cash Instrument Flows Alongside Short-Term Yields
    let cash_draining = m.bil_exact_flow_usd.is_some_and(|v| v < -flow_thr)
        || m.delta_wrmfns_pct.is_some_and(|p| p < -0.15)
        || m.delta_on_rrp_usd.is_some_and(|v| v < -25e9);
    active_path.push("Q5_CASH_PARKING_RETENTION".to_string());
    nodes.push(DiagnosticNode {
        node_id: "Q5_CASH_PARKING_RETENTION".to_string(),
        parent_id: Some("Q4_RALLY_QUALITY_REVAL_VS_INFLOW".to_string()),
        question: "Did short-term cash instruments (BIL, Retail MMF, ON RRP) record net outflows or remain flat/growing alongside current short-term yields?".to_string(),
        epistemic_tier: EpistemicTier::Tier1Accounting,
        status: if cash_draining { NodeEvalStatus::True } else { NodeEvalStatus::False },
        branch_taken: if cash_draining { "cash_instruments_outflow".to_string() } else { "cash_instruments_retained_or_inflow".to_string() },
        predicate_formula: format!("BIL_exact < -{:.2}B OR dWRMFNS_pct < -0.15% OR dON_RRP < -25B", flow_thr / 1e9),
        metrics: serde_json::json!({
            "bil_exact_flow_usd": m.bil_exact_flow_usd,
            "dtb3_latest_pct": m.dtb3_latest_pct,
            "sofr_latest_pct": m.sofr_latest_pct,
            "delta_wrmfns_usd": m.delta_wrmfns_usd,
            "delta_wrmfns_pct": m.delta_wrmfns_pct,
            "delta_on_rrp_usd": m.delta_on_rrp_usd,
            "delta_bank_deposits_usd": m.delta_bank_deposits_usd,
        }),
        summary: format!(
            "BIL={} (DTB3={}%, SOFR={}%), WRMFNS={} ({}), dON_RRP={}, dBankDeposits={}.",
            fmt_opt_b(m.bil_exact_flow_usd),
            fmt_opt_f(m.dtb3_latest_pct, 2),
            fmt_opt_f(m.sofr_latest_pct, 2),
            fmt_opt_b(m.delta_wrmfns_usd),
            fmt_opt_pct(m.delta_wrmfns_pct),
            fmt_opt_b(m.delta_on_rrp_usd),
            fmt_opt_b(m.delta_bank_deposits_usd),
        ),
        input_series: ["etf:BIL:shares_outstanding", "macro:US:FRED_DTB3_PCT", "macro:US:FRED_SOFR_PCT", "macro:US:FRED_WRMFNS_USD", "macro:US:FRED_RRPONTSYD_USD", "macro:US:FRED_BANK_DEPOSITS_USD"].into_iter().map(str::to_string).collect(),
        limitations: vec![
            "BIL inflows/retention under elevated short-term yields (DTB3/SOFR) may reflect yield/carry demand rather than precautionary cash hoarding.".to_string(),
            "ON RRP drawdowns often reflect Treasury bill supply absorption by MMFs rather than direct equity buying.".to_string(),
        ],
    });

    if !(primary_inflow || liq_expanding) {
        (
            "LEAF_REVALUATION_DRIVEN_WINDOW_LOW_PRIMARY_INFLOW".to_string(),
            EpistemicTier::Tier1Accounting,
            "Non-drawdown window occurred predominantly via mark-to-market price change without material net primary ETF creation or monetary expansion.".to_string(),
        )
    } else if cash_draining {
        (
            "LEAF_PRIMARY_INFLOWS_WITH_CASH_OUTFLOWS".to_string(),
            EpistemicTier::Tier1Accounting,
            "Net primary risk ETF inflows/expansion coincided with outflows from short-term cash instruments (BIL / Retail MMF / ON RRP).".to_string(),
        )
    } else {
        (
            "LEAF_PARALLEL_PRIMARY_INFLOWS_CASH_HELD".to_string(),
            EpistemicTier::Tier1Accounting,
            "Net primary risk ETF inflows occurred while short-term cash instruments (BIL / MMF / ON RRP) remained flat or growing (descriptive accounting co-movement; motive not separated from short-term yield carry).".to_string(),
        )
    }
}
