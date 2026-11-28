//! Composition Root Runtime Layer: wires pure analytics, storage ledgers, and background
//! continuous supervision cleanly above Vertical Slices (`src/features/`).

pub mod continuous_supervisor;
pub mod prospective_loader;
pub mod tree_loader;

pub use continuous_supervisor::ContinuousAnalyticsSupervisor;
pub use prospective_loader::build_prospective_inputs;
pub use tree_loader::{build_tree_inputs, compute_lake_watermark_hash};
