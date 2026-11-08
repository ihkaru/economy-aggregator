//! Dynamic Resampler: Aggregates daily OHLCV candles to higher timeframes (1w, 1M).
//! Follows industry standard TradingView UDF aggregation rules.

use chrono::Datelike;
use crate::core::types::{AggregatorCandle, TimeFrame};

pub struct CandleResampler;

impl CandleResampler {
    /// Resamples a slice of daily candles to the specified target timeframe.
    /// If target timeframe is already 1d or intraday, returns the candles as-is.
    pub fn resample(candles: &[AggregatorCandle], target: TimeFrame) -> Vec<AggregatorCandle> {
        match target {
            TimeFrame::W1 => Self::resample_to_weekly(candles),
            TimeFrame::MN1 => Self::resample_to_monthly(candles),
            _ => candles.to_vec(),
        }
    }

    /// Resamples daily candles into weekly (1w) candles based on ISO calendar week.
    pub fn resample_to_weekly(daily: &[AggregatorCandle]) -> Vec<AggregatorCandle> {
        if daily.is_empty() {
            return Vec::new();
        }

        let mut weekly = Vec::new();
        let mut current_group: Vec<&AggregatorCandle> = Vec::with_capacity(7);
        let mut current_key: Option<(i32, u32)> = None;

        for candle in daily {
            let key = (
                candle.timestamp_utc.iso_week().year(),
                candle.timestamp_utc.iso_week().week(),
            );

            if current_key == Some(key) {
                current_group.push(candle);
            } else {
                if !current_group.is_empty() {
                    weekly.push(Self::aggregate_group(&current_group, TimeFrame::W1));
                    current_group.clear();
                }
                current_key = Some(key);
                current_group.push(candle);
            }
        }

        if !current_group.is_empty() {
            weekly.push(Self::aggregate_group(&current_group, TimeFrame::W1));
        }

        weekly
    }

    /// Resamples daily candles into monthly (1M) candles based on calendar month.
    pub fn resample_to_monthly(daily: &[AggregatorCandle]) -> Vec<AggregatorCandle> {
        if daily.is_empty() {
            return Vec::new();
        }

        let mut monthly = Vec::new();
        let mut current_group: Vec<&AggregatorCandle> = Vec::with_capacity(31);
        let mut current_key: Option<(i32, u32)> = None;

        for candle in daily {
            let key = (candle.timestamp_utc.year(), candle.timestamp_utc.month());

            if current_key == Some(key) {
                current_group.push(candle);
            } else {
                if !current_group.is_empty() {
                    monthly.push(Self::aggregate_group(&current_group, TimeFrame::MN1));
                    current_group.clear();
                }
                current_key = Some(key);
                current_group.push(candle);
            }
        }

        if !current_group.is_empty() {
            monthly.push(Self::aggregate_group(&current_group, TimeFrame::MN1));
        }

        monthly
    }

    fn aggregate_group(bars: &[&AggregatorCandle], tf: TimeFrame) -> AggregatorCandle {
        let first = bars[0];
        let last = bars[bars.len() - 1];

        let mut high = first.high;
        let mut low = first.low;
        let mut volume_base = 0.0;
        let mut volume_quote = 0.0;
        let mut all_settled = true;

        for b in bars {
            if b.high > high {
                high = b.high;
            }
            if b.low < low {
                low = b.low;
            }
            volume_base += b.volume_base;
            volume_quote += b.volume_quote;
            if !b.is_settled {
                all_settled = false;
            }
        }

        AggregatorCandle {
            time: first.time,
            timestamp_utc: first.timestamp_utc,
            canonical_symbol: first.canonical_symbol.clone(),
            timeframe: tf,
            open: first.open,
            high,
            low,
            close: last.close,
            volume_base,
            volume_quote,
            source: first.source.clone(),
            raw_symbol: first.raw_symbol.clone(),
            ingestion_mode: first.ingestion_mode,
            is_settled: all_settled,
            sequence_id: None,
            taker_buy_volume_base: None,
            taker_buy_volume_quote: None,
            vwap: None,
            trade_count: None,
            price_type: first.price_type.clone(),
            raw_metadata: None,
        }
    }
}
