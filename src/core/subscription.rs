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
        let parsed: SubscriptionsConfig = serde_json::from_str(&content)?;
        
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

    /// Generates core default subscriptions matching our existing 12 instruments.
    fn generate_core_defaults(&self) -> Vec<SubscriptionItem> {
        let now = Utc::now().to_rfc3339();
        vec![
            SubscriptionItem {
                source: "yahoo".to_string(),
                symbol: "BBCA.JK".to_string(),
                asset_class: "equities".to_string(),
                timeframes: vec!["1d".to_string()],
                enabled: true,
                poll_interval_secs: Some(300),
                created_at_utc: now.clone(),
            },
            SubscriptionItem {
                source: "yahoo".to_string(),
                symbol: "^GSPC".to_string(),
                asset_class: "equities".to_string(),
                timeframes: vec!["1d".to_string()],
                enabled: true,
                poll_interval_secs: Some(300),
                created_at_utc: now.clone(),
            },
            SubscriptionItem {
                source: "binance".to_string(),
                symbol: "BTCUSDT".to_string(),
                asset_class: "crypto".to_string(),
                timeframes: vec!["1d".to_string(), "1h".to_string()],
                enabled: true,
                poll_interval_secs: None,
                created_at_utc: now.clone(),
            },
            SubscriptionItem {
                source: "yahoo".to_string(),
                symbol: "USDIDR=X".to_string(),
                asset_class: "forex".to_string(),
                timeframes: vec!["1d".to_string()],
                enabled: true,
                poll_interval_secs: Some(60),
                created_at_utc: now.clone(),
            },
            SubscriptionItem {
                source: "yahoo".to_string(),
                symbol: "GC=F".to_string(),
                asset_class: "commodities".to_string(),
                timeframes: vec!["1d".to_string()],
                enabled: true,
                poll_interval_secs: Some(60),
                created_at_utc: now.clone(),
            },
            SubscriptionItem {
                source: "yahoo".to_string(),
                symbol: "TLT".to_string(),
                asset_class: "equities".to_string(),
                timeframes: vec!["1d".to_string()],
                enabled: true,
                poll_interval_secs: Some(300),
                created_at_utc: now.clone(),
            },
            SubscriptionItem {
                source: "biquote".to_string(),
                symbol: "US,ID,EU,GB".to_string(),
                asset_class: "macro".to_string(),
                timeframes: vec!["events".to_string()],
                enabled: true,
                poll_interval_secs: Some(600),
                created_at_utc: now,
            },
        ]
    }
}
