//! Macroeconomic calendar fetcher & parser for real-time and scheduled economic releases.
//! Interacts with high-availability keyless calendar providers (e.g. Biquote API) and SDMX standards.

use std::time::Duration;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::core::types::EconomicEventRecord;

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
    /// Supports filtering by countries ("US,GB,EU") and importance ("high").
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
}
