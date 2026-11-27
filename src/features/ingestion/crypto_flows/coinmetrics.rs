//! CoinMetrics Community API Keyless Connector: BTC Exchange Inflow/Outflow/Netflow (Phase 319).

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, ObsParams,
};

#[derive(Debug, Deserialize)]
struct CmResp {
    data: Vec<CmRow>,
    #[serde(default)]
    next_page_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CmRow {
    asset: String,
    time: String,
    #[serde(rename = "FlowInExUSD")]
    flow_in_ex_usd: Option<String>,
    #[serde(rename = "FlowInExUSD-status")]
    flow_in_status: Option<String>,
    #[serde(rename = "FlowOutExUSD")]
    flow_out_ex_usd: Option<String>,
    #[serde(rename = "FlowOutExUSD-status")]
    flow_out_status: Option<String>,
}

pub struct CoinMetricsFetcher {
    client: Client,
    base_url: String,
}

impl Default for CoinMetricsFetcher {
    fn default() -> Self {
        Self::new("https://community-api.coinmetrics.io".to_string())
    }
}

impl CoinMetricsFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_btc_exchange_netflow(
        &self,
        start_date: Option<&str>,
        limit: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!(
            "{}/v4/timeseries/asset-metrics?assets=btc&metrics=FlowInExUSD,FlowOutExUSD&page_size={}",
            self.base_url,
            limit.clamp(1, 10_000)
        );
        if let Some(sd) = start_date {
            url.push_str(&format!("&start_time={}", sd));
        }
        let now = Utc::now();
        let mut out = Vec::new();
        for _ in 0..20 {
            let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
            let parsed: CmResp = serde_json::from_slice(&bytes)?;
            for r in parsed.data {
                let Ok(obs_ts) = DateTime::parse_from_rfc3339(&r.time).map(|d| d.with_timezone(&Utc)) else {
                    continue;
                };
                let Some(in_usd) = r.flow_in_ex_usd.as_deref().and_then(|s| s.parse::<f64>().ok()) else {
                    continue;
                };
                let Some(out_usd) = r.flow_out_ex_usd.as_deref().and_then(|s| s.parse::<f64>().ok()) else {
                    continue;
                };
                let netflow = in_usd - out_usd;
                let status_in = r.flow_in_status.as_deref().unwrap_or("unknown");
                let rev = if status_in == "final" { 1 } else { 0 };
                let extra = serde_json::json!({
                    "asset": r.asset,
                    "flow_in_ex_usd": in_usd,
                    "flow_out_ex_usd": out_usd,
                    "in_status": status_in,
                    "out_status": r.flow_out_status,
                })
                .to_string();

                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at: now,
                    revision_seq: rev,
                    series_id: "onchain:BTC:coinmetrics_netflow_usd",
                    domain: "onchain_flow",
                    compartment: Some("crypto"),
                    entity_id: "BTC",
                    metric_name: "exchange_netflow_usd",
                    frequency: ObservationFrequency::Daily,
                    value: netflow,
                    raw_value: Some(netflow),
                    unit: "usd",
                    source: "coinmetrics_community",
                    source_series_id: "btc:FlowInExUSD-FlowOutExUSD",
                    source_url: &url,
                    source_tier: SourceTier::CommunityApi,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: Some(extra),
                }));
            }
            match parsed.next_page_url {
                Some(next) if !next.trim().is_empty() => url = next,
                _ => break,
            }
        }
        if out.is_empty() {
            return Err("Fail-fast: CoinMetrics returned 0 BTC exchange netflow rows".into());
        }
        Ok(out)
    }
}
