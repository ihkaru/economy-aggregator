//! OCC Daily US Equity/Index/Futures Options Open Interest CSV Connector (Phase 316 + 327).
//! Uses csv::Reader to parse quoted comma-separated thousands cleanly.

use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use reqwest::Client;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_us_slash_date_eod_utc,
    ObsParams,
};

pub struct OccOiFetcher {
    client: Client,
    base_url: String,
}

impl Default for OccOiFetcher {
    fn default() -> Self {
        Self::new("https://marketdata.theocc.com".to_string())
    }
}

impl OccOiFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_month_for_date(
        &self,
        month_day_year: &str,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/daily-open-interest?reportDate={}&action=download&format=csv",
            self.base_url, month_day_year
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(bytes.as_slice());
        let now = Utc::now();
        let mut out = Vec::new();

        for rec in rdr.records().flatten() {
            let Some(date_raw) = rec.get(0) else { continue };
            let Some(obs_ts) = parse_us_slash_date_eod_utc(date_raw) else { continue };
            let Some(occ_total) = rec.get(11).and_then(parse_comma_number) else { continue };
            let eq_total = rec.get(3).and_then(parse_comma_number);

            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: "deriv:EQ_OPT:open_interest_contracts",
                domain: "derivatives_oi",
                compartment: Some("deleveraging"),
                entity_id: "OCC_TOTAL",
                metric_name: "open_interest_contracts",
                frequency: ObservationFrequency::Daily,
                value: occ_total,
                raw_value: Some(occ_total),
                unit: "contracts",
                source: "occ_csv",
                source_series_id: "daily-open-interest:OCC_Total",
                source_url: &url,
                source_tier: SourceTier::OfficialFile,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: eq_total.map(|e| serde_json::json!({"equity_total": e}).to_string()),
            }));
        }

        if out.is_empty() {
            return Err(format!("Fail-fast: parsed 0 valid OCC rows from {}", url).into());
        }
        Ok(out)
    }

    pub async fn fetch_recent_months(
        &self,
        months: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let mut d = Utc::now().date_naive();
        let mut all = Vec::new();
        for _ in 0..months.clamp(1, 24) {
            while d.weekday() == Weekday::Sat || d.weekday() == Weekday::Sun {
                d -= Duration::days(1);
            }
            let s = format!("{:02}/{:02}/{:04}", d.month(), d.day(), d.year());
            if let Ok(rows) = self.fetch_month_for_date(&s, batch_id).await {
                all.extend(rows);
            }
            d = step_prev_month_anchor(d);
        }
        if all.is_empty() {
            return Err("Fail-fast: OCC 24-month sync yielded 0 observations".into());
        }
        Ok(all)
    }
}

fn parse_comma_number(raw: &str) -> Option<f64> {
    let cleaned = raw.trim().replace([',', '"'], "");
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

fn step_prev_month_anchor(d: NaiveDate) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d);
    first - Duration::days(1)
}
