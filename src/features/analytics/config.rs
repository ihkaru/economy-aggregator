//! Declarative Portfolio Specifications & Presets.
//!
//! Eliminates procedural hardcoding of symbols, yields, and allocation weights.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSpec {
    pub symbol: String,
    pub canonical_name: String,
    pub asset_class: String,
    pub weight: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSpec {
    pub name: String,
    pub base_currency: String,
    pub assets: Vec<AssetSpec>,
    pub benchmark: Option<String>,
}

impl PortfolioSpec {
    /// Institutional All-Weather / Risk-Parity empirical portfolio using 100% real market instruments:
    /// - 25% Real Long-Term Treasury ETF (TLT)
    /// - 25% Real Intermediate Treasury ETF (IEF)
    /// - 15% Real Investment Grade Corporate Bond ETF (LQD)
    /// - 12.5% Real S&P 500 Equity Index (GSPC)
    /// - 12.5% Real Indo Banking Equity (BBCA)
    /// - 10% Real Spot Gold (XAU_USD)
    pub fn all_weather_empirical() -> Self {
        Self {
            name: "all-weather-empirical".to_string(),
            base_currency: "IDR".to_string(),
            assets: vec![
                AssetSpec {
                    symbol: "TLT".to_string(),
                    canonical_name: "TLT_LONG_BOND".to_string(),
                    asset_class: "equities".to_string(),
                    weight: 0.25,
                    currency: "USD".to_string(),
                },
                AssetSpec {
                    symbol: "IEF".to_string(),
                    canonical_name: "IEF_INTERMEDIATE_BOND".to_string(),
                    asset_class: "equities".to_string(),
                    weight: 0.25,
                    currency: "USD".to_string(),
                },
                AssetSpec {
                    symbol: "LQD".to_string(),
                    canonical_name: "LQD_CORP_BOND".to_string(),
                    asset_class: "equities".to_string(),
                    weight: 0.15,
                    currency: "USD".to_string(),
                },
                AssetSpec {
                    symbol: "GSPC".to_string(),
                    canonical_name: "SP500".to_string(),
                    asset_class: "equities".to_string(),
                    weight: 0.125,
                    currency: "USD".to_string(),
                },
                AssetSpec {
                    symbol: "BBCA".to_string(),
                    canonical_name: "BBCA".to_string(),
                    asset_class: "equities".to_string(),
                    weight: 0.125,
                    currency: "IDR".to_string(),
                },
                AssetSpec {
                    symbol: "XAU_USD".to_string(),
                    canonical_name: "GOLD".to_string(),
                    asset_class: "commodities".to_string(),
                    weight: 0.10,
                    currency: "USD".to_string(),
                },
            ],
            benchmark: Some("JKSE".to_string()),
        }
    }
}
