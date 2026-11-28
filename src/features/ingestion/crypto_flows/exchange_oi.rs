//! Direct Keyless Exchange Derivatives OI Collectors: Binance Futures, OKX Rubik, Bybit V5, Deribit (Phase 320).

use chrono::{TimeZone, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::core::observation_types::{MetricObservation, ObservationFrequency, SourceTier};
use crate::features::ingestion::liquidity_common::{
    build_browser_client, fetch_bytes_with_checksum, make_obs, ObsParams,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceOiRow {
    symbol: String,
    sum_open_interest_value: String,
    timestamp: i64,
}

#[derive(Debug, Deserialize)]
struct OkxRubikResp {
    data: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct BybitOiResp {
    result: BybitOiResult,
}

#[derive(Debug, Deserialize)]
struct BybitOiResult {
    list: Vec<BybitOiItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BybitOiItem {
    open_interest: String,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct BybitFundingResp {
    result: BybitFundingResult,
}

#[derive(Debug, Deserialize)]
struct BybitFundingResult {
    list: Vec<BybitFundingItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BybitFundingItem {
    symbol: String,
    funding_rate: String,
    funding_rate_timestamp: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceFundingRow {
    symbol: String,
    funding_rate: String,
    funding_time: i64,
}

#[derive(Debug, Deserialize)]
struct DeribitSummaryResp {
    result: Vec<DeribitInstrument>,
}

#[derive(Debug, Deserialize)]
struct DeribitInstrument {
    open_interest: Option<f64>,
    mark_price: Option<f64>,
}

pub struct ExchangeOiFetcher {
    client: Client,
}

impl Default for ExchangeOiFetcher {
    fn default() -> Self {
        Self {
            client: build_browser_client(),
        }
    }
}

impl ExchangeOiFetcher {
    pub async fn fetch_binance_btc_oi(
        &self,
        limit: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://fapi.binance.com/futures/data/openInterestHist?symbol=BTCUSDT&period=1d&limit={}",
            limit.clamp(1, 500)
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<BinanceOiRow> = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let (Some(ts), Ok(val)) = (
                Utc.timestamp_millis_opt(r.timestamp).single(),
                r.sum_open_interest_value.parse::<f64>(),
            ) else {
                continue;
            };
            out.push(make_obs(ObsParams {
                obs_ts: ts,
                known_at: now,
                revision_seq: 0,
                series_id: "deriv:BINANCE_BTC:open_interest_usd",
                domain: "derivatives_oi",
                compartment: Some("deleveraging"),
                entity_id: "BINANCE_BTCUSDT",
                metric_name: "open_interest_usd",
                frequency: ObservationFrequency::Daily,
                value: val,
                raw_value: Some(val),
                unit: "usd",
                source: "binance_futures",
                source_series_id: &r.symbol,
                source_url: &url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        Ok(out)
    }

    pub async fn fetch_okx_btc_oi(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://www.okx.com/api/v5/rubik/stat/contracts/open-interest-history?instId=BTC-USDT-SWAP&period=1D";
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, url).await?;
        let parsed: OkxRubikResp = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(parsed.data.len());
        for row in parsed.data {
            let Some(ms) = row.first().and_then(|s| s.parse::<i64>().ok()) else { continue };
            let Some(usd) = row.get(3).and_then(|s| s.parse::<f64>().ok()) else { continue };
            let Some(ts) = Utc.timestamp_millis_opt(ms).single() else { continue };
            out.push(make_obs(ObsParams {
                obs_ts: ts,
                known_at: now,
                revision_seq: 0,
                series_id: "deriv:OKX_BTC:open_interest_usd",
                domain: "derivatives_oi",
                compartment: Some("deleveraging"),
                entity_id: "OKX_BTC_SWAP",
                metric_name: "open_interest_usd",
                frequency: ObservationFrequency::Daily,
                value: usd,
                raw_value: Some(usd),
                unit: "usd",
                source: "okx_rubik",
                source_series_id: "BTC-USDT-SWAP",
                source_url: url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        Ok(out)
    }

    pub async fn fetch_bybit_btc_oi(
        &self,
        limit: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://api.bybit.com/v5/market/open-interest?category=linear&symbol=BTCUSDT&intervalTime=1d&limit={}",
            limit.clamp(1, 200)
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let parsed: BybitOiResp = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(parsed.result.list.len());
        for item in parsed.result.list {
            let (Ok(ms), Ok(oi_contracts)) = (
                item.timestamp.parse::<i64>(),
                item.open_interest.parse::<f64>(),
            ) else {
                continue;
            };
            let Some(ts) = Utc.timestamp_millis_opt(ms).single() else { continue };
            out.push(make_obs(ObsParams {
                obs_ts: ts,
                known_at: now,
                revision_seq: 0,
                series_id: "deriv:BYBIT_BTC:open_interest_contracts",
                domain: "derivatives_oi",
                compartment: Some("deleveraging"),
                entity_id: "BYBIT_BTCUSDT",
                metric_name: "open_interest_contracts",
                frequency: ObservationFrequency::Daily,
                value: oi_contracts,
                raw_value: Some(oi_contracts),
                unit: "contracts",
                source: "bybit_v5",
                source_series_id: "BTCUSDT",
                source_url: &url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: Some("{\"methodology\":\"unilateral_post_transition\"}".to_string()),
            }));
        }
        Ok(out)
    }

    pub async fn fetch_deribit_btc_oi_snapshot(
        &self,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://www.deribit.com/api/v2/public/get_book_summary_by_currency?currency=BTC&kind=future";
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, url).await?;
        let parsed: DeribitSummaryResp = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut total_usd = 0.0;
        for ins in &parsed.result {
            if let Some(oi) = ins.open_interest {
                // Deribit futures open_interest is in USD for inverse perpetual/futures (or quote)
                let _ = ins.mark_price;
                total_usd += oi;
            }
        }
        if total_usd <= 0.0 {
            return Ok(Vec::new());
        }
        Ok(vec![make_obs(ObsParams {
            obs_ts: now,
            known_at: now,
            revision_seq: 0,
            series_id: "deriv:DERIBIT_BTC:open_interest_usd",
            domain: "derivatives_oi",
            compartment: Some("deleveraging"),
            entity_id: "DERIBIT_BTC",
            metric_name: "open_interest_usd",
            frequency: ObservationFrequency::Daily,
            value: total_usd,
            raw_value: Some(total_usd),
            unit: "usd",
            source: "deribit",
            source_series_id: "BTC:future",
            source_url: url,
            source_tier: SourceTier::OfficialApi,
            source_checksum: Some(&checksum),
            batch_id,
            extra_json: None,
        })])
    }

    /// Fetches keyless Bybit V5 8-hour BTCUSDT perpetual funding rate history (raw as-is).
    pub async fn fetch_bybit_btc_funding_rate(
        &self,
        limit: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://api.bybit.com/v5/market/funding/history?category=linear&symbol=BTCUSDT&limit={}",
            limit.clamp(1, 200)
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let parsed: BybitFundingResp = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(parsed.result.list.len());
        for item in parsed.result.list {
            let (Ok(ms), Ok(rate)) = (
                item.funding_rate_timestamp.parse::<i64>(),
                item.funding_rate.parse::<f64>(),
            ) else {
                continue;
            };
            let Some(ts) = Utc.timestamp_millis_opt(ms).single() else { continue };
            out.push(make_obs(ObsParams {
                obs_ts: ts,
                known_at: now,
                revision_seq: 0,
                series_id: "deriv:BYBIT_BTC:funding_rate_8h",
                domain: "derivatives_funding",
                compartment: Some("deleveraging"),
                entity_id: "BYBIT_BTCUSDT",
                metric_name: "funding_rate_8h",
                frequency: ObservationFrequency::Irregular,
                value: rate,
                raw_value: Some(rate),
                unit: "rate_8h",
                source: "bybit_v5",
                source_series_id: &item.symbol,
                source_url: &url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        Ok(out)
    }

    /// Fetches keyless Binance Futures 8-hour BTCUSDT perpetual funding rate history (raw as-is).
    pub async fn fetch_binance_btc_funding_rate(
        &self,
        limit: usize,
        batch_id: &str,
    ) -> Result<Vec<MetricObservation>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://fapi.binance.com/fapi/v1/fundingRate?symbol=BTCUSDT&limit={}",
            limit.clamp(1, 1000)
        );
        let (bytes, checksum) = fetch_bytes_with_checksum(&self.client, &url).await?;
        let rows: Vec<BinanceFundingRow> = serde_json::from_slice(&bytes)?;
        let now = Utc::now();
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let (Some(ts), Ok(rate)) = (
                Utc.timestamp_millis_opt(r.funding_time).single(),
                r.funding_rate.parse::<f64>(),
            ) else {
                continue;
            };
            out.push(make_obs(ObsParams {
                obs_ts: ts,
                known_at: now,
                revision_seq: 0,
                series_id: "deriv:BINANCE_BTC:funding_rate_8h",
                domain: "derivatives_funding",
                compartment: Some("deleveraging"),
                entity_id: "BINANCE_BTCUSDT",
                metric_name: "funding_rate_8h",
                frequency: ObservationFrequency::Irregular,
                value: rate,
                raw_value: Some(rate),
                unit: "rate_8h",
                source: "binance_futures",
                source_series_id: &r.symbol,
                source_url: &url,
                source_tier: SourceTier::OfficialApi,
                source_checksum: Some(&checksum),
                batch_id,
                extra_json: None,
            }));
        }
        Ok(out)
    }
}
