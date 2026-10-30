//! Data validation gate and stream sequence gap detection.

use crate::core::types::AggregatorCandle;

#[derive(Debug, PartialEq, Eq)]
pub enum SanityError {
    InvalidHighLow,
    NonPositivePrice,
    NegativeVolume,
    CorruptTimestamp,
    AbnormalOutlier,
}

pub struct SanityGate {
    last_sequence_id: Option<u64>,
}

impl Default for SanityGate {
    fn default() -> Self {
        Self::new()
    }
}

impl SanityGate {
    pub fn new() -> Self {
        Self {
            last_sequence_id: None,
        }
    }

    /// Verifies core mathematical invariants of a candlestick.
    pub fn validate(&self, candle: &AggregatorCandle) -> Result<(), SanityError> {
        if candle.open <= 0.0 || candle.close <= 0.0 || candle.high <= 0.0 || candle.low <= 0.0 {
            return Err(SanityError::NonPositivePrice);
        }

        if candle.high < candle.low || candle.high < candle.open || candle.high < candle.close {
            return Err(SanityError::InvalidHighLow);
        }

        if candle.low > candle.open || candle.low > candle.close {
            return Err(SanityError::InvalidHighLow);
        }

        if candle.volume_base < 0.0 || candle.volume_quote < 0.0 {
            return Err(SanityError::NegativeVolume);
        }

        Ok(())
    }

    /// Detects abnormal price spikes/glitches across daily intervals (e.g. 10x sudden drop).
    pub fn is_price_outlier(prev_close: f64, current_close: f64, max_jump_ratio: f64) -> bool {
        if prev_close <= 0.0 || current_close <= 0.0 {
            return false;
        }
        let ratio = current_close / prev_close;
        ratio > max_jump_ratio || ratio < (1.0 / max_jump_ratio)
    }

    /// Audits sequence numbers to detect dropped packets / stream gaps.
    /// Returns Some(missing_count) if a gap was detected.
    pub fn track_sequence(&mut self, sequence_id: Option<u64>) -> Option<u64> {
        let current = sequence_id?;
        let gap = match self.last_sequence_id {
            Some(prev) if current > prev + 1 => Some(current - prev - 1),
            _ => None,
        };
        self.last_sequence_id = Some(current);
        gap
    }
}
