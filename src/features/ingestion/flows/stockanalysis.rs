//! StockAnalysis Unofficial SvelteKit JSON Connector: Forward ETF Shares/Assets Snapshot (Phase 315).

use chrono::Utc;
use reqwest::Client;
use serde_json::Value;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, ObsParams,
};

pub struct StockAnalysisFetcher {
    client: Client,
    base_url: String,
}

impl Default for StockAnalysisFetcher {
    fn default() -> Self {
        Self::new("https://stockanalysis.com".to_string())
    }
}

impl StockAnalysisFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_etf_snapshot(
        &self,
        ticker: &str,
        compartment: Option<&str>,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let sym = ticker.trim().to_ascii_uppercase();
        let url = format!("{}/etf/{}/__data.json", self.base_url, sym.to_ascii_lowercase());
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let root: Value = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let data_arr = root
            .get("nodes")
            .and_then(|n| n.as_array())
            .and_then(|nodes| nodes.last())
            .and_then(|node| node.get("data"))
            .and_then(|d| d.as_array())
            .ok_or("Missing SvelteKit nodes.last.data array")?;

        let mut out = Vec::new();
        for item in data_arr {
            let Some(obj) = item.as_object() else { continue };
            if let Some(shares_raw) = obj.get("sharesOut").and_then(|v| resolve_devalue_str(data_arr, v)) {
                if let Some(shares) = parse_human_number(&shares_raw) {
                    out.push(make_obs(ObsParams {
                        obs_ts: now,
                        known_at: now,
                        revision_seq: 0,
                        series_id: &format!("etf:{}:stockanalysis_shares_outstanding", sym),
                        domain: "etf_snapshot",
                        compartment,
                        entity_id: &sym,
                        metric_name: "shares_outstanding",
                        frequency: ObservationFrequency::Daily,
                        value: shares,
                        raw_value: Some(shares),
                        unit: "shares",
                        source: "stockanalysis",
                        source_series_id: &format!("{}:sharesOut", sym),
                        source_url: &url,
                        source_tier: SourceTier::UnofficialJson,
                        source_checksum: Some(&checksum),
                        batch_id,
                        extra_json: Some(serde_json::json!({"raw_text": shares_raw, "snapshot": true}).to_string()),
                    }));
                }
            }
        }
        if out.is_empty() {
            return Err(format!("Fail-fast: StockAnalysis JSON contained 0 sharesOut fields at {}", url).into());
        }
        Ok(out)
    }
}

fn resolve_devalue_str(arr: &[Value], ptr: &Value) -> Option<String> {
    if let Some(s) = ptr.as_str() {
        return Some(s.to_string());
    }
    let idx = ptr.as_u64()? as usize;
    arr.get(idx)?.as_str().map(str::to_string)
}

pub fn parse_human_number(raw: &str) -> Option<f64> {
    let cleaned = raw.trim().replace([',', '$', ' '], "");
    if cleaned.is_empty() || cleaned == "-" || cleaned.eq_ignore_ascii_case("n/a") {
        return None;
    }
    let (num_part, mult) = match cleaned.chars().last()? {
        'T' | 't' => (&cleaned[..cleaned.len() - 1], 1e12),
        'B' | 'b' => (&cleaned[..cleaned.len() - 1], 1e9),
        'M' | 'm' => (&cleaned[..cleaned.len() - 1], 1e6),
        'K' | 'k' => (&cleaned[..cleaned.len() - 1], 1e3),
        _ => (cleaned.as_str(), 1.0),
    };
    num_part.parse::<f64>().ok().map(|n| n * mult)
}
