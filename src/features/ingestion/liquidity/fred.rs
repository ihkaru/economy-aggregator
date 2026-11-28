//! Official St. Louis Fed (FRED REST v1) Connector for Macro Liquidity & Credit Stress (Phase 326 + 337).
//! Activated cleanly when FRED_API_KEY is present; never leaks the API key into persisted provenance.

use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, parse_date_eod_utc, ObsParams,
};

#[derive(Debug, Deserialize)]
struct FredObsResp {
    observations: Vec<FredObsRow>,
}

#[derive(Debug, Deserialize)]
struct FredObsRow {
    date: String,
    #[serde(default)]
    realtime_start: Option<String>,
    value: String,
}

#[derive(Debug, Clone, Copy)]
pub struct FredSeriesSpec {
    pub fred_id: &'static str,
    pub series_id: &'static str,
    pub entity_id: &'static str,
    pub compartment: &'static str,
    pub metric_name: &'static str,
    pub frequency: ObservationFrequency,
    pub scale_factor: f64,
    pub unit: &'static str,
    pub backfill_start: &'static str,
    pub min_history_years: usize,
}

pub const CANONICAL_FRED_SPECS: &[FredSeriesSpec] = &[
    FredSeriesSpec { fred_id: "WALCL", series_id: "macro:US:FRED_WALCL_USD", entity_id: "US", compartment: "macro_control", metric_name: "fed_total_assets_usd", frequency: ObservationFrequency::Weekly, scale_factor: 1_000_000.0, unit: "usd", backfill_start: "2002-12-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "WTREGEN", series_id: "macro:US:FRED_WTREGEN_USD", entity_id: "US", compartment: "macro_control", metric_name: "tga_balance_usd", frequency: ObservationFrequency::Weekly, scale_factor: 1_000_000.0, unit: "usd", backfill_start: "2002-12-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "RRPONTSYD", series_id: "macro:US:FRED_RRPONTSYD_USD", entity_id: "US", compartment: "fiat_park", metric_name: "on_rrp_usd", frequency: ObservationFrequency::Daily, scale_factor: 1_000_000_000.0, unit: "usd", backfill_start: "2016-01-01", min_history_years: 3 },
    FredSeriesSpec { fred_id: "TOTBKCR", series_id: "macro:US:FRED_TOTBKCR_USD", entity_id: "US", compartment: "deleveraging", metric_name: "bank_credit_usd", frequency: ObservationFrequency::Weekly, scale_factor: 1_000_000_000.0, unit: "usd", backfill_start: "1990-01-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "DPSACBW027SBOG", series_id: "macro:US:FRED_BANK_DEPOSITS_USD", entity_id: "US", compartment: "fiat_park", metric_name: "commercial_bank_deposits_usd", frequency: ObservationFrequency::Weekly, scale_factor: 1_000_000_000.0, unit: "usd", backfill_start: "1990-01-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "DTB3", series_id: "macro:US:FRED_DTB3_PCT", entity_id: "US", compartment: "fiat_park", metric_name: "tbill_3m_pct", frequency: ObservationFrequency::Daily, scale_factor: 1.0, unit: "percent", backfill_start: "1990-01-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "T10Y2Y", series_id: "macro:US:FRED_T10Y2Y_PCT", entity_id: "US", compartment: "macro_control", metric_name: "yield_curve_10y2y_pct", frequency: ObservationFrequency::Daily, scale_factor: 1.0, unit: "percent", backfill_start: "1990-01-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "BAMLH0A0HYM2", series_id: "macro:US:FRED_HY_OAS_PCT", entity_id: "US", compartment: "deleveraging", metric_name: "hy_credit_spread_pct", frequency: ObservationFrequency::Daily, scale_factor: 1.0, unit: "percent", backfill_start: "2016-01-01", min_history_years: 3 },
    FredSeriesSpec { fred_id: "WRMFNS", series_id: "macro:US:FRED_WRMFNS_USD", entity_id: "US", compartment: "fiat_park", metric_name: "retail_mmf_usd", frequency: ObservationFrequency::Weekly, scale_factor: 1_000_000_000.0, unit: "usd", backfill_start: "1990-01-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "ECBASSETSW", series_id: "macro:EU:FRED_ECB_ASSETS_EUR", entity_id: "EU", compartment: "macro_control", metric_name: "ecb_total_assets_eur", frequency: ObservationFrequency::Weekly, scale_factor: 1_000_000.0, unit: "eur", backfill_start: "2016-01-01", min_history_years: 3 },
    FredSeriesSpec { fred_id: "JPNASSETS", series_id: "macro:JP:FRED_BOJ_ASSETS_JPY", entity_id: "JP", compartment: "macro_control", metric_name: "boj_total_assets_jpy", frequency: ObservationFrequency::Monthly, scale_factor: 100_000_000.0, unit: "jpy", backfill_start: "2016-01-01", min_history_years: 3 },
    FredSeriesSpec { fred_id: "SOFR", series_id: "macro:US:FRED_SOFR_PCT", entity_id: "US", compartment: "fiat_park", metric_name: "sofr_pct", frequency: ObservationFrequency::Daily, scale_factor: 1.0, unit: "percent", backfill_start: "2016-01-01", min_history_years: 3 },
    FredSeriesSpec { fred_id: "STLFSI4", series_id: "macro:US:FRED_STLFSI4_IDX", entity_id: "US", compartment: "deleveraging", metric_name: "financial_stress_idx", frequency: ObservationFrequency::Weekly, scale_factor: 1.0, unit: "index", backfill_start: "1994-01-01", min_history_years: 15 },
    FredSeriesSpec { fred_id: "WM2NS", series_id: "macro:US:FRED_WM2NS_USD", entity_id: "US", compartment: "macro_control", metric_name: "m2_money_supply_usd", frequency: ObservationFrequency::Weekly, scale_factor: 1_000_000_000.0, unit: "usd", backfill_start: "1990-01-01", min_history_years: 15 },
];

pub struct FredFetcher {
    client: Client,
    base_url: String,
    api_key: String,
}

impl FredFetcher {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            client: build_browser_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.trim().to_string(),
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("FRED_API_KEY").ok()?;
        let trimmed = key.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        let base = std::env::var("FRED_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.stlouisfed.org".to_string());
        Some(Self::new(base, trimmed))
    }

    pub async fn fetch_series(
        &self,
        spec: &FredSeriesSpec,
        observation_start: &str,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        if self.api_key.is_empty() {
            return Err("Fail-fast: FRED_API_KEY is empty".into());
        }
        let request_url = format!(
            "{}/fred/series/observations?series_id={}&api_key={}&file_type=json&observation_start={}",
            self.base_url, spec.fred_id, self.api_key, observation_start
        );
        let safe_provenance_url = format!(
            "{}/fred/series/observations?series_id={}&file_type=json&observation_start={}",
            self.base_url, spec.fred_id, observation_start
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &request_url).await?;
        let parsed: FredObsResp = serde_json::from_slice(&bytes)?;
        let fallback_now = Utc::now();
        let mut out = Vec::with_capacity(parsed.observations.len());

        for row in parsed.observations {
            let trimmed = row.value.trim();
            if trimmed.is_empty() || trimmed == "." {
                continue;
            }
            let Ok(raw_num) = trimmed.parse::<f64>() else { continue };
            let Some(obs_ts) = parse_date_eod_utc(&row.date) else { continue };
            let known_at = row
                .realtime_start
                .as_deref()
                .and_then(parse_date_eod_utc)
                .unwrap_or(fallback_now);
            out.push(make_obs(ObsParams {
                obs_ts,
                known_at,
                revision_seq: 0,
                series_id: spec.series_id,
                domain: "macro_liquidity",
                compartment: Some(spec.compartment),
                entity_id: spec.entity_id,
                metric_name: spec.metric_name,
                frequency: spec.frequency,
                value: raw_num * spec.scale_factor,
                raw_value: Some(raw_num),
                unit: spec.unit,
                source: "fred_api",
                source_series_id: spec.fred_id,
                source_url: &safe_provenance_url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }

        if out.is_empty() {
            return Err(format!("Fail-fast: parsed 0 valid FRED rows for {}", spec.fred_id).into());
        }
        Ok(out)
    }
}
