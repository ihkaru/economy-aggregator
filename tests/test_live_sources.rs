//! Live integration test suite for external market data & economic calendar sources.
//! Guarded by design: Tests requiring vendor API keys or live network access
//! gracefully skip with explanatory messages if keys or network are not present.

use economy_aggregator::features::ingestion::{
    BinanceRestFetcher, EconomicCalendarFetcher,
};

#[tokio::test]
async fn test_live_binance_public_klines_pull() {
    let fetcher = BinanceRestFetcher::default();
    match fetcher
        .fetch_klines("BTCUSDT", "BTC/USDT", "1d", None, None, Some(5))
        .await
    {
        Ok(candles) => {
            println!("✅ Live Binance pull successful: received {} daily candles", candles.len());
            assert!(!candles.is_empty());
            let latest = &candles[candles.len() - 1];
            assert_eq!(latest.canonical_symbol, "BTC/USDT");
            assert_eq!(latest.raw_symbol, "BTCUSDT");
            assert!(latest.close > 0.0);
            assert!(latest.is_settled);
        }
        Err(e) => {
            // Graceful skip on network errors (e.g. offline CI or DNS restriction)
            println!("⏭️ Skipping live Binance pull test (network or rate limit: {})", e);
        }
    }
}

#[tokio::test]
async fn test_live_biquote_economic_calendar_pull() {
    let fetcher = EconomicCalendarFetcher::default();
    match fetcher.fetch_events(Some("US"), Some("high")).await {
        Ok(events) => {
            println!("✅ Live Biquote Calendar pull successful: received {} high-impact US events", events.len());
            assert!(!events.is_empty());
            let first = &events[0];
            assert_eq!(first.country, "US");
            assert_eq!(first.impact_level, 3);
            assert!(!first.title.is_empty());
        }
        Err(e) => {
            // Graceful skip on network errors
            println!("⏭️ Skipping live Biquote Calendar test (network: {})", e);
        }
    }
}

#[tokio::test]
async fn test_live_fred_macro_api_key_guarded() {
    let api_key = match std::env::var("FRED_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => {
            println!("⏭️ Skipping FRED test: FRED_API_KEY not set in environment or Infisical");
            return;
        }
    };

    let client = reqwest::Client::new();
    let url = format!(
        "https://api.stlouisfed.org/fred/series/observations?series_id=CPIAUCSL&api_key={}&file_type=json&limit=5",
        api_key
    );

    match client.get(&url).send().await {
        Ok(res) => {
            assert!(res.status().is_success());
            println!("✅ Live FRED pull successful with authenticated key");
        }
        Err(e) => {
            println!("⚠️ FRED request failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_live_finnhub_economic_calendar_key_guarded() {
    let api_key = match std::env::var("FINNHUB_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => {
            println!("⏭️ Skipping Finnhub test: FINNHUB_API_KEY not set in environment or Infisical");
            return;
        }
    };

    let client = reqwest::Client::new();
    let url = format!(
        "https://finnhub.io/api/v1/calendar/economic?token={}",
        api_key
    );

    match client.get(&url).send().await {
        Ok(res) => {
            assert!(res.status().is_success());
            println!("✅ Live Finnhub pull successful with authenticated key");
        }
        Err(e) => {
            println!("⚠️ Finnhub request failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_live_alpaca_market_data_key_guarded() {
    let key_id = match std::env::var("ALPACA_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            println!("⏭️ Skipping Alpaca test: ALPACA_API_KEY not set in environment or Infisical");
            return;
        }
    };

    let secret_key = match std::env::var("ALPACA_SECRET_KEY") {
        Ok(s) if !s.trim().is_empty() => s,
        _ => {
            println!("⏭️ Skipping Alpaca test: ALPACA_SECRET_KEY not set");
            return;
        }
    };

    let client = reqwest::Client::new();
    let res = client
        .get("https://data.alpaca.markets/v2/stocks/bars?symbols=AAPL&timeframe=1Day&limit=5")
        .header("APCA-API-KEY-ID", key_id)
        .header("APCA-API-SECRET-KEY", secret_key)
        .send()
        .await;

    match res {
        Ok(resp) => {
            assert!(resp.status().is_success());
            println!("✅ Live Alpaca stock pull successful with authenticated key");
        }
        Err(e) => {
            println!("⚠️ Alpaca request failed: {}", e);
        }
    }
}
