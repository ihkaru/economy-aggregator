//! US Treasury Office of Financial Research (OFR) STFM Keyless Connector: MMF Total Assets (Phase 312).

use chrono::Utc;
use reqwest::Client;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc, ObsParams,
};

pub struct OfrFetcher {
    client: Client,
    base_url: String,
}

impl Default for OfrFetcher {
    fn default() -> Self {
        Self::new("https://data.financialresearch.gov".to_string())
    }
}

impl OfrFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_mmf_total_assets(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/v1/series/timeseries?mnemonic=MMF-MMF_TOT-M", self.base_url);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<(String, Option<f64>)> = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(rows.len());

        for (date_s, opt_val) in rows {
            let (Some(obs_ts), Some(usd)) = (parse_date_eod_utc(&date_s), opt_val) else {
                continue;
            };
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: "macro:US:MMF_ASSETS_USD",
                domain: "macro_liquidity",
                compartment: Some("fiat_park"),
                entity_id: "US",
                metric_name: "mmf_assets_usd",
                frequency: ObservationFrequency::Monthly,
                value: usd,
                raw_value: Some(usd),
                unit: "usd",
                source: "ofr_stfm",
                source_series_id: "MMF-MMF_TOT-M",
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
