//! Universal Bitemporal Observation & Mandatory Provenance Domain Models (Phase 306).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Irregular,
}

impl ObservationFrequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "1d",
            Self::Weekly => "1w",
            Self::Monthly => "1M",
            Self::Quarterly => "1Q",
            Self::Irregular => "irregular",
        }
    }

    pub fn from_str_lenient(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "1d" | "d" | "daily" => Self::Daily,
            "1w" | "w" | "weekly" => Self::Weekly,
            "1m" | "m" | "monthly" => Self::Monthly,
            "1q" | "q" | "quarterly" => Self::Quarterly,
            _ => Self::Irregular,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTier {
    OfficialApi,
    OfficialFile,
    CommunityApi,
    UnofficialJson,
    GithubMirror,
    Derived,
}

impl SourceTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OfficialApi => "official_api",
            Self::OfficialFile => "official_file",
            Self::CommunityApi => "community_api",
            Self::UnofficialJson => "unofficial_json",
            Self::GithubMirror => "github_mirror",
            Self::Derived => "derived",
        }
    }

    pub fn from_str_lenient(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "official_api" => Self::OfficialApi,
            "official_file" => Self::OfficialFile,
            "community_api" => Self::CommunityApi,
            "unofficial_json" => Self::UnofficialJson,
            "github_mirror" => Self::GithubMirror,
            "derived" => Self::Derived,
            _ => Self::UnofficialJson,
        }
    }
}

/// Mandatory data-origin attribution attached to every observation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub source: String,
    pub source_series_id: String,
    pub source_url: String,
    pub source_tier: SourceTier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_checksum: Option<String>,
    pub batch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_json: Option<String>,
}

/// Canonical bitemporal metric observation persisted in narrow-long Parquet format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricObservation {
    pub observation_ts_utc: DateTime<Utc>,
    pub known_at_utc: DateTime<Utc>,
    pub revision_seq: u32,
    pub series_id: String,
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compartment: Option<String>,
    pub entity_id: String,
    pub metric_name: String,
    pub frequency: ObservationFrequency,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<f64>,
    pub unit: String,
    pub provenance: SourceProvenance,
}

impl MetricObservation {
    /// Deterministic natural key combining series, valid time, revision, and origin source.
    pub fn natural_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.series_id,
            self.observation_ts_utc.timestamp_nanos_opt().unwrap_or(0),
            self.revision_seq,
            self.provenance.source
        )
    }

    /// Enforces Zero Anonymous Data & numeric finite invariants prior to storage persistence.
    pub fn validate(&self) -> Result<(), String> {
        let non_empty = [
            ("series_id", self.series_id.trim()),
            ("domain", self.domain.trim()),
            ("entity_id", self.entity_id.trim()),
            ("metric_name", self.metric_name.trim()),
            ("unit", self.unit.trim()),
            ("provenance.source", self.provenance.source.trim()),
            (
                "provenance.source_series_id",
                self.provenance.source_series_id.trim(),
            ),
            ("provenance.source_url", self.provenance.source_url.trim()),
            ("provenance.batch_id", self.provenance.batch_id.trim()),
        ];
        for (field, val) in non_empty {
            if val.is_empty() {
                return Err(format!("Zero-anonymous-data violation: '{}' cannot be empty", field));
            }
        }
        if !self.value.is_finite() {
            return Err(format!(
                "Non-finite observation value for series '{}': {}",
                self.series_id, self.value
            ));
        }
        if let Some(raw) = self.raw_value {
            if !raw.is_finite() {
                return Err(format!(
                    "Non-finite raw_value for series '{}': {}",
                    self.series_id, raw
                ));
            }
        }
        Ok(())
    }
}

/// Computes lowercase hex SHA-256 checksum for raw upstream payloads.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{:02x}", b);
    }
    out
}

/// Validates and normalizes safe path segments for Hive partition keys.
pub fn sanitize_partition_component(raw: &str) -> String {
    raw.trim()
        .replace(['/', '\\', ':', ' ', '.'], "_")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}
