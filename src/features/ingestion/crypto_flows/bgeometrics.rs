//! BGeometrics Open Keyless Connector: Spot BTC ETF Flow & Multi-Exchange Futures OI (Phase 318).

use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc, ObsParams,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BgEtfRow {
    d: String,
    etf_flow: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BgOiRow {
    d: String,
    binance: Option<String>,
    bybit: Option<String>,
    okx: Option<String>,
    bitget: Option<String>,
    kucoin: Option<String>,
    open_interest_futures: Option<String>,
}

pub struct BGeometricsFetcher {
    client: Client,
    base_url: String,
}

impl Default for BGeometricsFetcher {
    fn default() -> Self {
        Self::new("https://bitcoin-data.com".to_string())
    }
}

impl BGeometricsFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_btc_etf_flows(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/v1/etf-flow-btc", self.base_url);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<BgEtfRow> = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(rows.len());

        for r in rows {
            let Some(obs_ts) = parse_date_eod_utc(&r.d) else { continue };
            let Some(val) = r.etf_flow.as_deref().and_then(|s| s.parse::<f64>().ok()) else {
                continue;
            };
            // BGeometrics reports net ETF flow in BTC units (preserved faithfully with explicit unit).
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: "etf:BTC_US:bgeometrics_flow_btc",
                domain: "etf_flow",
                compartment: Some("crypto"),
                entity_id: "BTC_US_ETFS",
                metric_name: "net_flow_btc",
                frequency: ObservationFrequency::Daily,
                value: val,
                raw_value: Some(val),
                unit: "btc",
                source: "bgeometrics",
                source_series_id: "/v1/etf-flow-btc",
                source_url: &url,
                source_tier: SourceTier::UnofficialJson,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        if out.is_empty() {
            return Err("Fail-fast: BGeometrics /v1/etf-flow-btc returned 0 valid rows".into());
        }
        Ok(out)
    }

    pub async fn fetch_futures_open_interest(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/v1/open-interest-futures", self.base_url);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<BgOiRow> = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(rows.len());

        for r in rows {
            let Some(obs_ts) = parse_date_eod_utc(&r.d) else { continue };
            let Some(total) = r
                .open_interest_futures
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
            else {
                continue;
            };
            let extra = serde_json::json!({
                "binance": r.binance,
                "bybit": r.bybit,
                "okx": r.okx,
                "bitget": r.bitget,
                "kucoin": r.kucoin,
            })
            .to_string();
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: "deriv:BTC:bgeometrics_oi_usd",
                domain: "derivatives_oi",
                compartment: Some("deleveraging"),
                entity_id: "BTC",
                metric_name: "open_interest_usd",
                frequency: ObservationFrequency::Daily,
                value: total,
                raw_value: Some(total),
                unit: "usd",
                source: "bgeometrics",
                source_series_id: "/v1/open-interest-futures",
                source_url: &url,
                source_tier: SourceTier::UnofficialJson,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: Some(extra),
            }));
        }
        if out.is_empty() {
            return Err("Fail-fast: BGeometrics /v1/open-interest-futures returned 0 valid rows".into());
        }
        Ok(out)
    }
}
