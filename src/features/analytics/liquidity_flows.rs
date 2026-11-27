//! Stage-1 Closed-System Liquidity Plumbing Analytics (Phase 324):
//! - Implied Net Flow: F_{i,t} = (V_{i,t} - V_{i,t-1}) - V_{i,t-1} * R_{i,t}
//! - Fed Net Liquidity: NL_t = WALCL_t - TGA_t - ON_RRP_t
//! Every derived data point links back to exact upstream source_series_id lineage.

use std::collections::BTreeMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::observation_types::{
    MetricObservation, ObservationFrequency, SourceProvenance, SourceTier,
};
use crate::core::types::AggregatorCandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpliedNetFlowPoint {
    pub timestamp_utc: DateTime<Utc>,
    pub entity_id: String,
    pub compartment: String,
    pub v_prev_usd: f64,
    pub v_curr_usd: f64,
    pub price_return: f64,
    pub delta_v_usd: f64,
    pub implied_net_flow_usd: f64,
    pub aum_source: String,
    pub price_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FedNetLiquidityPoint {
    pub timestamp_utc: DateTime<Utc>,
    pub fed_assets_usd: f64,
    pub tga_balance_usd: f64,
    pub on_rrp_usd: f64,
    pub net_liquidity_usd: f64,
    pub sources: Vec<String>,
}

/// Computes Stage-1 Implied Net Flows F_{i,t} = (V_t - V_{t-1}) - V_{t-1} * R_t
/// matching each AUM observation by date with its corresponding price return R_t.
pub fn compute_implied_net_flows(
    aum_obs: &[MetricObservation],
    price_candles: &[AggregatorCandle],
    entity_id: &str,
    compartment: &str,
) -> Vec<ImpliedNetFlowPoint> {
    if aum_obs.len() < 2 || price_candles.len() < 2 {
        return Vec::new();
    }

    let mut prices_by_day: BTreeMap<chrono::NaiveDate, (f64, &str)> = BTreeMap::new();
    for c in price_candles {
        if c.close > 0.0 && c.close.is_finite() {
            prices_by_day.insert(c.timestamp_utc.date_naive(), (c.close, c.source.as_str()));
        }
    }

    let mut aum_by_day: BTreeMap<chrono::NaiveDate, &MetricObservation> = BTreeMap::new();
    for o in aum_obs {
        if o.value.is_finite() {
            aum_by_day.insert(o.observation_ts_utc.date_naive(), o);
        }
    }

    let aligned: Vec<(&MetricObservation, f64, &str)> = aum_by_day
        .into_iter()
        .filter_map(|(d, obs)| prices_by_day.get(&d).map(|(p, src)| (obs, *p, *src)))
        .collect();

    let mut out = Vec::with_capacity(aligned.len().saturating_sub(1));
    for w in aligned.windows(2) {
        let (prev_obs, prev_px, _) = w[0];
        let (curr_obs, curr_px, px_src) = w[1];
        if prev_px <= 0.0 || !prev_obs.value.is_finite() || !curr_obs.value.is_finite() {
            continue;
        }
        let r_t = (curr_px / prev_px) - 1.0;
        let delta_v = curr_obs.value - prev_obs.value;
        let net_flow = delta_v - (prev_obs.value * r_t);
        out.push(ImpliedNetFlowPoint {
            timestamp_utc: curr_obs.observation_ts_utc,
            entity_id: entity_id.to_string(),
            compartment: compartment.to_string(),
            v_prev_usd: prev_obs.value,
            v_curr_usd: curr_obs.value,
            price_return: r_t,
            delta_v_usd: delta_v,
            implied_net_flow_usd: net_flow,
            aum_source: curr_obs.provenance.source.clone(),
            price_source: px_src.to_string(),
        });
    }
    out
}

/// Computes Fed Net Liquidity = WALCL - TGA - ON_RRP aligned by latest available prior observation.
pub fn compute_fed_net_liquidity(
    fed_assets: &[MetricObservation],
    tga_balances: &[MetricObservation],
    on_rrp: &[MetricObservation],
) -> Vec<FedNetLiquidityPoint> {
    if fed_assets.is_empty() || tga_balances.is_empty() || on_rrp.is_empty() {
        return Vec::new();
    }

    let tga_map: BTreeMap<i64, &MetricObservation> = tga_balances
        .iter()
        .map(|o| (o.observation_ts_utc.timestamp(), o))
        .collect();
    let rrp_map: BTreeMap<i64, &MetricObservation> = on_rrp
        .iter()
        .map(|o| (o.observation_ts_utc.timestamp(), o))
        .collect();

    let mut sorted_fed = fed_assets.to_vec();
    sorted_fed.sort_by_key(|o| o.observation_ts_utc);

    let mut out = Vec::with_capacity(sorted_fed.len());
    for walcl in sorted_fed {
        let ts = walcl.observation_ts_utc.timestamp();
        let Some((_, tga)) = tga_map.range(..=ts).next_back() else { continue };
        let Some((_, rrp)) = rrp_map.range(..=ts).next_back() else { continue };
        let nl = walcl.value - tga.value - rrp.value;
        out.push(FedNetLiquidityPoint {
            timestamp_utc: walcl.observation_ts_utc,
            fed_assets_usd: walcl.value,
            tga_balance_usd: tga.value,
            on_rrp_usd: rrp.value,
            net_liquidity_usd: nl,
            sources: vec![
                walcl.provenance.source.clone(),
                tga.provenance.source.clone(),
                rrp.provenance.source.clone(),
            ],
        });
    }
    out
}

/// Converts computed Stage-1 flows back into provenance-tagged MetricObservations for Parquet storage.
pub fn net_flows_to_observations(points: &[ImpliedNetFlowPoint], batch_prefix: &str) -> Vec<MetricObservation> {
    let now = Utc::now();
    let batch_id = format!("{}_{}", batch_prefix, &Uuid::new_v4().to_string()[..6]);
    points
        .iter()
        .map(|p| MetricObservation {
            observation_ts_utc: p.timestamp_utc,
            known_at_utc: now,
            revision_seq: 0,
            series_id: format!("derived:{}:implied_net_flow_usd", p.entity_id),
            domain: "derived".to_string(),
            compartment: Some(p.compartment.clone()),
            entity_id: p.entity_id.clone(),
            metric_name: "implied_net_flow_usd".to_string(),
            frequency: ObservationFrequency::Daily,
            value: p.implied_net_flow_usd,
            raw_value: Some(p.delta_v_usd),
            unit: "usd".to_string(),
            provenance: SourceProvenance {
                source: "economy_aggregator_stage1".to_string(),
                source_series_id: format!("stage1:{}", p.entity_id),
                source_url: "internal://analytics/liquidity_flows/stage1".to_string(),
                source_tier: SourceTier::Derived,
                source_checksum: None,
                batch_id: batch_id.clone(),
                extra_json: Some(
                    serde_json::json!({
                        "aum_source": p.aum_source,
                        "price_source": p.price_source,
                        "price_return": p.price_return,
                    })
                    .to_string(),
                ),
            },
        })
        .collect()
}
