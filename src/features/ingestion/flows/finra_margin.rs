//! FINRA Margin Statistics Dynamic XLSX Connector (Monthly Customer Margin Debit Balances) (Phase 316).

use std::io::Cursor;
use calamine::{Data, Reader, Xlsx};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use reqwest::Client;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::flows::ssga_xlsx::parse_cell_f64;
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, ObsParams,
};

pub struct FinraMarginFetcher {
    client: Client,
    base_url: String,
}

impl Default for FinraMarginFetcher {
    fn default() -> Self {
        Self::new("https://www.finra.org".to_string())
    }
}

impl FinraMarginFetcher {
    pub fn new(base_url: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_margin_debt(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let index_url = format!("{}/rules-guidance/key-topics/margin-accounts/margin-statistics", self.base_url);
        let (page_bytes, _) = fetch_bytes_with_checksum(&self.client, &index_url).await?;
        let html = String::from_utf8_lossy(&page_bytes);
        let xlsx_url = extract_finra_xlsx_url(&self.base_url, &html)
            .ok_or_else(|| format!("Fail-fast: FINRA page {} did not expose an .xlsx link (schema/layout changed)", index_url))?;

        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &xlsx_url).await?;
        let mut wb: Xlsx<Cursor<Vec<u8>>> = Xlsx::new(Cursor::new(bytes))?;
        let sheet = wb
            .sheet_names()
            .first()
            .cloned()
            .ok_or("FINRA XLSX workbook has no worksheets")?;
        let range = wb.worksheet_range(&sheet)?;
        let mut out = Vec::with_capacity(range.height());

        for row in range.rows() {
            let Some(obs_ts) = row.first().and_then(parse_year_month_eom) else {
                continue;
            };
            let Some(debit_millions) = row.get(1).and_then(parse_cell_f64) else {
                continue;
            };
            let free_cash = row.get(2).and_then(parse_cell_f64);
            let free_margin = row.get(3).and_then(parse_cell_f64);
            // FINRA publishes monthly figures roughly 3 weeks after month-end.
            let known_at = obs_ts + Duration::days(21);
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at,
                revision_seq: 0,
                series_id: "credit:US:FINRA_MARGIN_DEBT_USD",
                domain: "margin_debt",
                compartment: Some("deleveraging"),
                entity_id: "US",
                metric_name: "margin_debit_usd",
                frequency: ObservationFrequency::Monthly,
                value: debit_millions * 1_000_000.0,
                raw_value: Some(debit_millions),
                unit: "usd",
                source: "finra_xlsx",
                source_series_id: "margin-statistics:DebitBalances",
                source_url: &xlsx_url,
                source_tier: SourceTier::OfficialFile,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: Some(
                    serde_json::json!({
                        "free_credit_cash_m": free_cash,
                        "free_credit_margin_m": free_margin,
                    })
                    .to_string(),
                ),
            }));
        }
        if out.is_empty() {
            return Err(format!("Fail-fast: parsed 0 FINRA rows from {}", xlsx_url).into());
        }
        Ok(out)
    }
}

fn extract_finra_xlsx_url(base_url: &str, html: &str) -> Option<String> {
    let idx = html.find(".xlsx")?;
    let start = html[..idx].rfind(['"', '\''])?;
    let rel = &html[start + 1..idx + 5];
    if rel.starts_with("http://") || rel.starts_with("https://") {
        Some(rel.to_string())
    } else if rel.starts_with('/') {
        Some(format!("{}{}", base_url, rel))
    } else {
        Some(format!("{}/{}", base_url, rel))
    }
}

fn parse_year_month_eom(cell: &Data) -> Option<DateTime<Utc>> {
    let s = match cell {
        Data::String(v) => v.trim(),
        _ => return None,
    };
    let mut parts = s.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    let eom = next_month_first - Duration::days(1);
    Some(DateTime::from_naive_utc_and_offset(eom.and_hms_opt(23, 59, 59)?, Utc))
}
