//! Quantitative Analytics, Multi-Exchange Calendar Alignment, FX Normalization,
//! Statistical Hampel Outlier Cleaning, Corporate Actions, PIT Vintages, Futures Rolls,
//! Macroeconomic Consensus Divergence, and Portfolio Risk/Return Calculations.

pub mod bitemporal;
pub mod calendar_aligner;
pub mod config;
pub mod corporate_actions;
pub mod futures_roll;
pub mod fx_normalizer;
pub mod liquidity_flows;
pub mod liquidity_panel;
pub mod liquidity_tree;
mod liquidity_tree_branches;
pub mod macro_prediction_bridge;
pub mod outlier;
pub mod portfolio;
pub mod prospective_scorecard;
pub mod reconciliation;
pub mod reconcile_cli;
pub mod regime_transitions;
pub mod runner;
pub mod spillover;
pub mod spillover_breaks;
pub mod spillover_diagnostics;
pub mod synthetic_bond;
pub mod smh_predictive;
mod smh_predictive_math;
pub mod supply_chain_analysis;

pub use bitemporal::{MacroVintageRecord, PointInTimeEngine, VintageStage};
pub use calendar_aligner::{AlignedPanel, TradingCalendarAligner};
pub use config::{AssetSpec, PortfolioSpec};
pub use corporate_actions::{AdjustedBar, AdjustedPricePoint, CorporateActionAdjuster, DividendEvent, RawBar, SplitEvent};
pub use futures_roll::{ForexSessionNormalizer, FuturesRollEngine, FuturesRollEvent, RollMethod};
pub use fx_normalizer::FxNormalizer;
pub use liquidity_flows::{
    compute_btc_etf_usd_flows, compute_exact_share_flows, compute_fed_net_liquidity,
    compute_implied_net_flows, net_flows_to_observations, FedNetLiquidityPoint,
    ImpliedNetFlowPoint,
};
pub use liquidity_panel::{
    align_to_friday_date, build_weekly_flow_panel, WeeklyFlowPanel, WeeklyFlowRow,
    WeeklyPanelInputs,
};
pub use liquidity_tree::{
    evaluate_liquidity_tree, DiagnosticTreeInputs, DiagnosticTreeMetrics, TREE_PROTOCOL_VERSION,
};
pub use spillover::{
    compute_diebold_yilmaz_spillover, compute_diebold_yilmaz_spillover_full,
    compute_diebold_yilmaz_spillover_with_options, DirectionalSpillover, SpilloverReport,
};
pub use spillover_breaks::{EquationBreakStat, StructuralBreakDiagnostics};
pub use spillover_diagnostics::NullModel;
pub use macro_prediction_bridge::{MacroPredictionBridge, MacroPredictionDivergence};
pub use outlier::HampelFilter;
pub use portfolio::{PortfolioEngine, PortfolioMetrics, SingleAssetMetrics};
pub use prospective_scorecard::{
    evaluate_prospective_scorecard, ProspectiveScorecardInputs, ProspectiveWeeklyObs,
    OOS_START_WEEK, PROSPECTIVE_PROTOCOL_SHA256, PROSPECTIVE_PROTOCOL_VERSION,
};
pub use regime_transitions::detect_regime_transition;
pub use reconciliation::{CandleDiscrepancy, DiscrepancyType, ReconciliationEngine, ReconciliationReport};
pub use smh_predictive::{
    evaluate_smh_predictive, SmhPredictiveInputs, SmhPredictiveResponse, TradeSignal,
};
pub use supply_chain_analysis::{evaluate_supply_chain_lead_lag, SupplyChainAnalysisResponse};
pub use reconcile_cli::handle_reconcile;
pub use runner::handle_analyze;
pub use synthetic_bond::{DayCountConvention, SyntheticBondAccumulator};
