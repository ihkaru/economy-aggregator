use chrono::{TimeZone, Utc};
use economy_aggregator::core::types::{
    AggregatorCandle, IngestionMode, TimeFrame,
};
use economy_aggregator::features::ingestion::{SanityError, SanityGate};

fn sample_candle(open: f64, high: f64, low: f64, close: f64) -> AggregatorCandle {
    AggregatorCandle {
        timestamp_utc: Utc.with_ymd_and_hms(2026, 10, 30, 14, 0, 0).unwrap(),
        canonical_symbol: "BTC/USDT".to_string(),
        timeframe: TimeFrame::M1,
        open,
        high,
        low,
        close,
        volume_base: 1.0,
        volume_quote: 100.0,
        source: "binance".to_string(),
        raw_symbol: "BTCUSDT".to_string(),
        ingestion_mode: IngestionMode::LiveWs,
        is_settled: false,
        sequence_id: None,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        vwap: None,
        trade_count: None,
        price_type: "last".to_string(),
        raw_metadata: None,
    }
}

#[test]
fn test_sanity_gate_valid_candle() {
    let gate = SanityGate::new();
    let candle = sample_candle(100.0, 105.0, 95.0, 102.0);
    assert!(gate.validate(&candle).is_ok());
}

#[test]
fn test_sanity_gate_inverted_high_low() {
    let gate = SanityGate::new();
    let candle = sample_candle(100.0, 90.0, 105.0, 102.0);
    assert_eq!(gate.validate(&candle), Err(SanityError::InvalidHighLow));
}

#[test]
fn test_sanity_gate_negative_price() {
    let gate = SanityGate::new();
    let candle = sample_candle(-10.0, 105.0, 95.0, 102.0);
    assert_eq!(gate.validate(&candle), Err(SanityError::NonPositivePrice));
}

#[test]
fn test_sanity_gate_negative_price_allowed_in_general_mode() {
    let gate = SanityGate::new();
    // Real-world scenario: WTI crude futures during negative oil price event (open 1.0, high 2.5, low -40.32, close -37.63)
    let negative_candle = sample_candle(1.0, 2.5, -40.32, -37.63);
    assert!(gate.validate_general(&negative_candle).is_ok());
}

#[test]
fn test_gap_detection() {
    let mut gate = SanityGate::new();
    assert_eq!(gate.track_sequence(Some(100)), None);
    assert_eq!(gate.track_sequence(Some(101)), None);
    // Sequence jumps from 101 to 105: 3 missing packets
    assert_eq!(gate.track_sequence(Some(105)), Some(3));
}

#[test]
fn test_sanity_gate_outlier_detection() {
    // Valid 5% jump
    assert!(!SanityGate::is_price_outlier(100.0, 105.0, 5.0));
    // Valid 50% drop
    assert!(!SanityGate::is_price_outlier(100.0, 50.0, 5.0));
    // Outlier 10x jump
    assert!(SanityGate::is_price_outlier(100.0, 1100.0, 5.0));
    // Outlier 10x crash (e.g. dirty quote 888.11 vs 9000.0)
    assert!(SanityGate::is_price_outlier(9000.0, 888.11, 5.0));
}
