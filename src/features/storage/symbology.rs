//! Symbology Rosetta Stone registry for instant multi-vendor ticker resolution.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::core::types::{AssetClass, InstrumentType};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstrumentMapping {
    pub canonical_symbol: String,
    pub name: String,
    pub asset_class: AssetClass,
    pub instrument_type: InstrumentType,
    pub base_currency: String,
    pub quote_currency: String,
    pub market_timezone: String,
    pub is_24_7: bool,
    pub vendor_symbols: HashMap<String, String>, // {"binance": "BTCUSDT", "yahoo": "BTC-USD"}
    pub contract_address: Option<String>,
    pub network: Option<String>,
}

#[derive(Default)]
pub struct InstrumentRegistry {
    by_canonical: HashMap<String, InstrumentMapping>,
    vendor_lookup: HashMap<(String, String), String>, // (vendor, raw_symbol) -> canonical_symbol
}

impl InstrumentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, mapping: InstrumentMapping) {
        for (vendor, raw) in &mapping.vendor_symbols {
            self.vendor_lookup.insert(
                (vendor.to_lowercase(), raw.to_uppercase()),
                mapping.canonical_symbol.clone(),
            );
        }
        self.by_canonical.insert(mapping.canonical_symbol.clone(), mapping);
    }

    pub fn resolve_vendor(&self, vendor: &str, raw_symbol: &str) -> Option<&str> {
        self.vendor_lookup
            .get(&(vendor.to_lowercase(), raw_symbol.to_uppercase()))
            .map(|s| s.as_str())
    }

    pub fn get(&self, canonical_symbol: &str) -> Option<&InstrumentMapping> {
        self.by_canonical.get(canonical_symbol)
    }

    pub fn len(&self) -> usize {
        self.by_canonical.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_canonical.is_empty()
    }

    pub fn list_all(&self) -> Vec<InstrumentMapping> {
        self.by_canonical.values().cloned().collect()
    }

    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let list: Vec<&InstrumentMapping> = self.by_canonical.values().collect();
        let json = serde_json::to_string_pretty(&list)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(path, json)
    }

    pub fn load_from_file(&mut self, path: &Path) -> std::io::Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(path)?;
        let list: Vec<InstrumentMapping> = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        for item in list {
            self.register(item);
        }
        Ok(())
    }
}
