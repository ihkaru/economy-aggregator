//! GitHub Active Dataset Fallback: ErcinDedeoglu/crypto-market-data with Commit Provenance (Phase 319).

use chrono::{TimeZone, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, ObsParams,
};

#[derive(Debug, Deserialize)]
struct GithubMirrorFile {
    name: Option<String>,
    data: Vec<GithubMirrorPoint>,
}

#[derive(Debug, Deserialize)]
struct GithubMirrorPoint {
    timestamp: i64,
    value: Option<f64>,
    last_modified: Option<i64>,
}

pub struct GithubCryptoMirrorFetcher {
    client: Client,
    raw_base: String,
}

impl Default for GithubCryptoMirrorFetcher {
    fn default() -> Self {
        Self::new("https://raw.githubusercontent.com/ErcinDedeoglu/crypto-market-data/main/data/daily".to_string())
    }
}

impl GithubCryptoMirrorFetcher {
    pub fn new(raw_base: String) -> Self {
        Self {
            client: build_browser_client(),
            raw_base: raw_base.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_series(
        &self,
        filename: &str,
        series_id: &str,
        domain: &str,
        compartment: Option<&str>,
        entity_id: &str,
        metric_name: &str,
        unit: &str,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/{}", self.raw_base, filename);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let parsed: GithubMirrorFile = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let dataset_label = parsed.name.unwrap_or_else(|| filename.to_string());
        let mut out = Vec::with_capacity(parsed.data.len());

        for pt in parsed.data {
            let Some(val) = pt.value else { continue };
            let Some(obs_ts) = Utc.timestamp_millis_opt(pt.timestamp).single() else {
                continue;
            };
            let known_at = pt
                .last_modified
                .and_then(|ms| Utc.timestamp_millis_opt(ms).single())
                .unwrap_or(now);
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at,
                revision_seq: 0,
                series_id,
                domain,
                compartment,
                entity_id,
                metric_name,
                frequency: ObservationFrequency::Daily,
                value: val,
                raw_value: Some(val),
                unit,
                source: "github_ercindedeoglu",
                source_series_id: filename,
                source_url: &url,
                source_tier: SourceTier::GithubMirror,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: Some(serde_json::json!({"dataset_name": dataset_label}).to_string()),
            }));
        }
        Ok(out)
    }
}
