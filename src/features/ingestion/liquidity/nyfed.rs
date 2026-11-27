//! NY Fed Markets Keyless Connector: Overnight Reverse Repo (ON RRP) Daily Accepted Volume (Phase 311).

use chrono::{Duration, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc, ObsParams,
};

#[derive(Debug, Deserialize)]
struct NyFedRepoResp {
    repo: NyFedRepoInner,
}

#[derive(Debug, Deserialize)]
struct NyFedRepoInner {
    operations: Vec<NyFedOp>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NyFedOp {
    operation_id: Option<String>,
    operation_date: String,
    operation_type: Option<String>,
    total_amt_accepted: Option<f64>,
}

pub struct NyFedFetcher {
    client: Client,
    base_url: String,
}

impl Default for NyFedFetcher {
    fn default() -> Self {
        Self::new("https://markets.newyorkfed.org".to_string())
    }
}

impl NyFedFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_on_rrp(
        &self,
        lookback_days: i64,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let start = (Utc::now() - Duration::days(lookback_days.max(30)))
            .format("%Y-%m-%d")
            .to_string();
        let url = format!(
            "{}/api/rp/reverserepo/propositions/search.json?startDate={}",
            self.base_url, start
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let parsed: NyFedRepoResp = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::new();

        for op in parsed.repo.operations {
            if let Some(ref ty) = op.operation_type {
                if !ty.to_ascii_lowercase().contains("reverse repo") {
                    continue;
                }
            }
            let (Some(obs_ts), Some(amt)) = (parse_date_eod_utc(&op.operation_date), op.total_amt_accepted) else {
                continue;
            };
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: "macro:US:ON_RRP_USD",
                domain: "macro_liquidity",
                compartment: Some("fiat_park"),
                entity_id: "US",
                metric_name: "on_rrp_usd",
                frequency: ObservationFrequency::Daily,
                value: amt,
                raw_value: Some(amt),
                unit: "usd",
                source: "nyfed_api",
                source_series_id: "rp/reverserepo/propositions",
                source_url: &url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: op.operation_id.map(|id| serde_json::json!({"operation_id": id}).to_string()),
            }));
        }
        Ok(out)
    }
}
