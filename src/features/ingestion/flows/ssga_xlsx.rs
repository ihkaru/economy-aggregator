//! SSGA Official XLSX Connector (`SPY` & `GLD`) via pure-Rust `calamine` (Phase 313).

use std::io::Cursor;
use calamine::{Data, Reader, Xlsx};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use reqwest::Client;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc, ObsParams,
};

pub struct SsgaXlsxFetcher {
    client: Client,
}

impl Default for SsgaXlsxFetcher {
    fn default() -> Self {
        Self {
            client: build_browser_client(),
        }
    }
}

impl SsgaXlsxFetcher {
    pub async fn fetch_spy_history(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://www.ssga.com/library-content/products/fund-data/etfs/us/navhist-us-en-spy.xlsx";
        self.fetch_ssga_workbook(url, "SPY", "equities", batch_id).await
    }

    pub async fn fetch_gld_history(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://www.ssga.com/library-content/products/fund-data/etfs/us/navhist-us-en-gld.xlsx";
        self.fetch_ssga_workbook(url, "GLD", "gold", batch_id).await
    }

    pub async fn fetch_ssga_workbook(
        &self,
        url: &str,
        ticker: &str,
        compartment: &str,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, url).await?;
        let mut wb: Xlsx<Cursor<Vec<u8>>> = Xlsx::new(Cursor::new(bytes))?;
        let sheet_name = wb
            .sheet_names()
            .first()
            .cloned()
            .ok_or("SSGA XLSX workbook has no worksheets")?;
        let range = wb.worksheet_range(&sheet_name)?;
        let now = Utc::now();
        let sym = ticker.to_ascii_uppercase();
        let mut out = Vec::with_capacity(range.height() * 3);

        for row in range.rows() {
            let Some(obs_ts) = row.first().and_then(parse_excel_date) else {
                continue;
            };
            let nav = row.get(1).and_then(parse_cell_f64);
            let shares = row.get(2).and_then(parse_cell_f64);
            let aum = row.get(3).and_then(parse_cell_f64);

            if let Some(v) = aum {
                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at: now,
                    revision_seq: 0,
                    series_id: &format!("etf:{}:aum_usd", sym),
                    domain: "etf_flow",
                    compartment: Some(compartment),
                    entity_id: &sym,
                    metric_name: "aum_usd",
                    frequency: ObservationFrequency::Daily,
                    value: v,
                    raw_value: Some(v),
                    unit: "usd",
                    source: "ssga_xlsx",
                    source_series_id: &format!("navhist-{}:TotalNetAssets", sym),
                    source_url: url,
                    source_tier: SourceTier::OfficialFile,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: nav.map(|n| serde_json::json!({"nav_usd": n}).to_string()),
                }));
            }
            if let Some(sh) = shares {
                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at: now,
                    revision_seq: 0,
                    series_id: &format!("etf:{}:shares_outstanding", sym),
                    domain: "etf_flow",
                    compartment: Some(compartment),
                    entity_id: &sym,
                    metric_name: "shares_outstanding",
                    frequency: ObservationFrequency::Daily,
                    value: sh,
                    raw_value: Some(sh),
                    unit: "shares",
                    source: "ssga_xlsx",
                    source_series_id: &format!("navhist-{}:SharesOutstanding", sym),
                    source_url: url,
                    source_tier: SourceTier::OfficialFile,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: None,
                }));
            }
        }
        if out.is_empty() {
            return Err(format!("Fail-fast: parsed 0 SSGA rows from {} (workbook schema changed)", url).into());
        }
        Ok(out)
    }
}

fn parse_excel_date(cell: &Data) -> Option<DateTime<Utc>> {
    match cell {
        Data::String(s) => NaiveDate::parse_from_str(s.trim(), "%d-%b-%Y")
            .ok()
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
            .or_else(|| parse_date_eod_utc(s)),
        Data::DateTime(excel_dt) => excel_dt
            .as_datetime()
            .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc)),
        Data::Float(f) if *f > 20_000.0 => {
            let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?;
            let d = epoch + Duration::days(*f as i64);
            Some(DateTime::from_naive_utc_and_offset(d.and_hms_opt(23, 59, 59)?, Utc))
        }
        _ => None,
    }
}

pub fn parse_cell_f64(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::String(s) => s.trim().replace(',', "").parse::<f64>().ok(),
        _ => None,
    }
}
