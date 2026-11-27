//! DefiLlama Stablecoins Keyless Connector: Total, USDT & USDC Circulating Market Cap (Phase 317).

use chrono::{TimeZone, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, ObsParams,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AllChartPoint {
    date: String,
    total_circulating: Option<PeggedUsd>,
    #[serde(rename = "totalCirculatingUSD")]
    total_circulating_usd: Option<PeggedUsd>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeggedUsd {
    #[serde(rename = "peggedUSD")]
    pegged_usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SingleStablecoinResp {
    symbol: Option<String>,
    tokens: Option<Vec<SingleTokenPoint>>,
}

#[derive(Debug, Deserialize)]
struct SingleTokenPoint {
    date: serde_json::Value,
    circulating: Option<PeggedUsd>,
}

pub struct DefiLlamaFetcher {
    client: Client,
    base_url: String,
}

impl Default for DefiLlamaFetcher {
    fn default() -> Self {
        Self::new("https://stablecoins.llama.fi".to_string())
    }
}

impl DefiLlamaFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_total_stablecoin_mcap(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/stablecoincharts/all", self.base_url);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<AllChartPoint> = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(rows.len());

        for r in rows {
            let Ok(secs) = r.date.parse::<i64>() else { continue };
            let Some(obs_ts) = Utc.timestamp_opt(secs, 0).single() else { continue };
            let Some(val) = r
                .total_circulating_usd
                .as_ref()
                .and_then(|p| p.pegged_usd)
                .or_else(|| r.total_circulating.as_ref().and_then(|p| p.pegged_usd))
            else {
                continue;
            };
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: "stablecoin:ALL:mcap_usd",
                domain: "stablecoin",
                compartment: Some("crypto_park"),
                entity_id: "ALL",
                metric_name: "market_cap_usd",
                frequency: ObservationFrequency::Daily,
                value: val,
                raw_value: Some(val),
                unit: "usd",
                source: "defillama",
                source_series_id: "/stablecoincharts/all",
                source_url: &url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        Ok(out)
    }

    pub async fn fetch_asset_stablecoin_mcap(
        &self,
        llama_id: u32,
        symbol: &str,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/stablecoin/{}", self.base_url, llama_id);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let parsed: SingleStablecoinResp = serde_json::from_slice(&bytes)?;
        let sym = parsed
            .symbol
            .unwrap_or_else(|| symbol.to_ascii_uppercase());
        let series_id = format!("stablecoin:{}:mcap_usd", sym);
        let tokens = parsed.tokens.unwrap_or_default();
        let now = Utc::now();
        let mut out = Vec::with_capacity(tokens.len());

        for t in tokens {
            let secs = t
                .date
                .as_i64()
                .or_else(|| t.date.as_str().and_then(|s| s.parse::<i64>().ok()));
            let Some(s) = secs else { continue };
            let Some(obs_ts) = Utc.timestamp_opt(s, 0).single() else { continue };
            let Some(val) = t.circulating.and_then(|c| c.pegged_usd) else { continue };
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: &series_id,
                domain: "stablecoin",
                compartment: Some("crypto_park"),
                entity_id: &sym,
                metric_name: "market_cap_usd",
                frequency: ObservationFrequency::Daily,
                value: val,
                raw_value: Some(val),
                unit: "usd",
                source: "defillama",
                source_series_id: &format!("/stablecoin/{}", llama_id),
                source_url: &url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        Ok(out)
    }
}
