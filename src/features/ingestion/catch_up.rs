//! Historical batch catch-up engine for filling data gaps when container resumes after downtime.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::core::config::LakehouseConfig;
use crate::core::subscription::SubscriptionItem;
use crate::core::types::AssetClass;
use crate::features::ingestion::binance_rest::BinanceRestFetcher;
use crate::features::ingestion::economic_calendar::EconomicCalendarFetcher;
use crate::features::ingestion::watermark::{WatermarkScanner, WatermarkStatus};
use crate::features::ingestion::yahoo_finance::YahooFinanceFetcher;
use crate::features::storage::partition::PartitionResolver;
use crate::features::storage::writer::LakeWriter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchUpSummary {
    pub source: String,
    pub symbol: String,
    pub asset_class: String,
    pub timeframe: String,
    pub gap_days: i64,
    pub candles_ingested: usize,
    pub status: String,
    pub error: Option<String>,
}

pub struct CatchUpEngine {
    config: LakehouseConfig,
    resolver: PartitionResolver,
}

impl CatchUpEngine {
    pub fn new(config: LakehouseConfig) -> Self {
        let resolver = PartitionResolver::new(config.lake_dir.clone());
        Self { config, resolver }
    }

    /// Evaluates watermarks and pulls missing historical ranges for a subscription.
    pub async fn catch_up_subscription(
        &self,
        sub: &SubscriptionItem,
        force: bool,
    ) -> Vec<CatchUpSummary> {
        let mut summaries = Vec::new();

        if sub.asset_class == "observations" {
            let report = crate::features::ingestion::liquidity_sync::sync_observations(&self.config, &sub.source).await;
            summaries.push(CatchUpSummary {
                source: sub.source.clone(),
                symbol: sub.symbol.clone(),
                asset_class: "observations".to_string(),
                timeframe: "1d".to_string(),
                gap_days: 0,
                candles_ingested: report.total_rows_written,
                status: "caught_up".to_string(),
                error: None,
            });
            return summaries;
        }

        if sub.asset_class == "macro" {
            let summary = self.catch_up_macro(sub).await;
            summaries.push(summary);
            return summaries;
        }

        if sub.asset_class == "prediction_markets"
            || matches!(sub.source.to_lowercase().as_str(), "polymarket" | "kalshi")
        {
            let summary = self.catch_up_prediction_market(sub).await;
            summaries.push(summary);
            return summaries;
        }

        for tf in &sub.timeframes {
            let watermark = WatermarkScanner::scan_watermark(
                &self.config.lake_dir,
                &sub.asset_class,
                &sub.symbol,
                tf,
            );

            if !watermark.has_gap && !force {
                summaries.push(CatchUpSummary {
                    source: sub.source.clone(),
                    symbol: sub.symbol.clone(),
                    asset_class: sub.asset_class.clone(),
                    timeframe: tf.clone(),
                    gap_days: 0,
                    candles_ingested: 0,
                    status: "up_to_date".to_string(),
                    error: None,
                });
                continue;
            }

            let summary = self.execute_catch_up(sub, tf, &watermark).await;
            summaries.push(summary);
        }

        summaries
    }

    async fn execute_catch_up(
        &self,
        sub: &SubscriptionItem,
        timeframe: &str,
        watermark: &WatermarkStatus,
    ) -> CatchUpSummary {
        let asset_class = match sub.asset_class.to_lowercase().as_str() {
            "crypto" => AssetClass::Crypto,
            "forex" => AssetClass::Forex,
            "commodities" => AssetClass::Commodities,
            _ => AssetClass::Equities,
        };

        let result = match sub.source.to_lowercase().as_str() {
            "binance" => {
                let fetcher = BinanceRestFetcher::default();
                let start_time = watermark
                    .latest_timestamp_utc
                    .map(|ts| ts.timestamp_millis() + 1000);
                fetcher
                    .fetch_klines(
                        &sub.symbol,
                        &sub.symbol,
                        timeframe,
                        start_time,
                        None,
                        Some(1000),
                    )
                    .await
            }
            "tradingview" | "tv" => {
                let fetcher = crate::features::ingestion::tradingview::TradingViewFetcher::default();
                let tf_enum = match timeframe {
                    "1m" => crate::core::types::TimeFrame::M1,
                    "5m" => crate::core::types::TimeFrame::M5,
                    "15m" => crate::core::types::TimeFrame::M15,
                    "1h" => crate::core::types::TimeFrame::H1,
                    "4h" => crate::core::types::TimeFrame::H4,
                    "1w" => crate::core::types::TimeFrame::W1,
                    "1M" => crate::core::types::TimeFrame::MN1,
                    _ => crate::core::types::TimeFrame::D1,
                };
                let count = if watermark.gap_duration_days <= 5 {
                    50
                } else if watermark.gap_duration_days <= 30 {
                    100
                } else if watermark.gap_duration_days <= 90 {
                    300
                } else {
                    500
                };
                fetcher.fetch_bars(&sub.symbol, tf_enum, count).await
            }
            _ => {
                let fetcher = YahooFinanceFetcher::default();
                let range = if watermark.gap_duration_days <= 5 {
                    "5d"
                } else if watermark.gap_duration_days <= 30 {
                    "1mo"
                } else if watermark.gap_duration_days <= 90 {
                    "3mo"
                } else if watermark.gap_duration_days <= 180 {
                    "6mo"
                } else if watermark.gap_duration_days <= 365 {
                    "1y"
                } else {
                    "2y"
                };
                fetcher
                    .fetch_daily_bars(&sub.symbol, &sub.symbol, range)
                    .await
            }
        };

        match result {
            Ok(candles) if !candles.is_empty() => {
                let writer = LakeWriter::new(self.resolver.clone());
                match writer.write_candles(&candles, asset_class) {
                    Ok(paths) => {
                        info!(
                            "🚀 Catch-up SUCCESS for {} ({}): {} candles saved to {} chunk files",
                            sub.symbol, timeframe, candles.len(), paths.len()
                        );
                        CatchUpSummary {
                            source: sub.source.clone(),
                            symbol: sub.symbol.clone(),
                            asset_class: sub.asset_class.clone(),
                            timeframe: timeframe.to_string(),
                            gap_days: watermark.gap_duration_days,
                            candles_ingested: candles.len(),
                            status: "caught_up".to_string(),
                            error: None,
                        }
                    }
                    Err(e) => CatchUpSummary {
                        source: sub.source.clone(),
                        symbol: sub.symbol.clone(),
                        asset_class: sub.asset_class.clone(),
                        timeframe: timeframe.to_string(),
                        gap_days: watermark.gap_duration_days,
                        candles_ingested: 0,
                        status: "write_error".to_string(),
                        error: Some(e.to_string()),
                    },
                }
            }
            Ok(_) => CatchUpSummary {
                source: sub.source.clone(),
                symbol: sub.symbol.clone(),
                asset_class: sub.asset_class.clone(),
                timeframe: timeframe.to_string(),
                gap_days: watermark.gap_duration_days,
                candles_ingested: 0,
                status: "no_new_candles".to_string(),
                error: None,
            },
            Err(e) => {
                warn!("⚠️ Catch-up fetch failed for {}: {}", sub.symbol, e);
                CatchUpSummary {
                    source: sub.source.clone(),
                    symbol: sub.symbol.clone(),
                    asset_class: sub.asset_class.clone(),
                    timeframe: timeframe.to_string(),
                    gap_days: watermark.gap_duration_days,
                    candles_ingested: 0,
                    status: "fetch_error".to_string(),
                    error: Some(e.to_string()),
                }
            }
        }
    }

    async fn catch_up_macro(&self, sub: &SubscriptionItem) -> CatchUpSummary {
        let fetcher = EconomicCalendarFetcher::default();
        match fetcher.fetch_rich_events(Some(&sub.symbol), None).await {
            Ok(events) => match EconomicCalendarFetcher::persist_to_lake(&events, &self.resolver) {
                Ok(saved) => CatchUpSummary {
                    source: sub.source.clone(),
                    symbol: sub.symbol.clone(),
                    asset_class: "macro".to_string(),
                    timeframe: "events".to_string(),
                    gap_days: 0,
                    candles_ingested: saved,
                    status: "caught_up".to_string(),
                    error: None,
                },
                Err(e) => CatchUpSummary {
                    source: sub.source.clone(),
                    symbol: sub.symbol.clone(),
                    asset_class: "macro".to_string(),
                    timeframe: "events".to_string(),
                    gap_days: 0,
                    candles_ingested: 0,
                    status: "persist_error".to_string(),
                    error: Some(e.to_string()),
                },
            },
            Err(e) => CatchUpSummary {
                source: sub.source.clone(),
                symbol: sub.symbol.clone(),
                asset_class: "macro".to_string(),
                timeframe: "events".to_string(),
                gap_days: 0,
                candles_ingested: 0,
                status: "fetch_error".to_string(),
                error: Some(e.to_string()),
            },
        }
    }

    async fn catch_up_prediction_market(&self, sub: &SubscriptionItem) -> CatchUpSummary {
        use chrono::Utc;
        use crate::core::prediction_types::{PredictionMarketCandle, ProbabilityPriceSource};
        use crate::features::ingestion::prediction_market::{
            persist_prediction_candles_to_lake, KalshiFetcher, PolymarketFetcher,
        };

        let tf = sub.timeframes.first().map(|s| s.as_str()).unwrap_or("1h");
        let venue = sub.source.to_lowercase();
        let fetched = if venue == "kalshi" {
            KalshiFetcher::default().fetch_markets(25).await
        } else {
            PolymarketFetcher::default().fetch_contracts(25).await
        };

        match fetched {
            Ok(contracts) => {
                let now = Utc::now();
                let snapshots: Vec<PredictionMarketCandle> = contracts
                    .into_iter()
                    .filter(|c| {
                        sub.symbol.eq_ignore_ascii_case("TOP_ACTIVE")
                            || sub.symbol == "*"
                            || c.market_id.eq_ignore_ascii_case(&sub.symbol)
                            || c.venue_market_id.eq_ignore_ascii_case(&sub.symbol)
                    })
                    .map(|c| {
                        let prob = c.implied_probability.or(c.last_price);
                        PredictionMarketCandle {
                            time: now.timestamp(),
                            timestamp_utc: now,
                            market_id: c.venue_market_id,
                            outcome: c.outcomes.into_iter().next().unwrap_or_else(|| "YES".to_string()),
                            timeframe: tf.to_string(),
                            prob_open: prob,
                            prob_high: prob,
                            prob_low: prob,
                            prob_close: prob,
                            book_bid_close: c.best_bid,
                            book_ask_close: c.best_ask,
                            volume_contracts: None,
                            volume_usd: c.volume_usd,
                            open_interest_usd: c.open_interest_usd,
                            trade_count: None,
                            price_source: if venue == "kalshi" {
                                ProbabilityPriceSource::Trade
                            } else {
                                ProbabilityPriceSource::SampledMid
                            },
                            venue: c.venue,
                        }
                    })
                    .collect();

                match persist_prediction_candles_to_lake(&snapshots, &self.resolver) {
                    Ok(saved) => CatchUpSummary {
                        source: sub.source.clone(),
                        symbol: sub.symbol.clone(),
                        asset_class: "prediction_markets".to_string(),
                        timeframe: tf.to_string(),
                        gap_days: 0,
                        candles_ingested: saved,
                        status: "caught_up".to_string(),
                        error: None,
                    },
                    Err(e) => CatchUpSummary {
                        source: sub.source.clone(),
                        symbol: sub.symbol.clone(),
                        asset_class: "prediction_markets".to_string(),
                        timeframe: tf.to_string(),
                        gap_days: 0,
                        candles_ingested: 0,
                        status: "persist_error".to_string(),
                        error: Some(e.to_string()),
                    },
                }
            }
            Err(e) => CatchUpSummary {
                source: sub.source.clone(),
                symbol: sub.symbol.clone(),
                asset_class: "prediction_markets".to_string(),
                timeframe: tf.to_string(),
                gap_days: 0,
                candles_ingested: 0,
                status: "fetch_error".to_string(),
                error: Some(e.to_string()),
            },
        }
    }
}
