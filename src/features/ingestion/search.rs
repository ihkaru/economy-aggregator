//! Multi-source symbol discovery and search engine across equities, crypto, macro, and prediction markets.

use std::time::Duration;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub canonical_symbol: String,
    pub raw_symbol: String,
    pub name: String,
    pub asset_class: String,
    pub source: String,
    pub exchange: Option<String>,
    pub currency: Option<String>,
    pub quote_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YahooSearchResponse {
    #[serde(default)]
    quotes: Vec<YahooQuoteItem>,
}

#[derive(Debug, Deserialize)]
struct YahooQuoteItem {
    symbol: String,
    shortname: Option<String>,
    longname: Option<String>,
    exchange: Option<String>,
    #[serde(rename = "quoteType")]
    quote_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BinanceTickerItem {
    symbol: String,
    #[serde(default)]
    #[allow(dead_code)]
    price: String,
}

#[derive(Debug, Deserialize)]
struct PolymarketSearchResponse {
    #[serde(default)]
    events: Vec<PolymarketSearchEvent>,
}

#[derive(Debug, Deserialize)]
struct PolymarketSearchEvent {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    ticker: Option<String>,
    slug: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KalshiSeriesResponse {
    #[serde(default)]
    series: Vec<KalshiSeriesItem>,
}

#[derive(Debug, Deserialize)]
struct KalshiSeriesItem {
    ticker: String,
    title: String,
    category: Option<String>,
}

pub struct UnifiedSearchEngine {
    client: Client,
}

impl Default for UnifiedSearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedSearchEngine {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build HTTP search client");
        Self { client }
    }

    /// Search Yahoo Finance for equities, indices, forex, commodities, and bonds.
    pub async fn search_yahoo(&self, query: &str, limit: usize) -> Vec<SearchResultItem> {
        let url = format!(
            "https://query1.finance.yahoo.com/v1/finance/search?q={}&quotesCount={}&newsCount=0",
            urlencoding::encode(query),
            limit
        );

        let mut results = Vec::new();
        if let Ok(res) = self.client.get(&url).send().await {
            if let Ok(payload) = res.json::<YahooSearchResponse>().await {
                for q in payload.quotes.into_iter().take(limit) {
                    let name = q.longname.or(q.shortname).unwrap_or_else(|| q.symbol.clone());
                    let qtype = q.quote_type.unwrap_or_else(|| "EQUITY".to_string()).to_uppercase();
                    let asset_class = match qtype.as_str() {
                        "EQUITY" | "ETF" | "MUTUALFUND" => "equities",
                        "CURRENCY" => "forex",
                        "COMMODITY" | "FUTURE" => "commodities",
                        "INDEX" => "equities",
                        _ => "equities",
                    };
                    let canonical = q.symbol.trim_start_matches('^').replace(".JK", "").replace("=X", "").to_uppercase();
                    results.push(SearchResultItem {
                        canonical_symbol: canonical,
                        raw_symbol: q.symbol,
                        name,
                        asset_class: asset_class.to_string(),
                        source: "yahoo".to_string(),
                        exchange: q.exchange,
                        currency: None,
                        quote_type: Some(qtype),
                    });
                }
            }
        }
        results
    }

    /// Search Binance spot market pairs matching query string.
    pub async fn search_binance(&self, query: &str, limit: usize) -> Vec<SearchResultItem> {
        let urls = [
            "https://data-api.binance.vision/api/v3/ticker/price",
            "https://api.binance.com/api/v3/ticker/price",
        ];
        let query_upper = query.to_uppercase().replace('/', "").replace('_', "");
        let mut results = Vec::new();

        for url in urls {
            if let Ok(res) = self.client.get(url).send().await {
                if let Ok(tickers) = res.json::<Vec<BinanceTickerItem>>().await {
                    for t in tickers {
                        if t.symbol.contains(&query_upper) {
                            let (base, quote) = if t.symbol.ends_with("USDT") {
                                (t.symbol.trim_end_matches("USDT"), "USDT")
                            } else if t.symbol.ends_with("BTC") {
                                (t.symbol.trim_end_matches("BTC"), "BTC")
                            } else {
                                (t.symbol.as_str(), "UNKNOWN")
                            };
                            results.push(SearchResultItem {
                                canonical_symbol: format!("{}/{}", base, quote),
                                raw_symbol: t.symbol.clone(),
                                name: format!("{} / {} Spot Pair", base, quote),
                                asset_class: "crypto".to_string(),
                                source: "binance".to_string(),
                                exchange: Some("BINANCE".to_string()),
                                currency: Some(quote.to_string()),
                                quote_type: Some("CRYPTO_SPOT".to_string()),
                            });
                            if results.len() >= limit {
                                break;
                            }
                        }
                    }
                    if !results.is_empty() {
                        break;
                    }
                }
            }
        }
        results
    }

    /// Search Polymarket for public macroeconomic and geopolitical prediction markets.
    pub async fn search_polymarket(&self, query: &str, limit: usize) -> Vec<SearchResultItem> {
        let url = format!(
            "https://gamma-api.polymarket.com/public-search?q={}",
            urlencoding::encode(query)
        );

        let mut results = Vec::new();
        if let Ok(res) = self.client.get(&url).send().await {
            if let Ok(payload) = res.json::<PolymarketSearchResponse>().await {
                for event in payload.events.into_iter().take(limit) {
                    let raw = event.slug.or(event.ticker).unwrap_or(event.id.clone());
                    let canonical = format!("PM_{}", event.id);
                    results.push(SearchResultItem {
                        canonical_symbol: canonical,
                        raw_symbol: raw,
                        name: event.title,
                        asset_class: "prediction".to_string(),
                        source: "polymarket".to_string(),
                        exchange: Some("POLYMARKET".to_string()),
                        currency: Some("USD".to_string()),
                        quote_type: Some("BINARY_OUTCOME".to_string()),
                    });
                }
            }
        }
        results
    }

    /// Search Kalshi series for regulated economic and financial prediction contracts.
    pub async fn search_kalshi(&self, query: &str, limit: usize) -> Vec<SearchResultItem> {
        let url = "https://api.elections.kalshi.com/trade-api/v2/series";
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        if let Ok(res) = self.client.get(url).send().await {
            if let Ok(payload) = res.json::<KalshiSeriesResponse>().await {
                for s in payload.series {
                    if s.title.to_lowercase().contains(&query_lower) || s.ticker.to_lowercase().contains(&query_lower) {
                        results.push(SearchResultItem {
                            canonical_symbol: format!("KALSHI_{}", s.ticker),
                            raw_symbol: s.ticker,
                            name: s.title,
                            asset_class: "prediction".to_string(),
                            source: "kalshi".to_string(),
                            exchange: Some("KALSHI".to_string()),
                            currency: Some("USD".to_string()),
                            quote_type: s.category.or_else(|| Some("SERIES".to_string())),
                        });
                        if results.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }
        results
    }

    /// Discovers multi-exchange trading venues via TradingView scanner and discovery gateway.
    pub async fn search_tradingview(&self, query: &str, limit: usize) -> Vec<SearchResultItem> {
        let fetcher = crate::features::ingestion::tradingview::TradingViewFetcher::default();
        fetcher.discover_venues(query, limit).await
    }

    /// Dispatches multi-source search in parallel across all supported vendors.
    pub async fn search_all(&self, query: &str, limit_per_source: usize) -> Vec<SearchResultItem> {
        let (yahoo_res, binance_res, poly_res, kalshi_res, tv_res) = tokio::join!(
            self.search_yahoo(query, limit_per_source),
            self.search_binance(query, limit_per_source),
            self.search_polymarket(query, limit_per_source),
            self.search_kalshi(query, limit_per_source),
            self.search_tradingview(query, limit_per_source),
        );

        let mut combined = Vec::new();
        combined.extend(yahoo_res);
        combined.extend(binance_res);
        combined.extend(poly_res);
        combined.extend(kalshi_res);
        combined.extend(tv_res);
        combined
    }
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut encoded = String::new();
        for b in s.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(b as char);
                }
                b' ' => encoded.push_str("%20"),
                _ => {
                    encoded.push_str(&format!("%{:02X}", b));
                }
            }
        }
        encoded
    }
}
