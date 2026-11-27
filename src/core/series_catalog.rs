//! Series Metadata & Econometric Compartment Catalog (Phase 307).

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::core::observation_types::ObservationFrequency;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesMetadata {
    pub series_id: String,
    pub domain: String,
    pub compartment: Option<String>,
    pub entity_id: String,
    pub metric_name: String,
    pub frequency: ObservationFrequency,
    pub unit: String,
    pub description: String,
    pub primary_source: String,
    pub fallback_sources: Vec<String>,
    pub limitation_note: Option<String>,
}

pub struct SeriesCatalog {
    entries: Vec<SeriesMetadata>,
    by_id: HashMap<String, usize>,
}

impl Default for SeriesCatalog {
    fn default() -> Self {
        Self::new_canonical()
    }
}

impl SeriesCatalog {
    pub fn from_entries(entries: Vec<SeriesMetadata>) -> Self {
        let mut by_id = HashMap::with_capacity(entries.len());
        for (idx, e) in entries.iter().enumerate() {
            by_id.insert(e.series_id.clone(), idx);
        }
        Self { entries, by_id }
    }

    pub fn get(&self, series_id: &str) -> Option<&SeriesMetadata> {
        self.by_id.get(series_id).and_then(|i| self.entries.get(*i))
    }

    pub fn list(&self) -> &[SeriesMetadata] {
        &self.entries
    }

    /// Canonical registry covering all 6 closed-system liquidity compartments & 2 macro controls.
    pub fn new_canonical() -> Self {
        let m = |series_id: &str,
                 domain: &str,
                 compartment: Option<&str>,
                 entity_id: &str,
                 metric_name: &str,
                 frequency: ObservationFrequency,
                 unit: &str,
                 description: &str,
                 primary_source: &str,
                 fallback: &[&str],
                 limitation: Option<&str>| SeriesMetadata {
            series_id: series_id.to_string(),
            domain: domain.to_string(),
            compartment: compartment.map(str::to_string),
            entity_id: entity_id.to_string(),
            metric_name: metric_name.to_string(),
            frequency,
            unit: unit.to_string(),
            description: description.to_string(),
            primary_source: primary_source.to_string(),
            fallback_sources: fallback.iter().map(|s| (*s).to_string()).collect(),
            limitation_note: limitation.map(str::to_string),
        };

        let entries = vec![
            m("etf:SPY:aum_usd", "etf_flow", Some("equities"), "SPY", "aum_usd", ObservationFrequency::Daily, "usd", "SPDR S&P 500 ETF Trust Total Net Assets (SSGA)", "ssga_xlsx", &[], Some("Substitutes ICI weekly retail mutual fund flows (no public API)")),
            m("etf:SPY:shares_outstanding", "etf_flow", Some("equities"), "SPY", "shares_outstanding", ObservationFrequency::Daily, "shares", "SPDR S&P 500 ETF Trust Exact Shares Outstanding (SSGA)", "ssga_xlsx", &[], None),
            m("etf:SPY:stockanalysis_shares_outstanding", "etf_snapshot", Some("equities"), "SPY", "shares_outstanding", ObservationFrequency::Daily, "shares", "StockAnalysis Rounded Shares Snapshot (SPY)", "stockanalysis", &[], Some("Rounded frontend snapshot (1.06B scale); strictly separated from exact SSGA XLSX series")),
            m("etf:QQQ:stockanalysis_shares_outstanding", "etf_snapshot", Some("equities"), "QQQ", "shares_outstanding", ObservationFrequency::Daily, "shares", "Invesco QQQ Trust Rounded Shares Snapshot", "stockanalysis", &[], Some("Forward-accumulating daily snapshot from StockAnalysis (no 10y backfill API)")),
            m("etf:GLD:aum_usd", "etf_flow", Some("gold"), "GLD", "aum_usd", ObservationFrequency::Daily, "usd", "SPDR Gold Trust Total Net Assets (SSGA)", "ssga_xlsx", &[], Some("Proxies physical gold ETFs (GLD full history; IAU CSV gated)")),
            m("etf:GLD:shares_outstanding", "etf_flow", Some("gold"), "GLD", "shares_outstanding", ObservationFrequency::Daily, "shares", "SPDR Gold Trust Exact Shares Outstanding (SSGA)", "ssga_xlsx", &[], None),
            m("cot:GOLD:managed_money_net", "positioning_cot", Some("gold"), "COMEX_GOLD", "managed_money_net", ObservationFrequency::Weekly, "contracts", "CFTC Disaggregated COT Managed Money Net Gold Futures", "cftc_cot", &[], Some("Tuesday snapshot released Friday (3-day known_at lag)")),
            m("cot:GOLD:open_interest_contracts", "positioning_cot", Some("gold"), "COMEX_GOLD", "open_interest_contracts", ObservationFrequency::Weekly, "contracts", "CFTC Disaggregated COT Total Gold Futures Open Interest", "cftc_cot", &[], None),
            m("etf:BTC_US:bgeometrics_flow_btc", "etf_flow", Some("crypto"), "BTC_US_ETFS", "net_flow_btc", ObservationFrequency::Daily, "btc", "US Spot Bitcoin ETF Aggregate Daily Net Flow in BTC", "bgeometrics", &[], Some("Covers US Spot BTC ETFs from Jan 2024; ETH ETF requires paid SoSoValue/CoinGlass")),
            m("onchain:BTC:coinmetrics_netflow_usd", "onchain_flow", Some("crypto"), "BTC", "exchange_netflow_usd", ObservationFrequency::Daily, "usd", "BTC Net Inflow-Outflow Across Tracked Exchanges (USD)", "coinmetrics_community", &[], Some("CoinMetrics flash/final BTC netflow; Glassnode/CryptoQuant paywalled for multi-asset")),
            m("onchain:BTC:github_netflow_btc", "onchain_flow", Some("crypto"), "BTC", "exchange_netflow_btc", ObservationFrequency::Daily, "btc", "BTC Net Exchange Inflow-Outflow in BTC (GitHub Mirror)", "github_ercindedeoglu", &[], Some("Independent BTC-denominated mirror; strictly isolated from CoinMetrics USD series")),
            m("macro:US:MMF_ASSETS_USD", "macro_liquidity", Some("fiat_park"), "US", "mmf_assets_usd", ObservationFrequency::Monthly, "usd", "US Treasury OFR Money Market Funds Total Assets", "ofr_stfm", &[], Some("Replaces quarterly FRED MMMFFAQ027S with monthly official OFR series")),
            m("macro:US:ON_RRP_USD", "macro_liquidity", Some("fiat_park"), "US", "on_rrp_usd", ObservationFrequency::Daily, "usd", "NY Fed Overnight Reverse Repo Operations Total Accepted", "nyfed_api", &[], None),
            m("macro:US:TBILL_3M_PCT", "macro_liquidity", Some("fiat_park"), "US", "tbill_3m_pct", ObservationFrequency::Daily, "percent", "US Treasury 13-Week T-Bill Bank Discount Rate", "treasury_tbill_csv", &[], None),
            m("stablecoin:ALL:mcap_usd", "stablecoin", Some("crypto_park"), "ALL", "market_cap_usd", ObservationFrequency::Daily, "usd", "Total Global Stablecoin Circulating Market Cap", "defillama", &[], None),
            m("stablecoin:USDT:mcap_usd", "stablecoin", Some("crypto_park"), "USDT", "market_cap_usd", ObservationFrequency::Daily, "usd", "Tether (USDT) Circulating Market Cap", "defillama", &[], None),
            m("stablecoin:USDC:mcap_usd", "stablecoin", Some("crypto_park"), "USDC", "market_cap_usd", ObservationFrequency::Daily, "usd", "USD Coin (USDC) Circulating Market Cap", "defillama", &[], None),
            m("deriv:BTC:bgeometrics_oi_usd", "derivatives_oi", Some("deleveraging"), "BTC", "open_interest_usd", ObservationFrequency::Daily, "usd", "Aggregate BTC Futures Open Interest (Multi-Venue Sum, USD)", "bgeometrics", &[], Some("BGeometrics aggregate since Nov 2022; never mixed into exchange-specific OI series")),
            m("deriv:BTC:github_oi_usd", "derivatives_oi", Some("deleveraging"), "BTC", "open_interest_usd", ObservationFrequency::Daily, "usd", "Aggregate BTC Open Interest USD (GitHub Mirror)", "github_ercindedeoglu", &[], Some("Independent mirror coverage; strictly isolated from BGeometrics series")),
            m("deriv:EQ_OPT:open_interest_contracts", "derivatives_oi", Some("deleveraging"), "OCC_TOTAL", "open_interest_contracts", ObservationFrequency::Daily, "contracts", "OCC Total US Equity/Index/Futures Options Open Interest", "occ_csv", &[], Some("OCC rolling 24-month window; pair with CFTC S&P 500 Futures OI for 10y span")),
            m("cot:SP500:open_interest_contracts", "positioning_cot", Some("deleveraging"), "CME_SP500", "open_interest_contracts", ObservationFrequency::Weekly, "contracts", "CFTC E-Mini S&P 500 Futures Open Interest (10y Equity Derivatives Proxy)", "cftc_cot", &[], None),
            m("macro:US:BANK_CREDIT_USD", "macro_liquidity", Some("deleveraging"), "US", "bank_credit_usd", ObservationFrequency::Weekly, "usd", "Federal Reserve H.8 All Commercial Banks Total Credit", "dbnomics_fed", &[], Some("DBnomics FED/H8 B1001NCBA (equivalent to TOTBKCR, ~2-3 week sync lag)")),
            m("credit:US:FINRA_MARGIN_DEBT_USD", "margin_debt", Some("deleveraging"), "US", "margin_debit_usd", ObservationFrequency::Monthly, "usd", "FINRA Customer Securities Margin Debit Balances", "finra_xlsx", &[], Some("Monthly release (~21 day known_at lag); no weekly granularity")),
            m("macro:US:FED_TOTAL_ASSETS_USD", "macro_liquidity", Some("macro_control"), "US", "fed_total_assets_usd", ObservationFrequency::Weekly, "usd", "Federal Reserve H.4.1 Total Assets Snapshot (Thursday Release HTML)", "fed_h41_html", &[], Some("Direct Thursday Federal Reserve release total assets; isolated from DBnomics reserve factors")),
            m("macro:US:FED_H41_RESERVE_FACTORS_USD", "macro_liquidity", Some("macro_control"), "US", "fed_reserve_factors_usd", ObservationFrequency::Weekly, "usd", "Federal Reserve H.4.1 Factors Supplying Reserve Funds Backfill (DBnomics)", "dbnomics_fed", &[], Some("DBnomics FED/H41 RESH4S_N.WW backfill (~2-3 week lag); strictly separated from H.4.1 HTML Total Assets")),
            m("macro:US:TGA_BALANCE_USD", "macro_liquidity", Some("macro_control"), "US", "tga_balance_usd", ObservationFrequency::Daily, "usd", "US Treasury General Account Daily Closing/Opening Balance", "treasury_fiscaldata", &[], None),
            m("macro:US:FRED_WALCL_USD", "macro_liquidity", Some("macro_control"), "US", "fed_total_assets_usd", ObservationFrequency::Weekly, "usd", "Fed Total Assets Less Eliminations from Consolidation (FRED WALCL)", "fred_api", &[], Some("Official 10y+ weekly Fed balance sheet via FRED REST v1; strictly isolated from fed_h41_html")),
            m("macro:US:FRED_WTREGEN_USD", "macro_liquidity", Some("macro_control"), "US", "tga_balance_usd", ObservationFrequency::Weekly, "usd", "Treasury General Account Weekly Average (FRED WTREGEN)", "fred_api", &[], Some("Weekly Wednesday Treasury balance from FRED; strictly isolated from daily Treasury FiscalData")),
            m("macro:US:FRED_RRPONTSYD_USD", "macro_liquidity", Some("fiat_park"), "US", "on_rrp_usd", ObservationFrequency::Daily, "usd", "Overnight Reverse Repurchase Agreements Award (FRED RRPONTSYD)", "fred_api", &[], Some("10y+ daily ON RRP history from FRED; strictly isolated from nyfed_api")),
            m("macro:US:FRED_TOTBKCR_USD", "macro_liquidity", Some("deleveraging"), "US", "bank_credit_usd", ObservationFrequency::Weekly, "usd", "Bank Credit, All Commercial Banks (FRED TOTBKCR)", "fred_api", &[], Some("Weekly Fed H.8 bank credit via FRED; strictly isolated from DBnomics")),
            m("macro:US:FRED_DTB3_PCT", "macro_liquidity", Some("fiat_park"), "US", "tbill_3m_pct", ObservationFrequency::Daily, "percent", "3-Month Treasury Bill Secondary Market Rate (FRED DTB3)", "fred_api", &[], None),
            m("macro:US:FRED_T10Y2Y_PCT", "macro_liquidity", Some("macro_control"), "US", "yield_curve_10y2y_pct", ObservationFrequency::Daily, "percent", "10-Year Minus 2-Year Treasury Yield Spread (FRED T10Y2Y)", "fred_api", &[], None),
            m("macro:US:FRED_HY_OAS_PCT", "macro_liquidity", Some("deleveraging"), "US", "hy_credit_spread_pct", ObservationFrequency::Daily, "percent", "ICE BofA US High Yield Index Option-Adjusted Spread (FRED BAMLH0A0HYM2)", "fred_api", &[], Some("Free FRED redistribution window may span ~3 recent years for ICE BofA index series")),
        ];

        Self::from_entries(entries)
    }
}
