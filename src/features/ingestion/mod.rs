pub mod binance_rest;
pub mod binance_ws;
pub mod catch_up;
pub mod dukascopy;
pub mod economic_calendar;
pub mod orchestrator;
pub mod prediction_market;
pub mod sanity_gate;
pub mod search;
pub mod tradingview;
pub mod watermark;
pub mod yahoo_finance;

pub use binance_rest::{extract_rate_limit_weight, BinanceRestFetcher};
pub use binance_ws::{
    parse_binance_message, BinanceCombinedStreamEvent, BinanceKlineEvent, BinanceKlinePayload,
    BinanceWsClient, BinanceWsConfig,
};
pub use catch_up::{CatchUpEngine, CatchUpSummary};
pub use dukascopy::{DukascopyObjectBar, DukascopyParser};
pub use economic_calendar::{EconomicCalendarFetcher, RawCalendarItem};
pub use orchestrator::{IngestionOrchestrator, OrchestratorStatus};
pub use prediction_market::{
    persist_prediction_candles_to_lake, KalshiFetcher, PolymarketFetcher,
};
pub use sanity_gate::{SanityError, SanityGate};
pub use search::{SearchResultItem, UnifiedSearchEngine};
pub use watermark::{WatermarkScanner, WatermarkStatus};
pub use tradingview::{TradingViewParser, TradingViewUdfResponse};
pub use yahoo_finance::YahooFinanceFetcher;
