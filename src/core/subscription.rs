//! Subscription configuration management for autonomous multi-source data ingestion.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubscriptionItem {
    pub source: String,
    #[serde(alias = "raw_symbol")]
    pub symbol: String,
    pub asset_class: String,
    #[serde(default = "default_timeframes")]
    pub timeframes: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval_secs: Option<u64>,
    pub created_at_utc: String,
}

fn default_timeframes() -> Vec<String> {
    vec!["1d".to_string()]
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionsConfig {
    pub version: u32,
    pub updated_at_utc: String,
    pub subscriptions: Vec<SubscriptionItem>,
}

pub struct SubscriptionManager {
    file_path: PathBuf,
    cache: RwLock<Option<Vec<SubscriptionItem>>>,
}

impl SubscriptionManager {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            cache: RwLock::new(None),
        }
    }

    pub fn default_path(base_dir: &Path) -> PathBuf {
        base_dir.join("config").join("subscriptions.json")
    }

    /// Loads active subscriptions from persistent storage or returns in-memory cache.
    pub fn load(&self) -> Result<Vec<SubscriptionItem>, Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(guard) = self.cache.read() {
            if let Some(items) = &*guard {
                return Ok(items.clone());
            }
        }

        if !self.file_path.exists() {
            let defaults = self.generate_core_defaults();
            self.save(&defaults)?;
            return Ok(defaults);
        }

        let content = fs::read_to_string(&self.file_path)?;
        let mut parsed: SubscriptionsConfig = serde_json::from_str(&content)?;
        let defaults = self.generate_core_defaults();
        let mut changed = false;
        for def in defaults {
            let exists = parsed.subscriptions.iter().any(|s| {
                s.source.eq_ignore_ascii_case(&def.source) && s.symbol.eq_ignore_ascii_case(&def.symbol)
            });
            if !exists {
                parsed.subscriptions.push(def);
                changed = true;
            }
        }
        if changed {
            let _ = self.save(&parsed.subscriptions);
        }
        
        if let Ok(mut guard) = self.cache.write() {
            *guard = Some(parsed.subscriptions.clone());
        }

        Ok(parsed.subscriptions)
    }

    /// Atomically writes subscriptions list to persistent JSON file.
    pub fn save(&self, items: &[SubscriptionItem]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let config = SubscriptionsConfig {
            version: 1,
            updated_at_utc: Utc::now().to_rfc3339(),
            subscriptions: items.to_vec(),
        };

        let json = serde_json::to_string_pretty(&config)?;
        let temp_path = self.file_path.with_extension("tmp");
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(json.as_bytes())?;
            file.flush()?;
        }
        fs::rename(temp_path, &self.file_path)?;

        if let Ok(mut guard) = self.cache.write() {
            *guard = Some(items.to_vec());
        }

        Ok(())
    }

    /// Adds or updates a subscription idempotently by composite key (source, symbol).
    pub fn subscribe(&self, mut item: SubscriptionItem) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut current = self.load()?;
        item.created_at_utc = Utc::now().to_rfc3339();
        
        let mut replaced = false;
        for existing in current.iter_mut() {
            if existing.symbol.eq_ignore_ascii_case(&item.symbol)
                && existing.source.eq_ignore_ascii_case(&item.source)
            {
                *existing = item.clone();
                replaced = true;
                break;
            }
        }

        if !replaced {
            current.push(item);
        }

        self.save(&current)?;
        Ok(replaced)
    }

    /// Removes an existing subscription by symbol, optionally filtered by source.
    /// If source is None, removes all subscriptions matching symbol across all sources.
    pub fn unsubscribe(
        &self,
        source: Option<&str>,
        symbol: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut current = self.load()?;
        let initial_len = current.len();

        current.retain(|s| {
            if !s.symbol.eq_ignore_ascii_case(symbol) {
                return true; // Keep other symbols
            }
            if let Some(src) = source {
                // Keep if source does not match
                !s.source.eq_ignore_ascii_case(src)
            } else {
                // If no source specified, remove all matches for this symbol
                false
            }
        });

        if current.len() != initial_len {
            self.save(&current)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lists only active/enabled subscriptions.
    pub fn list_active(&self) -> Result<Vec<SubscriptionItem>, Box<dyn std::error::Error + Send + Sync>> {
        let all = self.load()?;
        Ok(all.into_iter().filter(|s| s.enabled).collect())
    }

    /// Generates core default subscriptions covering all asset classes and sources
    /// (OHLCV candles: Yahoo, Binance, TradingView; non-candle: Macro Calendar, Polymarket, Kalshi).
    fn generate_core_defaults(&self) -> Vec<SubscriptionItem> {
        let now = Utc::now().to_rfc3339();
        let make = |source: &str, symbol: &str, asset_class: &str, tfs: &[&str], poll: Option<u64>| {
            SubscriptionItem {
                source: source.to_string(),
                symbol: symbol.to_string(),
                asset_class: asset_class.to_string(),
                timeframes: tfs.iter().map(|t| t.to_string()).collect(),
                enabled: true,
                poll_interval_secs: poll,
                created_at_utc: now.clone(),
            }
        };
        vec![
            // 1. Yahoo Finance — Global & IDX Equities, Bonds, FX, Commodities (OHLCV Candles)
            make("yahoo", "BBCA.JK", "equities", &["1d"], Some(300)),
            make("yahoo", "^GSPC", "equities", &["1d"], Some(300)),
            make("yahoo", "TLT", "equities", &["1d"], Some(300)),
            make("yahoo", "USDIDR=X", "forex", &["1d"], Some(60)),
            make("yahoo", "GC=F", "commodities", &["1d"], Some(60)),
            // 2. Binance — Spot Crypto (OHLCV Candles)
            make("binance", "BTCUSDT", "crypto", &["1d", "1h"], None),
            // 3. TradingView — Multi-Venue Equities, Crypto & Gold (OHLCV Candles)
            make("tradingview", "IDX:BBCA", "equities", &["1d"], Some(300)),
            make("tradingview", "COINBASE:BTCUSD", "crypto", &["1d", "4h"], Some(300)),
            make("tradingview", "OANDA:XAUUSD", "commodities", &["1d"], Some(300)),
            // 4. Biquote — Macroeconomic Calendar Releases (Non-Candle Events)
            make("biquote", "US,ID,EU,GB", "macro", &["events"], Some(600)),
            // 5. Polymarket & Kalshi — Prediction Market Contracts & Implied Probabilities (Non-Candle)
            make("polymarket", "TOP_ACTIVE", "prediction_markets", &["1h"], Some(600)),
            make("kalshi", "TOP_ACTIVE", "prediction_markets", &["1h"], Some(600)),
            // 6. Yahoo Finance Liquidity Proxies & Benchmark Indices (^MOVE Bond Vol, DXY, ETFs)
            make("yahoo", "DX-Y.NYB", "forex", &["1d"], Some(300)),
            make("yahoo", "^MOVE", "equities", &["1d"], Some(300)),
            make("yahoo", "SPY", "equities", &["1d"], Some(300)),
            make("yahoo", "QQQ", "equities", &["1d"], Some(300)),
            make("yahoo", "GLD", "commodities", &["1d"], Some(300)),
            make("yahoo", "IAU", "commodities", &["1d"], Some(300)),
            make("yahoo", "IBIT", "equities", &["1d"], Some(300)),
            make("yahoo", "BIL", "equities", &["1d"], Some(300)),
            make("yahoo", "JNK", "equities", &["1d"], Some(300)),
            make("yahoo", "SPTL", "equities", &["1d"], Some(300)),
            // 7. Closed-System Liquidity & Flow Observations (Frequency-Aware Cadence)
            make("liquidity_macro", "US_MACRO_LIQUIDITY", "observations", &["1d"], Some(21_600)),
            make("tradfi_flows", "TRADFI_FLOWS_COT", "observations", &["1d"], Some(21_600)),
            make("crypto_flows", "CRYPTO_FLOWS_OI", "observations", &["1d"], Some(3_600)),
        ]
    }
}
