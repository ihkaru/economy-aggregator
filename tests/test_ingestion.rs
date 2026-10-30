use economy_aggregator::core::types::TimeFrame;
use economy_aggregator::features::ingestion::{
    extract_rate_limit_weight, parse_binance_message, BinanceKlineEvent, BinanceWsClient,
    BinanceWsConfig,
};
use reqwest::header::{HeaderMap, HeaderValue};

#[test]
fn test_binance_single_kline_payload_parse() {
    let raw = r#"{
        "e": "kline",
        "E": 1672531200000,
        "s": "BTCUSDT",
        "k": {
            "t": 1672531200000,
            "T": 1672531259999,
            "s": "BTCUSDT",
            "i": "1m",
            "f": 100,
            "L": 200,
            "o": "16500.50",
            "c": "16520.00",
            "h": "16530.00",
            "l": "16490.00",
            "v": "10.5",
            "n": 42,
            "x": true,
            "q": "173360.00",
            "V": "6.2",
            "Q": "102300.00",
            "B": "0"
        }
    }"#;

    let event = BinanceKlineEvent::parse_json(raw).expect("valid JSON kline");
    assert_eq!(event.symbol, "BTCUSDT");
    assert_eq!(event.kline.trade_count, 42);
    assert!(event.kline.is_closed);

    let candle = event.to_aggregator_candle("BTC/USDT", TimeFrame::M1).unwrap();
    assert_eq!(candle.canonical_symbol, "BTC/USDT");
    assert_eq!(candle.open, 16500.50);
    assert_eq!(candle.high, 16530.00);
    assert_eq!(candle.low, 16490.00);
    assert_eq!(candle.close, 16520.00);
    assert_eq!(candle.volume_base, 10.5);
    assert_eq!(candle.taker_buy_volume_base, Some(6.2));
    assert!(candle.is_settled);
}

#[test]
fn test_binance_combined_stream_parse() {
    let raw = r#"{
        "stream": "btcusdt@kline_1m",
        "data": {
            "e": "kline",
            "E": 1672531200000,
            "s": "BTCUSDT",
            "k": {
                "t": 1672531200000,
                "s": "BTCUSDT",
                "i": "1m",
                "o": "20000.00",
                "c": "20050.00",
                "h": "20100.00",
                "l": "19950.00",
                "v": "50.0",
                "q": "1000000.00",
                "V": "25.0",
                "Q": "500000.00",
                "n": 120,
                "x": false
            }
        }
    }"#;

    let event = parse_binance_message(raw).expect("parsed combined stream");
    assert_eq!(event.symbol, "BTCUSDT");
    assert!(!event.kline.is_closed);
}

#[test]
fn test_binance_ws_url_generation() {
    // Single stream
    let cfg_single = BinanceWsConfig {
        base_url: "wss://stream.binance.com:9443".to_string(),
        streams: vec!["btcusdt@kline_1m".to_string()],
        max_reconnect_attempts: 5,
        initial_backoff_secs: 1,
    };
    let client_single = BinanceWsClient::new(cfg_single);
    assert_eq!(
        client_single.build_url(),
        "wss://stream.binance.com:9443/ws/btcusdt@kline_1m"
    );

    // Multi streams (multiplexed)
    let cfg_multi = BinanceWsConfig {
        base_url: "wss://stream.binance.com:9443".to_string(),
        streams: vec![
            "btcusdt@kline_1m".to_string(),
            "ethusdt@kline_1m".to_string(),
        ],
        max_reconnect_attempts: 5,
        initial_backoff_secs: 1,
    };
    let client_multi = BinanceWsClient::new(cfg_multi);
    assert_eq!(
        client_multi.build_url(),
        "wss://stream.binance.com:9443/stream?streams=btcusdt@kline_1m/ethusdt@kline_1m"
    );
}

#[test]
fn test_rate_limit_weight_extraction() {
    let mut headers = HeaderMap::new();
    headers.insert("x-mbx-used-weight-1m", HeaderValue::from_static("45"));

    let weight = extract_rate_limit_weight(&headers);
    assert_eq!(weight, Some(45));

    let empty_headers = HeaderMap::new();
    assert_eq!(extract_rate_limit_weight(&empty_headers), None);
}
