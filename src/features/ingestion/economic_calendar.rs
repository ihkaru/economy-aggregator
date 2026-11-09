//! Macroeconomic calendar fetcher & parser for real-time and scheduled economic releases.
//! Interacts with high-availability keyless calendar providers and SDMX standards.

use std::fs;
use std::time::Duration;
use chrono::{DateTime, Datelike, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::core::macro_types::{MetricUnitType, RichEconomicEvent, SourceAttribution};
use crate::core::types::EconomicEventRecord;
use crate::features::storage::macro_arrow_schema::macro_events_to_record_batch;
use crate::features::storage::partition::PartitionResolver;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCalendarItem {
    pub id: String,
    pub event_id: String,
    pub time: String,             // ISO-8601 (e.g. "2026-10-29T12:30:00Z")
    pub period: Option<String>,
    pub country_code: String,     // ISO-3166-1 (e.g. "US", "GB")
    pub currency: String,         // ISO-4217 (e.g. "USD", "GBP")
    pub name: String,
    pub importance: String,       // "low", "medium", "high"
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub sector: Option<String>,
    pub unit: Option<String>,
    pub multiplier: Option<String>,
    pub digits: Option<u8>,
    pub actual: Option<f64>,
    pub forecast: Option<f64>,
    pub previous: Option<f64>,
    pub revised_previous: Option<f64>,
    pub source: Option<String>,
    pub source_url: Option<String>,
}

impl RawCalendarItem {
    /// Maps raw external calendar release into standardized EconomicEventRecord domain model.
    pub fn to_domain_record(&self) -> Result<EconomicEventRecord, chrono::ParseError> {
        let scheduled_utc = DateTime::parse_from_rfc3339(&self.time)?
            .with_timezone(&Utc);

        let impact_level = match self.importance.to_lowercase().as_str() {
            "high" => 3,
            "medium" => 2,
            _ => 1,
        };

        let category = self.sector
            .clone()
            .or_else(|| self.event_type.clone())
            .unwrap_or_else(|| "general".to_string());

        Ok(EconomicEventRecord {
            event_id: self.id.clone(),
            scheduled_utc,
            country: self.country_code.to_uppercase(),
            currency: self.currency.to_uppercase(),
            category,
            indicator_code: self.event_id.clone(),
            title: self.name.clone(),
            impact_level,
            source: self.source.clone().unwrap_or_else(|| "calendar".to_string()),
            unit: self.unit.clone().unwrap_or_else(|| "none".to_string()),
            actual: self.actual,
            forecast: self.forecast,
            previous: self.previous,
            revised_previous: self.revised_previous,
            released_at_utc: if self.actual.is_some() { Some(scheduled_utc) } else { None },
            source_url: self.source_url.clone(),
            raw_payload: None,
        })
    }

    /// Maps raw external calendar release into RichEconomicEvent with full provenance and units.
    pub fn to_rich_domain_record(&self) -> Result<RichEconomicEvent, chrono::ParseError> {
        let scheduled_utc = DateTime::parse_from_rfc3339(&self.time)?
            .with_timezone(&Utc);

        let impact_tier = match self.importance.to_lowercase().as_str() {
            "high" => 3,
            "medium" => 2,
            "low" => 1,
            _ => 0,
        };

        let category = self.sector
            .clone()
            .or_else(|| self.event_type.clone())
            .unwrap_or_else(|| "general".to_string());

        let indicator_code = self.name
            .to_lowercase()
            .replace(['/', '-', ' '], "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();

        let (unit_mult, unit_sym) = match self.multiplier.as_deref().unwrap_or("none") {
            "thousands" => (1000.0, "K"),
            "millions" => (1000000.0, "M"),
            "billions" => (1000000000.0, "B"),
            _ => match self.unit.as_deref().unwrap_or("none") {
                "percent" => (0.01, "%"),
                _ => (1.0, ""),
            },
        };

        let raw_actual = self.actual.map(|v| format!("{}{}", v, unit_sym));
        let raw_forecast = self.forecast.map(|v| format!("{}{}", v, unit_sym));
        let raw_previous = self.previous.map(|v| format!("{}{}", v, unit_sym));

        let normalized_actual = self.actual.map(|v| v * if unit_sym == "%" { 0.01 } else { unit_mult });
        let normalized_forecast = self.forecast.map(|v| v * if unit_sym == "%" { 0.01 } else { unit_mult });
        let normalized_previous = self.previous.map(|v| v * if unit_sym == "%" { 0.01 } else { unit_mult });
        let normalized_revised_previous = self.revised_previous.map(|v| v * if unit_sym == "%" { 0.01 } else { unit_mult });

        let unit_type = MetricUnitType::from_str_lenient(self.unit.as_deref().unwrap_or("none"));

        // Determine sentiment polarity: lower is better for unemployment/claims, higher for others
        let better_than_expected = match (self.actual, self.forecast) {
            (Some(act), Some(fcst)) => {
                let name_lower = self.name.to_lowercase();
                let lower_is_better = name_lower.contains("unemployment") || name_lower.contains("jobless");
                if (act - fcst).abs() < 1e-6 {
                    None
                } else if lower_is_better {
                    Some(act < fcst)
                } else {
                    Some(act > fcst)
                }
            }
            _ => None,
        };

        let agency_name = self.source_url.as_deref().map(|url| {
            if url.contains("bls.gov") {
                "U.S. Bureau of Labor Statistics".to_string()
            } else if url.contains("dol.gov") {
                "U.S. Department of Labor".to_string()
            } else if url.contains("treasury.gov") {
                "U.S. Department of the Treasury".to_string()
            } else if url.contains("eia.gov") {
                "U.S. Energy Information Administration".to_string()
            } else if url.contains("nar.realtor") {
                "National Association of Realtors".to_string()
            } else {
                "Official National Agency".to_string()
            }
        });

        let attribution = SourceAttribution {
            provider: self.source.clone().unwrap_or_else(|| "biquote".to_string()),
            original_id: self.id.clone(),
            source_url: self.source_url.clone(),
            agency_name,
            agency_url: self.source_url.clone(),
            observed_at_utc: Utc::now(),
        };

        Ok(RichEconomicEvent {
            event_id: format!("macro:{}:{}:{}", self.country_code.to_uppercase(), indicator_code, scheduled_utc.format("%Y%m%d")),
            scheduled_utc,
            country: self.country_code.to_uppercase(),
            currency: self.currency.to_uppercase(),
            category,
            indicator_code,
            title: self.name.clone(),
            impact_tier,
            is_all_day: false,
            is_tentative: false,
            raw_actual,
            raw_forecast,
            raw_previous,
            normalized_actual,
            normalized_forecast,
            normalized_previous,
            normalized_revised_previous,
            unit_type,
            unit_multiplier: unit_mult,
            unit_symbol: unit_sym.to_string(),
            better_than_expected,
            attribution,
            revision_count: 0,
            revision_history_json: None,
        })
    }
}

pub struct EconomicCalendarFetcher {
    client: Client,
    base_url: String,
}

impl Default for EconomicCalendarFetcher {
    fn default() -> Self {
        Self::new("https://biquote.io".to_string())
    }
}

impl EconomicCalendarFetcher {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, base_url }
    }

    /// Fetches calendar releases from keyless REST provider.
    pub async fn fetch_events(
        &self,
        countries: Option<&str>,
        importance: Option<&str>,
    ) -> Result<Vec<EconomicEventRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/calendar", self.base_url);
        let mut query = Vec::new();

        if let Some(c) = countries {
            query.push(("countries", c));
        }
        if let Some(imp) = importance {
            query.push(("importance", imp));
        }

        let res = self.client.get(&url).query(&query).send().await?;
        if !res.status().is_success() {
            return Err(format!("Calendar API error: HTTP {}", res.status()).into());
        }

        let raw_items: Vec<RawCalendarItem> = res.json().await?;
        let mut records = Vec::with_capacity(raw_items.len());
        for item in raw_items {
            if let Ok(rec) = item.to_domain_record() {
                records.push(rec);
            }
        }

        Ok(records)
    }

    /// Fetches rich standardized macroeconomic events with units, impact, and provenance.
    pub async fn fetch_rich_events(
        &self,
        countries: Option<&str>,
        importance: Option<&str>,
    ) -> Result<Vec<RichEconomicEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/calendar", self.base_url);
        let mut query = Vec::new();

        if let Some(c) = countries {
            query.push(("countries", c));
        }
        if let Some(imp) = importance {
            query.push(("importance", imp));
        }

        let res = self.client.get(&url).query(&query).send().await?;
        if !res.status().is_success() {
            return Err(format!("Calendar API error: HTTP {}", res.status()).into());
        }

        let raw_items: Vec<RawCalendarItem> = res.json().await?;
        let mut records = Vec::with_capacity(raw_items.len());
        for item in raw_items {
            if let Ok(rec) = item.to_rich_domain_record() {
                records.push(rec);
            }
        }

        Ok(records)
    }

    /// Persists rich macroeconomic events into partitioned Parquet lakehouse.
    pub fn persist_to_lake(
        events: &[RichEconomicEvent],
        resolver: &PartitionResolver,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        if events.is_empty() {
            return Ok(0);
        }

        // Group by country, year, month
        let mut grouped: std::collections::HashMap<(String, i32, u32), Vec<&RichEconomicEvent>> =
            std::collections::HashMap::new();

        for event in events {
            let key = (
                event.country.clone(),
                event.scheduled_utc.year(),
                event.scheduled_utc.month(),
            );
            grouped.entry(key).or_default().push(event);
        }

        let mut total_saved = 0;
        for ((country, year, month), slice) in grouped {
            let dir = resolver.resolve_macro_partition(&country, year, month);
            fs::create_dir_all(&dir)?;

            let batch = macro_events_to_record_batch(&slice)?;
            let file_path = dir.join(format!("events_{}.parquet", Utc::now().timestamp_millis()));
            let file = fs::File::create(&file_path)?;

            let props = parquet::file::properties::WriterProperties::builder()
                .set_compression(parquet::basic::Compression::SNAPPY)
                .build();

            let mut writer =
                parquet::arrow::ArrowWriter::try_new(file, batch.schema(), Some(props))?;
            writer.write(&batch)?;
            writer.close()?;

            total_saved += slice.len();
        }

        Ok(total_saved)
    }
}
