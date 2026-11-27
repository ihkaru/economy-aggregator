pub mod dbnomics_and_fed;
pub mod fred;
pub mod nyfed;
pub mod ofr;
pub mod treasury;

pub use dbnomics_and_fed::DbnomicsFedFetcher;
pub use fred::{FredFetcher, FredSeriesSpec, CANONICAL_FRED_SPECS};
pub use nyfed::NyFedFetcher;
pub use ofr::OfrFetcher;
pub use treasury::TreasuryFetcher;
