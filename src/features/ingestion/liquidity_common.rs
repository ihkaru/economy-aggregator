//! Shared zero-key HTTP, SHA-256 provenance, and date utilities for liquidity & flow connectors.

use std::time::Duration;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;

use crate::core::observation_types::{
    sha256_hex, MetricObservation, ObservationFrequency, SourceProvenance, SourceTier,
};

pub fn build_browser_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .build()
        .expect("Failed to construct HTTP client")
}

/// Honest non-browser API client (avoids Chrome UA / rustls TLS fingerprint mismatch on strict CDNs).
pub fn build_api_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("economy-aggregator/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to construct API HTTP client")
}

pub async fn fetch_bytes_with_checksum(
    client: &Client,
    url: &str,
) -> Result<(Vec<u8>, String), Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} for {}", resp.status(), url).into());
    }
    let bytes = resp.bytes().await?.to_vec();
    let checksum = sha256_hex(&bytes);
    Ok((bytes, checksum))
}

pub fn parse_date_eod_utc(raw: &str) -> Option<DateTime<Utc>> {
    let clean = raw.trim().get(..10)?;
    let d = NaiveDate::parse_from_str(clean, "%Y-%m-%d").ok()?;
    Some(DateTime::from_naive_utc_and_offset(
        d.and_hms_opt(23, 59, 59)?,
        Utc,
    ))
}

pub fn parse_us_slash_date_eod_utc(raw: &str) -> Option<DateTime<Utc>> {
    let d = NaiveDate::parse_from_str(raw.trim(), "%m/%d/%Y").ok()?;
    Some(DateTime::from_naive_utc_and_offset(
        d.and_hms_opt(23, 59, 59)?,
        Utc,
    ))
}

pub struct ObsParams<'a> {
    pub obs_ts: DateTime<Utc>,
    pub known_at: DateTime<Utc>,
    pub revision_seq: u32,
    pub series_id: &'a str,
    pub domain: &'a str,
    pub compartment: Option<&'a str>,
    pub entity_id: &'a str,
    pub metric_name: &'a str,
    pub frequency: ObservationFrequency,
    pub value: f64,
    pub raw_value: Option<f64>,
    pub unit: &'a str,
    pub source: &'a str,
    pub source_series_id: &'a str,
    pub source_url: &'a str,
    pub source_tier: SourceTier,
    pub source_checksum: Option<&'a str>,
    pub batch_id: &'a str,
    pub extra_json: Option<String>,
}

pub fn make_obs(p: ObsParams<'_>) -> MetricObservation {
    MetricObservation {
        observation_ts_utc: p.obs_ts,
        known_at_utc: p.known_at,
        revision_seq: p.revision_seq,
        series_id: p.series_id.to_string(),
        domain: p.domain.to_string(),
        compartment: p.compartment.map(str::to_string),
        entity_id: p.entity_id.to_string(),
        metric_name: p.metric_name.to_string(),
        frequency: p.frequency,
        value: p.value,
        raw_value: p.raw_value,
        unit: p.unit.to_string(),
        provenance: SourceProvenance {
            source: p.source.to_string(),
            source_series_id: p.source_series_id.to_string(),
            source_url: p.source_url.to_string(),
            source_tier: p.source_tier,
            source_checksum: p.source_checksum.map(str::to_string),
            batch_id: p.batch_id.to_string(),
            extra_json: p.extra_json,
        },
    }
}

pub fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&")
}
