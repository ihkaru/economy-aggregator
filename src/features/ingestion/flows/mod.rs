pub mod cftc_cot;
pub mod finra_margin;
pub mod occ_oi;
pub mod ssga_xlsx;
pub mod stockanalysis;

pub use cftc_cot::CftcCotFetcher;
pub use finra_margin::FinraMarginFetcher;
pub use occ_oi::OccOiFetcher;
pub use ssga_xlsx::SsgaXlsxFetcher;
pub use stockanalysis::StockAnalysisFetcher;
