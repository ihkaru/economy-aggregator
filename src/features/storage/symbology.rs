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

    /// Seeds standard canonical instruments with authentic market data vendor mappings (zero "parquet" placeholders).
    pub fn seed_core_instruments(&mut self) {
        let entries = [
            ("BBCA", "Bank Central Asia", AssetClass::Equities, InstrumentType::Spot, "IDR", "IDR", "Asia/Jakarta", "yahoo", "BBCA.JK"),
            ("GSPC", "S&P 500 Index", AssetClass::Equities, InstrumentType::Index, "USD", "USD", "America/New_York", "yahoo", "^GSPC"),
            ("JKSE", "Jakarta Composite Index (IHSG)", AssetClass::Equities, InstrumentType::Index, "IDR", "IDR", "Asia/Jakarta", "yahoo", "^JKSE"),
            ("N225", "Nikkei 225 Index", AssetClass::Equities, InstrumentType::Index, "JPY", "JPY", "Asia/Tokyo", "yahoo", "^N225"),
            ("TLT", "iShares 20+ Year Treasury Bond", AssetClass::Equities, InstrumentType::Spot, "USD", "USD", "America/New_York", "yahoo", "TLT"),
            ("IEF", "iShares 7-10 Year Treasury Bond", AssetClass::Equities, InstrumentType::Spot, "USD", "USD", "America/New_York", "yahoo", "IEF"),
            ("LQD", "iShares Investment Grade Corporate Bond", AssetClass::Equities, InstrumentType::Spot, "USD", "USD", "America/New_York", "yahoo", "LQD"),
            ("XAU/USD", "Gold Spot / US Dollar", AssetClass::Commodities, InstrumentType::Spot, "XAU", "USD", "UTC", "yahoo", "GC=F"),
            ("USD/IDR", "US Dollar / Indonesian Rupiah", AssetClass::Forex, InstrumentType::Spot, "USD", "IDR", "Asia/Jakarta", "yahoo", "USDIDR=X"),
            ("BTC/USDT", "Bitcoin / Tether USD", AssetClass::Crypto, InstrumentType::Spot, "BTC", "USDT", "UTC", "binance", "BTCUSDT"),
            ("ETH/USDT", "Ethereum / Tether USD", AssetClass::Crypto, InstrumentType::Spot, "ETH", "USDT", "UTC", "binance", "ETHUSDT"),
            ("EUR/USD", "Euro / US Dollar", AssetClass::Forex, InstrumentType::Spot, "EUR", "USD", "America/New_York", "oanda", "EUR_USD"),
        ];

        for (sym, name, ac, it, base, quote, tz, vendor, raw) in entries {
            let mut vendors = HashMap::new();
            vendors.insert(vendor.to_string(), raw.to_string());
            if sym == "BTC/USDT" {
                vendors.insert("yahoo".to_string(), "BTC-USD".to_string());
                vendors.insert("coinbase".to_string(), "BTC-USD".to_string());
            } else if sym == "EUR/USD" {
                vendors.insert("yahoo".to_string(), "EURUSD=X".to_string());
            }

            self.register(InstrumentMapping {
                canonical_symbol: sym.to_string(),
                name: name.to_string(),
                asset_class: ac,
                instrument_type: it,
                base_currency: base.to_string(),
                quote_currency: quote.to_string(),
                market_timezone: tz.to_string(),
                is_24_7: ac == AssetClass::Crypto,
                vendor_symbols: vendors,
                contract_address: None,
                network: None,
                isin: None,
                figi: None,
                perm_id: None,
                aliases: Vec::new(),
            });
        }
    }
}

pub fn friendly_symbol_name(sym: &str) -> &'static str {
    match sym.trim().to_uppercase().as_str() {
        "GSPC" => "S&P 500 Index",
        "BBCA" => "Bank Central Asia",
        "JKSE" => "Jakarta Composite Index (IHSG)",
        "N225" => "Nikkei 225 Index",
        "TLT" => "iShares 20+ Year Treasury Bond",
        "IEF" => "iShares 7-10 Year Treasury Bond",
        "LQD" => "iShares Investment Grade Corporate Bond",
        "XAU_USD" | "XAU/USD" => "Gold Spot / US Dollar",
        "USD_IDR" | "USD/IDR" => "US Dollar / Indonesian Rupiah",
        "BTC_USDT" | "BTC/USDT" => "Bitcoin / Tether USD",
        _ => "Market Instrument",
    }
}

pub fn resolve_instrument_specs(sym: &str, asset_class: &str) -> (String, usize, f64, Vec<String>, bool) {
    let clean = sym.to_uppercase();
    let (currency, precision, min_tick) = match asset_class {
        "crypto" => ("USDT".to_string(), 2, 0.01),
        "forex" => {
            if clean.contains("IDR") {
                ("IDR".to_string(), 2, 0.5)
            } else {
                ("USD".to_string(), 4, 0.0001)
            }
        }
        "commodities" => ("USD".to_string(), 2, 0.05),
        "macro" => ("PCT".to_string(), 2, 0.01),
        _ => {
            if clean.ends_with(".JK") || clean == "BBCA" || clean == "JKSE" {
                ("IDR".to_string(), 0, 25.0)
            } else if clean == "N225" {
                ("JPY".to_string(), 2, 0.01)
            } else {
                ("USD".to_string(), 2, 0.01)
            }
        }
    };

    let has_intraday = asset_class == "crypto";
    let supported_tf = if has_intraday {
        vec!["1m".into(), "5m".into(), "15m".into(), "1h".into(), "1d".into(), "1w".into(), "1M".into()]
    } else {
        vec!["1d".into(), "1w".into(), "1M".into()]
    };

    (currency, precision, min_tick, supported_tf, has_intraday)
}

/// Resolves the canonical upstream vendor source for an instrument (e.g. yahoo, binance, oanda).
/// Eliminates synthetic storage placeholders like "parquet_lake".
pub fn resolve_instrument_vendor(sym: &str, asset_class: &str) -> &'static str {
    match asset_class {
        "crypto" => "binance",
        "forex" => {
            if sym.to_uppercase().contains("EUR") {
                "oanda"
            } else {
                "yahoo"
            }
        }
        _ => "yahoo",
    }
}
