//! Quantitative Analytics, Multi-Exchange Calendar Alignment, FX Normalization,
//! Statistical Hampel Outlier Cleaning, Corporate Actions, PIT Vintages, Futures Rolls,
//! and Portfolio Risk/Return Calculations.

pub mod bitemporal;
pub mod calendar_aligner;
pub mod config;
pub mod corporate_actions;
pub mod futures_roll;
pub mod fx_normalizer;
pub mod outlier;
pub mod portfolio;
pub mod runner;
pub mod synthetic_bond;

pub use bitemporal::{MacroVintageRecord, PointInTimeEngine, VintageStage};
pub use calendar_aligner::{AlignedPanel, TradingCalendarAligner};
pub use config::{AssetSpec, PortfolioSpec};
pub use corporate_actions::{AdjustedPricePoint, CorporateActionAdjuster, DividendEvent, SplitEvent};
pub use futures_roll::{ForexSessionNormalizer, FuturesRollEngine, FuturesRollEvent, RollMethod};
pub use fx_normalizer::FxNormalizer;
pub use outlier::HampelFilter;
pub use portfolio::{PortfolioEngine, PortfolioMetrics, SingleAssetMetrics};
pub use runner::handle_analyze;
pub use synthetic_bond::{DayCountConvention, SyntheticBondAccumulator};
