use chrono::{TimeZone, Utc};
use economy_aggregator::core::types::{
    AggregatorCandle, IngestionMode, TimeFrame,
};
use economy_aggregator::features::ingestion::{SanityError, SanityGate};

fn sample_candle(open: f64, high: f64, low: f64, close: f64) -> AggregatorCandle {
    let dt = Utc.with_ymd_and_hms(2026, 10, 30, 14, 0, 0).unwrap();
    AggregatorCandle {
        time: dt.timestamp(),
        timestamp_utc: dt,
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

#[test]
fn test_deterministic_file_line_limits_under_400() {
    use std::fs;
    use std::path::Path;

    fn check_dir(dir: &Path, max_limit: usize, violations: &mut Vec<(String, usize)>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let file_name = p.file_name().unwrap_or_default().to_string_lossy();
                if file_name.starts_with('.') || file_name == "node_modules" || file_name == "target" || file_name == "dist" {
                    continue;
                }
                if p.is_dir() {
                    check_dir(&p, max_limit, violations);
                } else if let Some(ext) = p.extension() {
                    let ext_str = ext.to_string_lossy();
                    if matches!(ext_str.as_ref(), "rs" | "svelte" | "ts" | "js" | "css") {
                        if let Ok(content) = fs::read_to_string(&p) {
                            let line_count = content.lines().count();
                            if line_count > max_limit {
                                violations.push((p.display().to_string(), line_count));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut violations = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    check_dir(&root.join("src"), 400, &mut violations);
    check_dir(&root.join("tests"), 400, &mut violations);
    check_dir(&root.join("web").join("src"), 400, &mut violations);
    check_dir(&root.join("web").join("tests"), 400, &mut violations);

    if !violations.is_empty() {
        let msg = violations
            .iter()
            .map(|(path, lines)| format!("{}: {} lines", path, lines))
            .collect::<Vec<_>>()
            .join("\n");
        panic!("Found files exceeding deterministic 400-line ceiling:\n{}", msg);
    }
}
