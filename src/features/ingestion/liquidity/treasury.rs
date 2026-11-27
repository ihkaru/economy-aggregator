//! US Treasury Keyless Connectors: TGA Daily Balance & Daily 13-Week T-Bill Yield (Phase 311).

use chrono::{Datelike, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc,
    parse_us_slash_date_eod_utc, ObsParams,
};

#[derive(Debug, Deserialize)]
struct FiscalDataResp {
    data: Vec<FiscalDataRow>,
    #[serde(default)]
    meta: Option<FiscalDataMeta>,
}

#[derive(Debug, Deserialize)]
struct FiscalDataMeta {
    #[serde(rename = "total-pages", default)]
    total_pages: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FiscalDataRow {
    record_date: String,
    #[serde(default)]
    account_type: Option<String>,
    #[serde(default)]
    close_today_bal: Option<String>,
    #[serde(default)]
    open_today_bal: Option<String>,
}

pub struct TreasuryFetcher {
    client: Client,
    fiscal_base: String,
    rates_base: String,
}

impl Default for TreasuryFetcher {
    fn default() -> Self {
        Self::new(
            "https://api.fiscaldata.treasury.gov".to_string(),
            "https://home.treasury.gov".to_string(),
        )
    }
}

impl TreasuryFetcher {
    pub fn new(fiscal_base: String, rates_base: String) -> Self {
        Self {
            client: build_browser_client(),
            fiscal_base: fiscal_base.trim_end_matches('/').to_string(),
            rates_base: rates_base.trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_tga_balance(
        &self,
        page_size: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let clamped_size = page_size.clamp(1, 10_000);
        let now = Utc::now();
        let mut out = Vec::new();
        let mut page = 1usize;

        loop {
            let url = format!(
                "{}/services/api/fiscal_service/v1/accounting/dts/operating_cash_balance?filter=record_date:gte:2016-01-01&sort=-record_date&page[number]={}&page[size]={}",
                self.fiscal_base, page, clamped_size
            );
            let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
            let parsed: FiscalDataResp = serde_json::from_slice(&bytes)?;
            let total_pages = parsed.meta.as_ref().and_then(|m| m.total_pages).unwrap_or(1);
            if parsed.data.is_empty() {
                break;
            }
            for row in parsed.data {
                let acct = row.account_type.as_deref().unwrap_or("");
                if !acct.contains("Treasury General Account") && !acct.contains("Closing Balance") {
                    continue;
                }
                let raw_num = row
                    .close_today_bal
                    .as_deref()
                    .filter(|s| *s != "null" && !s.trim().is_empty())
                    .or(row.open_today_bal.as_deref())
                    .and_then(|s| s.parse::<f64>().ok());
                let (Some(obs_ts), Some(raw_m)) = (parse_date_eod_utc(&row.record_date), raw_num) else {
                    continue;
                };
                // Fiscal Data reports balances in $ Millions -> scale to full USD.
                out.push(make_obs(ObsParams {
                    obs_ts,
                    known_at: now,
                    revision_seq: 0,
                    series_id: "macro:US:TGA_BALANCE_USD",
                    domain: "macro_liquidity",
                    compartment: Some("macro_control"),
                    entity_id: "US",
                    metric_name: "tga_balance_usd",
                    frequency: ObservationFrequency::Daily,
                    value: raw_m * 1_000_000.0,
                    raw_value: Some(raw_m),
                    unit: "usd",
                    source: "treasury_fiscaldata",
                    source_series_id: "dts/operating_cash_balance/TGA",
                    source_url: &url,
                    source_tier: SourceTier::OfficialApi,
                    source_checksum: Some(&checksum),
                    batch_id,
                    extra_json: row.account_type.map(|a| serde_json::json!({"account_type": a}).to_string()),
                }));
            }
            if page >= total_pages || page >= 20 {
                break;
            }
            page += 1;
        }
        if out.is_empty() {
            return Err("Fail-fast: FiscalData returned 0 TGA rows".into());
        }
        Ok(out)
    }

    pub async fn fetch_tbill_3m_year(
        &self,
        year: i32,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let target_year = if year <= 0 { Utc::now().year() } else { year };
        let url = format!(
            "{}/resource-center/data-chart-center/interest-rates/daily-treasury-rates.csv/{}/all?type=daily_treasury_bill_rates&field_tdr_date_value={}&page&_format=csv",
            self.rates_base, target_year, target_year
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let mut rdr = csv::Reader::from_reader(bytes.as_slice());
        let headers = rdr.headers()?.clone();
        let col_idx = headers
            .iter()
            .position(|h| h.to_ascii_uppercase().contains("13 WEEKS BANK DISCOUNT"))
            .or_else(|| headers.iter().position(|h| h.to_ascii_uppercase().contains("13 WEEKS")))
            .ok_or("Missing 13 WEEKS column in Treasury CSV")?;

        let now = Utc::now();
        let mut out = Vec::new();
        for rec in rdr.records().flatten() {
            let Some(date_s) = rec.get(0) else { continue };
            let Some(rate) = rec.get(col_idx).and_then(|v| v.trim().parse::<f64>().ok()) else {
                continue;
            };
            let Some(obs_ts) = parse_us_slash_date_eod_utc(date_s).or_else(|| parse_date_eod_utc(date_s)) else {
                continue;
            };
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at: now,
                revision_seq: 0,
                series_id: "macro:US:TBILL_3M_PCT",
                domain: "macro_liquidity",
                compartment: Some("fiat_park"),
                entity_id: "US",
                metric_name: "tbill_3m_pct",
                frequency: ObservationFrequency::Daily,
                value: rate,
                raw_value: Some(rate),
                unit: "percent",
                source: "treasury_tbill_csv",
                source_series_id: "daily_treasury_bill_rates:13w",
                source_url: &url,
                source_tier: SourceTier::OfficialFile,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        Ok(out)
    }
}
