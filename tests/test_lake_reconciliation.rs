//! Automated Test Suite for Multi-Source Candle Reconciliation & Drift Auditing.
//! Verifies data integrity between persisted Parquet lakehouse and live external feeds (Yahoo, Binance, Dukascopy, TradingView).

use std::path::Path;
use chrono::{NaiveDate, TimeZone, Utc};

use economy_aggregator::core::types::{AggregatorCandle, IngestionMode, TimeFrame};
use economy_aggregator::features::analytics::{
    DiscrepancyType, ReconciliationEngine,
};
use economy_aggregator::features::ingestion::{
    BinanceRestFetcher, DukascopyParser, TradingViewParser, YahooFinanceFetcher,
};
use economy_aggregator::features::storage::LakeReader;

fn make_candle(
    date: NaiveDate,
    symbol: &str,
    close: f64,
    volume: f64,
    source: &str,
) -> AggregatorCandle {
    AggregatorCandle {
        timestamp_utc: Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()),
        canonical_symbol: symbol.to_string(),
        timeframe: TimeFrame::D1,
        open: close * 0.99,
        high: close * 1.01,
        low: close * 0.98,
        close,
        volume_base: volume,
        volume_quote: close * volume,
        taker_buy_volume_base: None,
        taker_buy_volume_quote: None,
        source: source.to_string(),
        raw_symbol: symbol.to_string(),
        ingestion_mode: IngestionMode::RestBatch,
        is_settled: true,
        sequence_id: None,
        vwap: Some(close),
        trade_count: Some(100),
        price_type: "last".to_string(),
        raw_metadata: None,
    }
}

#[test]
fn test_reconciliation_exact_match_zero_drift() {
    let d1 = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
    let d3 = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();

    let lake = vec![
        make_candle(d1, "BBCA", 10000.0, 1000.0, "lake"),
        make_candle(d2, "BBCA", 10050.0, 1200.0, "lake"),
        make_candle(d3, "BBCA", 10100.0, 950.0, "lake"),
    ];

    let live = vec![
        make_candle(d1, "BBCA", 10000.0, 1000.0, "yahoo"),
        make_candle(d2, "BBCA", 10050.0, 1200.0, "yahoo"),
        make_candle(d3, "BBCA", 10100.0, 950.0, "yahoo"),
    ];

    let report = ReconciliationEngine::compare_candles("BBCA", &lake, &live, 0.5);

    assert_eq!(report.matched_bars, 3);
    assert_eq!(report.exact_matches, 3);
    assert_eq!(report.drifted_bars_count, 0);
    assert_eq!(report.missing_in_lake_count, 0);
    assert_eq!(report.missing_in_live_count, 0);
    assert!(report.max_close_drift_pct < 0.0001);
    assert!(report.is_valid);
    assert!(report.discrepancies.is_empty());
}

#[test]
fn test_reconciliation_detects_price_drift_exceeding_tolerance() {
    let d1 = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
    let d3 = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();

    let lake = vec![
        make_candle(d1, "GSPC", 5000.0, 1000.0, "lake"),
        make_candle(d2, "GSPC", 5050.0, 1200.0, "lake"),
        make_candle(d3, "GSPC", 5100.0, 950.0, "lake"),
    ];

    // Day 2 has a 2.0% drift (5050 vs 5151)
    let live = vec![
        make_candle(d1, "GSPC", 5000.0, 1000.0, "yahoo"),
        make_candle(d2, "GSPC", 5151.0, 1200.0, "yahoo"),
        make_candle(d3, "GSPC", 5100.0, 950.0, "yahoo"),
    ];

    let report = ReconciliationEngine::compare_candles("GSPC", &lake, &live, 0.5);

    assert_eq!(report.matched_bars, 3);
    assert_eq!(report.drifted_bars_count, 1);
    assert!(!report.is_valid);
    assert_eq!(report.discrepancies.len(), 1);
    assert_eq!(report.discrepancies[0].date, d2);
    assert_eq!(report.discrepancies[0].discrepancy_type, DiscrepancyType::PriceDrift);
    assert!((report.discrepancies[0].drift_pct - 2.0).abs() < 0.05);
}

#[test]
fn test_reconciliation_detects_missing_bars() {
    let d1 = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
    let d3 = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let d4 = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();

    // Lake is missing d2
    let lake = vec![
        make_candle(d1, "TLT", 90.0, 100.0, "lake"),
        make_candle(d3, "TLT", 92.0, 100.0, "lake"),
        make_candle(d4, "TLT", 93.0, 100.0, "lake"),
    ];

    // Live is missing d3 (within d1..d4)
    let live = vec![
        make_candle(d1, "TLT", 90.0, 100.0, "live"),
        make_candle(d2, "TLT", 91.0, 100.0, "live"),
        make_candle(d4, "TLT", 93.0, 100.0, "live"),
    ];

    let report = ReconciliationEngine::compare_candles("TLT", &lake, &live, 0.5);

    assert_eq!(report.matched_bars, 2); // d1 and d4
    assert_eq!(report.missing_in_lake_count, 1); // d2 is missing in lake
    assert_eq!(report.missing_in_live_count, 1); // d3 is missing in live
    assert!(!report.is_valid);
}

#[test]
fn test_reconciliation_daily_series_comparison() {
    let d1 = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();

    let lake = vec![(d1, 100.0), (d2, 102.0)];
    let live = vec![(d1, 100.1), (d2, 102.0)];

    let report = ReconciliationEngine::compare_daily_series("TEST", &lake, &live, 0.5);
    assert_eq!(report.matched_bars, 2);
    assert_eq!(report.exact_matches, 1); // d2 is exact
    assert_eq!(report.drifted_bars_count, 0); // 0.1% is below 0.5% tolerance
    assert!(report.is_valid);
}

#[test]
fn test_reconciliation_dukascopy_array_and_object_parsing() {
    let object_json = r#"[
        {"timestamp": 1704067200000, "open": 1.1035, "high": 1.1050, "low": 1.1020, "close": 1.1042, "volume": 25400.0}
    ]"#;
    let array_json = r#"[
        [1704153600000, 1.1042, 1.1060, 1.1030, 1.1055, 31200.0]
    ]"#;

    let obj_candles = DukascopyParser::parse_json(object_json, "EUR/USD", TimeFrame::D1).unwrap();
    assert_eq!(obj_candles.len(), 1);
    assert_eq!(obj_candles[0].close, 1.1042);
    assert_eq!(obj_candles[0].source, "dukascopy");

    let arr_candles = DukascopyParser::parse_json(array_json, "EUR/USD", TimeFrame::D1).unwrap();
    assert_eq!(arr_candles.len(), 1);
    assert_eq!(arr_candles[0].close, 1.1055);
    assert_eq!(arr_candles[0].source, "dukascopy");

    // Reconcile Dukascopy against simulated lake
    let mut combined_live = obj_candles;
    combined_live.extend(arr_candles);

    let lake_candles = vec![
        make_candle(combined_live[0].timestamp_utc.date_naive(), "EUR/USD", 1.1042, 25400.0, "lake"),
        make_candle(combined_live[1].timestamp_utc.date_naive(), "EUR/USD", 1.1055, 31200.0, "lake"),
    ];

    let report = ReconciliationEngine::compare_candles("EUR/USD", &lake_candles, &combined_live, 0.1);
    assert_eq!(report.matched_bars, 2);
    assert_eq!(report.exact_matches, 2);
    assert!(report.is_valid);
}

#[test]
fn test_reconciliation_tradingview_udf_parsing() {
    let udf_json = r#"{
        "s": "ok",
        "t": [1704067200, 1704153600],
        "o": [100.0, 101.0],
        "h": [102.0, 103.0],
        "l": [99.0, 100.5],
        "c": [101.5, 102.5],
        "v": [50000.0, 60000.0]
    }"#;

    let tv_candles = TradingViewParser::parse_udf_json(udf_json, "AAPL", TimeFrame::D1).unwrap();
    assert_eq!(tv_candles.len(), 2);
    assert_eq!(tv_candles[0].close, 101.5);
    assert_eq!(tv_candles[1].close, 102.5);
    assert_eq!(tv_candles[0].source, "tradingview");

    let lake_candles = vec![
        make_candle(tv_candles[0].timestamp_utc.date_naive(), "AAPL", 101.5, 50000.0, "lake"),
        make_candle(tv_candles[1].timestamp_utc.date_naive(), "AAPL", 102.5, 60000.0, "lake"),
    ];

    let report = ReconciliationEngine::compare_candles("AAPL", &lake_candles, &tv_candles, 0.1);
    assert_eq!(report.matched_bars, 2);
    assert_eq!(report.exact_matches, 2);
    assert!(report.is_valid);
}

#[tokio::test]
async fn test_live_lake_reconciliation_real_parquet_and_yahoo() {
    let lake_dir = Path::new("data/lake");
    if !lake_dir.exists() {
        println!("⏭️ data/lake does not exist, skipping real Parquet test");
        return;
    }

    // Read real Parquet candles from lake for BBCA
    let start_date = Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let end_date = Some(NaiveDate::from_ymd_opt(2024, 1, 31).unwrap());

    let lake_candles = LakeReader::read_candles(
        lake_dir,
        Some("equities"),
        "BBCA",
        "1d",
        start_date,
        end_date,
    );

    let lake_bars = match lake_candles {
        Ok(b) if !b.is_empty() => b,
        _ => {
            println!("⏭️ No BBCA Parquet bars found for Jan 2024, skipping live comparison");
            return;
        }
    };

    println!("📊 Read {} real BBCA Parquet bars from lake", lake_bars.len());

    // Fetch live from Yahoo
    let fetcher = YahooFinanceFetcher::default();
    match fetcher.fetch_daily_bars("BBCA.JK", "BBCA", "5y").await {
        Ok(live_bars) => {
            let report = ReconciliationEngine::compare_candles("BBCA", &lake_bars, &live_bars, 1.0);
            println!(
                "✅ Live Yahoo reconciliation complete: matched={}, exact={}, max_drift={:.3}%, is_valid={}",
                report.matched_bars, report.exact_matches, report.max_close_drift_pct, report.is_valid
            );
            assert!(report.matched_bars > 0, "Expected at least 1 matching trading day");
        }
        Err(e) => {
            println!("⏭️ Skipping live Yahoo call due to network/rate limit: {}", e);
        }
    }
}

#[tokio::test]
async fn test_live_lake_reconciliation_real_parquet_and_binance() {
    let lake_dir = Path::new("data/lake");
    if !lake_dir.exists() {
        return;
    }

    // Read real Parquet candles from lake for BTC_USDT (recent period)
    let start_date = Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let end_date = Some(NaiveDate::from_ymd_opt(2024, 1, 31).unwrap());

    let lake_bars = match LakeReader::read_candles(
        lake_dir,
        Some("crypto"),
        "BTC_USDT",
        "1d",
        start_date,
        end_date,
    ) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            println!("⏭️ No BTC_USDT Parquet bars found for Jan 2024, skipping");
            return;
        }
    };

    println!("📊 Read {} real BTC_USDT Parquet bars from lake", lake_bars.len());

    let fetcher = BinanceRestFetcher::default();
    match fetcher.fetch_klines("BTCUSDT", "BTC/USDT", "1d", None, None, Some(100)).await {
        Ok(live_bars) => {
            let report = ReconciliationEngine::compare_candles("BTC/USDT", &lake_bars, &live_bars, 1.0);
            println!(
                "✅ Live Binance reconciliation complete: matched={}, exact={}, max_drift={:.3}%",
                report.matched_bars, report.exact_matches, report.max_close_drift_pct
            );
        }
        Err(e) => {
            println!("⏭️ Skipping live Binance call due to network/rate limit: {}", e);
        }
    }
}
