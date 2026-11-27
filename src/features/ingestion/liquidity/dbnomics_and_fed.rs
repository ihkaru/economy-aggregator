//! DBnomics Keyless Federal Reserve Mirror (H4.1, H8, H15) & Thursday Fed H.4.1 HTML Parser (Phase 312).

use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc, strip_html_tags,
    ObsParams,
};

#[derive(Debug, Deserialize)]
struct DbnomicsResp {
    series: DbnomicsSeriesWrap,
}

#[derive(Debug, Deserialize)]
struct DbnomicsSeriesWrap {
    docs: Vec<DbnomicsDoc>,
}

#[derive(Debug, Deserialize)]
struct DbnomicsDoc {
    series_code: String,
    #[serde(default)]
    period: Vec<String>,
    #[serde(default)]
    value: Vec<serde_json::Value>,
}

pub struct DbnomicsFedFetcher {
    client: Client,
    db_base: String,
    fed_base: String,
}

impl Default for DbnomicsFedFetcher {
    fn default() -> Self {
        Self::new(
            "https://api.db.nomics.world".to_string(),
            "https://www.federalreserve.gov".to_string(),
        )
    }
}

impl DbnomicsFedFetcher {
    pub fn new(db_base: String, fed_base: String) -> Self {
        Self {
            client: build_browser_client(),
            db_base: db_base.trim_end_matches('/').to_string(),
            fed_base: fed_base.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_h8_bank_credit(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_dbnomics_series(
            "FED/H8",
            "B1001NCBA",
            "macro:US:BANK_CREDIT_USD",
            "deleveraging",
            "bank_credit_usd",
            ObservationFrequency::Weekly,
            1_000_000.0,
            "usd",
            batch_id,
        )
        .await
    }

    pub async fn fetch_h41_assets_backfill(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_dbnomics_series(
            "FED/H41",
            "RESH4S_N.WW",
            "macro:US:FED_H41_RESERVE_FACTORS_USD",
            "macro_control",
            "fed_reserve_factors_usd",
            ObservationFrequency::Weekly,
            1_000_000.0,
            "usd",
            batch_id,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_dbnomics_series(
        &self,
        dataset: &str,
        series_code: &str,
        canonical_series_id: &str,
        compartment: &str,
        metric_name: &str,
        freq: ObservationFrequency,
        multiplier: f64,
        unit: &str,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/v22/series/{}/{}?observations=1", self.db_base, dataset, series_code);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let parsed: DbnomicsResp = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::new();

        for doc in parsed.series.docs {
            for (p, v) in doc.period.iter().zip(doc.value.iter()) {
                let (Some(obs_ts), Some(raw_num)) = (parse_date_eod_utc(p), v.as_f64()) else {
                    continue;
                };
                if raw_num < 0.0 && (raw_num + 999999.0).abs() < 1.0 {
                    continue;
                }
                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at: now,
                    revision_seq: 0,
                    series_id: canonical_series_id,
                    domain: "macro_liquidity",
                    compartment: Some(compartment),
                    entity_id: "US",
                    metric_name,
                    frequency: freq,
                    value: raw_num * multiplier,
                    raw_value: Some(raw_num),
                    unit,
                    source: "dbnomics_fed",
                    source_series_id: &format!("{}/{}", dataset, doc.series_code),
                    source_url: &url,
                    source_tier: SourceTier::OfficialApi,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: None,
                }));
            }
        }
        if out.is_empty() {
            return Err(format!("DBnomics returned 0 valid rows for {}/{}", dataset, series_code).into());
        }
        Ok(out)
    }

    /// Parses latest Thursday H.4.1 release HTML to bridge DBnomics 2-3 week lag keylessly.
    pub async fn fetch_latest_fed_h41_html(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/releases/h41/current/h41.htm", self.fed_base);
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let html = String::from_utf8_lossy(&bytes);
        let obs_ts = parse_h41_release_date(&html)
            .ok_or("Could not locate H.4.1 release date in HTML")?;
        let total_assets_m = parse_h41_total_assets_millions(&html)
            .ok_or("Could not parse H.4.1 Total assets figure")?;
        let now = Utc::now();

        Ok(vec![make_obs(ObsParams {
            obs_ts,
            known_at: now,
            revision_seq: 1,
            series_id: "macro:US:FED_TOTAL_ASSETS_USD",
            domain: "macro_liquidity",
            compartment: Some("macro_control"),
            entity_id: "US",
            metric_name: "fed_total_assets_usd",
            frequency: ObservationFrequency::Weekly,
            value: total_assets_m * 1_000_000.0,
            raw_value: Some(total_assets_m),
            unit: "usd",
            source: "fed_h41_html",
            source_series_id: "h41:total_assets",
            source_url: &url,
            source_tier: SourceTier::OfficialFile,
            source_checksum: Some(&checksum),
            batch_id,
            extra_json: None,
        })])
    }
}

fn parse_h41_release_date(html: &str) -> Option<DateTime<Utc>> {
    let head = &html[..html.len().min(6000)];
    let clean = strip_html_tags(head);
    let idx = clean.find("H.4.1")?;
    let after = &clean[idx..clean.len().min(idx + 80)];
    let dash_pos = after.find('-')?;
    let date_slice = after[dash_pos + 1..].trim();
    let candidate = date_slice.split(['|', '\n', '\r']).next()?.trim();
    // Parse "November 19, 2026"
    let d = NaiveDate::parse_from_str(candidate.get(..30).unwrap_or(candidate).trim(), "%B %d, %Y")
        .or_else(|_| {
            let parts: Vec<&str> = candidate.split_whitespace().take(3).collect();
            NaiveDate::parse_from_str(&parts.join(" "), "%B %d, %Y")
        })
        .ok()?;
    Some(DateTime::from_naive_utc_and_offset(d.and_hms_opt(23, 59, 59)?, Utc))
}

fn parse_h41_total_assets_millions(html: &str) -> Option<f64> {
    let idx = html.rfind("Total assets")?;
    let window = &html[idx..html.len().min(idx + 600)];
    let stripped = strip_html_tags(window);
    for token in stripped.split_whitespace() {
        let digits = token.replace([',', '*'], "");
        if let Ok(val) = digits.parse::<f64>() {
            if val > 1_000_000.0 {
                return Some(val);
            }
        }
    }
    None
}
