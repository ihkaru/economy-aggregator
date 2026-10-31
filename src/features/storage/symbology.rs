//! Symbology Rosetta Stone registry for instant multi-vendor ticker resolution,
//! permanent identifiers (ISIN / OpenFIGI / PermID), and temporal point-in-time ticker lineage.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use crate::core::types::{AssetClass, InstrumentType};

/// Historical ticker alias with point-in-time validity bounds.
/// Handles company rebranding (e.g. FB -> META), symbol migrations, or reverse-split ticker changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickerAlias {
    pub raw_symbol: String,
    pub vendor: String,
    #[serde(default)]
    pub valid_from: Option<NaiveDate>,
    #[serde(default)]
    pub valid_to: Option<NaiveDate>,
}

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
    #[serde(default)]
    pub isin: Option<String>,                    // e.g. "ID1000106703" (Permanent ISIN)
    #[serde(default)]
    pub figi: Option<String>,                    // e.g. "BBG000MM2P62" (OpenFIGI)
    #[serde(default)]
    pub perm_id: Option<u64>,                    // Permanent numeric ID (CRSP-style)
    #[serde(default)]
    pub aliases: Vec<TickerAlias>,               // Temporal historical ticker drift
}

impl InstrumentMapping {
    pub fn with_identifiers(
        mut self,
        isin: Option<String>,
        figi: Option<String>,
        perm_id: Option<u64>,
    ) -> Self {
        self.isin = isin;
        self.figi = figi;
        self.perm_id = perm_id;
        self
    }

    pub fn with_alias(mut self, alias: TickerAlias) -> Self {
        self.aliases.push(alias);
        self
    }
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

    /// Resolves vendor-specific raw ticker to canonical symbol using current active mappings.
    pub fn resolve_vendor(&self, vendor: &str, raw_symbol: &str) -> Option<&str> {
        self.vendor_lookup
            .get(&(vendor.to_lowercase(), raw_symbol.to_uppercase()))
            .map(|s| s.as_str())
    }

    /// Resolves a ticker at a specific point in time (Point-in-Time Temporal Symbology).
    /// Respects ticker drift: checks temporal aliases matching the requested `as_of` date first,
    /// and gracefully falls back to the active vendor mapping.
    pub fn resolve_at(&self, vendor: &str, raw_symbol: &str, as_of: NaiveDate) -> Option<&InstrumentMapping> {
        let v_lower = vendor.to_lowercase();
        let r_upper = raw_symbol.to_uppercase();

        // 1. Check temporal aliases
        for mapping in self.by_canonical.values() {
            for alias in &mapping.aliases {
                if alias.vendor.eq_ignore_ascii_case(&v_lower) && alias.raw_symbol.eq_ignore_ascii_case(&r_upper) {
                    let after_start = alias.valid_from.map_or(true, |start| as_of >= start);
                    let before_end = alias.valid_to.map_or(true, |end| as_of <= end);
                    if after_start && before_end {
                        return Some(mapping);
                    }
                }
            }
        }

        // 2. Fallback to active static mapping
        if let Some(canonical) = self.resolve_vendor(vendor, raw_symbol) {
            return self.get(canonical);
        }

        None
    }

    /// Resolves canonical symbol name at a specific point in time.
    pub fn resolve_canonical_at(&self, vendor: &str, raw_symbol: &str, as_of: NaiveDate) -> Option<&str> {
        self.resolve_at(vendor, raw_symbol, as_of).map(|m| m.canonical_symbol.as_str())
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
