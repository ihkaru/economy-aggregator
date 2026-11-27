//! CFTC Commitments of Traders (COT) Socrata Keyless Connector: COMEX Gold & CME S&P 500 / Nasdaq OI (Phase 314).

use chrono::Duration;
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc, ObsParams,
};

#[derive(Debug, Deserialize)]
struct CftcDisaggRow {
    report_date_as_yyyy_mm_dd: String,
    #[serde(default)]
    open_interest_all: Option<String>,
    #[serde(default)]
    m_money_positions_long_all: Option<String>,
    #[serde(default)]
    m_money_positions_short_all: Option<String>,
    #[serde(default)]
    noncomm_positions_long_all: Option<String>,
    #[serde(default)]
    noncomm_positions_short_all: Option<String>,
}

pub struct CftcCotFetcher {
    client: Client,
    base_url: String,
}

impl Default for CftcCotFetcher {
    fn default() -> Self {
        Self::new("https://publicreporting.cftc.gov".to_string())
    }
}

impl CftcCotFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Fetches Disaggregated Gold Futures (`088691`, endpoint `72hh-3qpy.json`):
    /// Managed Money Net Position & Total Open Interest (Friday release lag applied to `known_at_utc`).
    pub async fn fetch_gold_cot(
        &self,
        limit: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/resource/72hh-3qpy.json?cftc_contract_market_code=088691&$order=report_date_as_yyyy_mm_dd%20DESC&$limit={}",
            self.base_url,
            limit.clamp(1, 2000)
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<CftcDisaggRow> = serde_json::from_slice(&bytes)?;
        let mut out = Vec::with_capacity(rows.len() * 2);

        for r in rows {
            let Some(obs_ts) = parse_date_eod_utc(&r.report_date_as_yyyy_mm_dd) else {
                continue;
            };
            // CFTC Tuesday survey is published Friday (~3 days later).
            let known_at = obs_ts + Duration::days(3);
            if let (Some(l), Some(s)) = (
                parse_opt_f64(&r.m_money_positions_long_all),
                parse_opt_f64(&r.m_money_positions_short_all),
            ) {
                let net = l - s;
                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at,
                    revision_seq: 0,
                    series_id: "cot:GOLD:managed_money_net",
                    domain: "positioning_cot",
                    compartment: Some("gold"),
                    entity_id: "COMEX_GOLD",
                    metric_name: "managed_money_net",
                    frequency: ObservationFrequency::Weekly,
                    value: net,
                    raw_value: Some(net),
                    unit: "contracts",
                    source: "cftc_cot",
                    source_series_id: "72hh-3qpy:088691",
                    source_url: &url,
                    source_tier: SourceTier::OfficialApi,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: Some(serde_json::json!({"long": l, "short": s}).to_string()),
                }));
            }
            if let Some(oi) = parse_opt_f64(&r.open_interest_all) {
                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at,
                    revision_seq: 0,
                    series_id: "cot:GOLD:open_interest_contracts",
                    domain: "positioning_cot",
                    compartment: Some("gold"),
                    entity_id: "COMEX_GOLD",
                    metric_name: "open_interest_contracts",
                    frequency: ObservationFrequency::Weekly,
                    value: oi,
                    raw_value: Some(oi),
                    unit: "contracts",
                    source: "cftc_cot",
                    source_series_id: "72hh-3qpy:088691",
                    source_url: &url,
                    source_tier: SourceTier::OfficialApi,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: None,
                }));
            }
        }
        Ok(out)
    }

    /// Fetches Legacy Futures COT Open Interest for CME Equity Index contracts
    /// (e.g., `13874A` = E-Mini S&P 500, `209742` = Nasdaq-100 E-Mini) on `6dca-aqww.json`.
    pub async fn fetch_equity_index_cot(
        &self,
        contract_code: &str,
        series_id: &str,
        entity_id: &str,
        limit: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/resource/6dca-aqww.json?cftc_contract_market_code={}&$order=report_date_as_yyyy_mm_dd%20DESC&$limit={}",
            self.base_url,
            contract_code,
            limit.clamp(1, 2000)
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<CftcDisaggRow> = serde_json::from_slice(&bytes)?;
        let mut out = Vec::with_capacity(rows.len());

        for r in rows {
            let Some(obs_ts) = parse_date_eod_utc(&r.report_date_as_yyyy_mm_dd) else {
                continue;
            };
            let known_at = obs_ts + Duration::days(3);
            if let Some(oi) = parse_opt_f64(&r.open_interest_all) {
                let net = match (
                    parse_opt_f64(&r.noncomm_positions_long_all),
                    parse_opt_f64(&r.noncomm_positions_short_all),
                ) {
                    (Some(l), Some(s)) => Some(l - s),
                    _ => None,
                };
                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at,
                    revision_seq: 0,
                    series_id,
                    domain: "positioning_cot",
                    compartment: Some("deleveraging"),
                    entity_id,
                    metric_name: "open_interest_contracts",
                    frequency: ObservationFrequency::Weekly,
                    value: oi,
                    raw_value: Some(oi),
                    unit: "contracts",
                    source: "cftc_cot",
                    source_series_id: &format!("6dca-aqww:{}", contract_code),
                    source_url: &url,
                    source_tier: SourceTier::OfficialApi,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: net.map(|n| serde_json::json!({"noncomm_net": n}).to_string()),
                }));
            }
        }
        Ok(out)
    }
}

fn parse_opt_f64(opt: &Option<String>) -> Option<f64> {
    opt.as_deref().and_then(|s| s.trim().parse::<f64>().ok())
}
