//! Composition Root Runtime Layer: wires pure analytics, storage ledgers, and background
//! continuous supervision cleanly above Vertical Slices (`src/features/`).

pub mod atlas_loader;
mod atlas_series_builders;
pub mod continuous_supervisor;
pub mod prospective_loader;
pub mod smh_loader;
pub mod supply_chain_loader;
pub mod tree_loader;
mod tree_loader_helpers;

pub use atlas_loader::build_liquidity_atlas;

pub use continuous_supervisor::ContinuousAnalyticsSupervisor;
pub use prospective_loader::build_prospective_inputs;
pub use smh_loader::build_smh_predictive;
pub use supply_chain_loader::build_supply_chain_analysis;
pub use tree_loader::{build_tree_inputs, compute_lake_watermark_hash};
