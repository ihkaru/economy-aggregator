//! Quantitative Analytics, Multi-Exchange Calendar Alignment, FX Normalization,
//! Statistical Hampel Outlier Cleaning, and Portfolio Risk/Return Calculations.

pub mod calendar_aligner;
pub mod config;
pub mod fx_normalizer;
pub mod outlier;
pub mod portfolio;
pub mod runner;
pub mod synthetic_bond;

pub use calendar_aligner::{AlignedPanel, TradingCalendarAligner};
pub use config::{AssetSpec, PortfolioSpec};
pub use fx_normalizer::FxNormalizer;
pub use outlier::HampelFilter;
pub use portfolio::{PortfolioEngine, PortfolioMetrics, SingleAssetMetrics};
pub use runner::handle_analyze;
pub use synthetic_bond::{DayCountConvention, SyntheticBondAccumulator};
