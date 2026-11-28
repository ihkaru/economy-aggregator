//! Pure Functional Prospective OOS Scorecard Engine (Phase 353).
//! Tracks the frozen pre-registered protocol `docs/validation/prospective_rules_2026_11_28.json`
//! (`v1.1.0`, anchored via GitHub Release & OpenTimestamps Bitcoin proof) with strict anti-peeking
//! and strictly expanding (zero-lookahead) quantiles.

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::core::continuous_types::{
    ProspectiveRuleTracker, ProspectiveScorecardReport, CONTINUOUS_SCHEMA_VERSION,
};
use crate::core::observation_types::sha256_hex;

pub const PROSPECTIVE_PROTOCOL_VERSION: &str = "1.1.0";
pub const PROSPECTIVE_ANCHOR_TAG: &str = "prospective-protocol-v1.1-2026-11-28";
pub const PROSPECTIVE_PROTOCOL_SHA256: &str =
    "07949adaa09fa1c5975a585e2a2495f8841e0bbe3e91a3f69ffe05fbc958a708";
pub const BURN_IN_END_WEEK: &str = "2026-11-27";
pub const OOS_START_WEEK: &str = "2026-12-04";
pub const PRIMARY_READOUT_DATE_UTC: &str = "2028-11-24";
pub const SECONDARY_READOUT_DATE_UTC: &str = "2031-11-21";
pub const PRIMARY_READOUT_WEEKS: usize = 104;
pub const SECONDARY_READOUT_WEEKS: usize = 260;

#[derive(Debug, Clone)]
pub struct ProspectiveWeeklyObs {
    pub week_friday: String,
    pub spy_weekly_return_pct: Option<f64>,
    pub btc_weekly_return_pct: Option<f64>,
    pub btc_etf_flow_usd: Option<f64>,
    pub delta_btc_oi_usd: Option<f64>,
    pub bil_exact_flow_usd: Option<f64>,
    pub jnk_exact_flow_usd: Option<f64>,
    pub gld_exact_flow_usd: Option<f64>,
}

pub struct ProspectiveScorecardInputs {
    pub as_of_date: NaiveDate,
    pub weekly_rows: Vec<ProspectiveWeeklyObs>,
    pub daily_move_bil_oos_days: usize,
    pub daily_move_bil_running_corr: Option<f64>,
    pub latest_ex_rrp_tci_z: Option<f64>,
    pub latest_ex_rrp_bias_adj_pct: Option<f64>,
}

/// Evaluates cumulative out-of-sample progress strictly on weeks >= `2026-12-04`
/// without altering frozen rule forms or emitting premature confirmatory claims.
pub fn evaluate_prospective_scorecard(
    inputs: &ProspectiveScorecardInputs,
) -> ProspectiveScorecardReport {
    let as_of_str = inputs.as_of_date.format("%Y-%m-%d").to_string();
    // Strictly exclude partial in-progress weeks whose Friday EOW has not yet closed on as_of_date
    // (e.g., Saturday 2026-11-28 24/7 crypto candles aligned forward to Friday 2026-12-04).
    let mut rows: Vec<ProspectiveWeeklyObs> = inputs
        .weekly_rows
        .iter()
        .filter(|r| r.week_friday.as_str() <= as_of_str.as_str())
        .cloned()
        .collect();
    rows.sort_by(|a, b| a.week_friday.cmp(&b.week_friday));
    let latest_panel_week = rows.last().map(|r| r.week_friday.clone());

    let oos_rows: Vec<&ProspectiveWeeklyObs> = rows
        .iter()
        .filter(|r| r.week_friday.as_str() >= OOS_START_WEEK)
        .collect();
    let oos_weeks = oos_rows.len();
    let primary_rem = PRIMARY_READOUT_WEEKS.saturating_sub(oos_weeks);
    let secondary_rem = SECONDARY_READOUT_WEEKS.saturating_sub(oos_weeks);

    let primary_status = readout_status(oos_weeks, PRIMARY_READOUT_WEEKS);
    let secondary_status = readout_status(oos_weeks, SECONDARY_READOUT_WEEKS);

    // CONF_1: BTC weekly return <= -5.0% -> US Spot BTC ETF USD flow lower than non-crash weeks
    let (c1_obs, c1_trig, c1_diff) = crash_group_diff(
        &oos_rows,
        |r| r.btc_weekly_return_pct.map(|v| v <= -5.0),
        |r| r.btc_etf_flow_usd,
    );
    let conf_1 = ProspectiveRuleTracker {
        rule_id: "CONF_1_BTC_SPOT_ETF_WEEKLY_CRASH".to_string(),
        family: "primary_confirmatory_M2".to_string(),
        rule_class: "descriptive_contemporaneous".to_string(),
        readout_status: primary_status.to_string(),
        target_readout_weeks: PRIMARY_READOUT_WEEKS,
        oos_observations_collected: c1_obs,
        trigger_count: c1_trig,
        hit_count: None,
        running_metric_label: "crash_minus_noncrash_btc_etf_flow_usd_bn".to_string(),
        running_metric_value: c1_diff.map(|v| v / 1e9),
        comparator_value: Some(-1.18),
        progress_pct: progress(oos_weeks, PRIMARY_READOUT_WEEKS),
        governance_note: "Holm-Bonferroni (M=2) 10,000-perm readout locked until Week 104 (2028-11-24).".to_string(),
    };

    // CONF_2: Ex-RRP circular-shift surrogate TCI z-score
    let conf_2 = ProspectiveRuleTracker {
        rule_id: "CONF_2_EX_RRP_CIRCULAR_SURROGATE_TCI".to_string(),
        family: "primary_confirmatory_M2".to_string(),
        rule_class: "reduced_form_connectedness".to_string(),
        readout_status: primary_status.to_string(),
        target_readout_weeks: PRIMARY_READOUT_WEEKS,
        oos_observations_collected: oos_weeks,
        trigger_count: usize::from(inputs.latest_ex_rrp_tci_z.is_some_and(|z| z > 2.0)),
        hit_count: None,
        running_metric_label: "latest_ex_rrp_circular_shift_tci_z".to_string(),
        running_metric_value: inputs.latest_ex_rrp_tci_z,
        comparator_value: inputs.latest_ex_rrp_bias_adj_pct,
        progress_pct: progress(oos_weeks, PRIMARY_READOUT_WEEKS),
        governance_note: "Reduced-form non-independence gauge only; confirmatory test evaluated on 156w window ending Week 104.".to_string(),
    };

    // SEC_1: Daily ^MOVE -> BIL (controlled; 4.7-yr horizon = 1,179 trading days)
    let sec_1 = ProspectiveRuleTracker {
        rule_id: "SEC_1_DAILY_MOVE_TO_BIL_CONTROLLED".to_string(),
        family: "secondary_long_horizon_M5".to_string(),
        rule_class: "conditional_association".to_string(),
        readout_status: secondary_status.to_string(),
        target_readout_weeks: SECONDARY_READOUT_WEEKS,
        oos_observations_collected: inputs.daily_move_bil_oos_days,
        trigger_count: 0,
        hit_count: None,
        running_metric_label: "oos_daily_move_bil_pearson_r".to_string(),
        running_metric_value: inputs.daily_move_bil_running_corr,
        comparator_value: Some(1179.0),
        progress_pct: progress(inputs.daily_move_bil_oos_days, 1179),
        governance_note: "Requires ~1,179 OOS trading days (~4.7y) for 80% HAC(10) power; tracker only.".to_string(),
    };

    // SEC_2: BTC crash -> delta BTC OI
    let (s2_obs, s2_trig, s2_diff) = crash_group_diff(
        &oos_rows,
        |r| r.btc_weekly_return_pct.map(|v| v <= -5.0),
        |r| r.delta_btc_oi_usd,
    );
    let sec_2 = build_sec_crash_tracker(
        "SEC_2_BTC_OI_WEEKLY_CRASH",
        secondary_status,
        s2_obs,
        s2_trig,
        s2_diff.map(|v| v / 1e9),
        "crash_minus_noncrash_delta_btc_oi_usd_bn",
        oos_weeks,
        "Needs ~204 weeks for 80% Holm power; no confirmatory verdict before Week 260.",
    );

    // SEC_3: SPY crash -> BIL inflow
    let (s3_obs, s3_trig, s3_diff) = crash_group_diff(
        &oos_rows,
        |r| r.spy_weekly_return_pct.map(|v| v <= -2.0),
        |r| r.bil_exact_flow_usd,
    );
    let sec_3 = build_sec_crash_tracker(
        "SEC_3_BIL_WEEKLY_CRASH",
        secondary_status,
        s3_obs,
        s3_trig,
        s3_diff.map(|v| v / 1e9),
        "crash_minus_noncrash_bil_exact_flow_usd_bn",
        oos_weeks,
        "Needs ~315 weeks for 80% Holm power; secondary tracker only.",
    );

    // SEC_4: SPY crash -> JNK outflow
    let (s4_obs, s4_trig, s4_diff) = crash_group_diff(
        &oos_rows,
        |r| r.spy_weekly_return_pct.map(|v| v <= -2.0),
        |r| r.jnk_exact_flow_usd,
    );
    let sec_4 = build_sec_crash_tracker(
        "SEC_4_JNK_WEEKLY_CRASH",
        secondary_status,
        s4_obs,
        s4_trig,
        s4_diff.map(|v| v / 1e9),
        "crash_minus_noncrash_jnk_exact_flow_usd_bn",
        oos_weeks,
        "Needs ~338 weeks for 80% Holm power; secondary tracker only.",
    );

    // SEC_5: Strictly expanding Q80(BIL) & Q20(GLD) joint classifier for SPY <= -2%
    let (s5_obs, s5_alerts, s5_hits, s5_lift, s5_base_rate) = eval_sec5_expanding_classifier(&rows);
    let sec_5 = ProspectiveRuleTracker {
        rule_id: "SEC_5_Q2_BIL_GLD_CLASSIFIER_UNDERPOWERED_TRACKER".to_string(),
        family: "secondary_long_horizon_M5".to_string(),
        rule_class: "probabilistic_contemporaneous_classifier".to_string(),
        readout_status: if s5_alerts >= 35 && oos_weeks >= SECONDARY_READOUT_WEEKS {
            "READY_FOR_FIXED_CALENDAR_READOUT".to_string()
        } else {
            secondary_status.to_string()
        },
        target_readout_weeks: SECONDARY_READOUT_WEEKS,
        oos_observations_collected: s5_obs,
        trigger_count: s5_alerts,
        hit_count: Some(s5_hits),
        running_metric_label: "oos_running_lift_vs_base_rate".to_string(),
        running_metric_value: s5_lift,
        comparator_value: s5_base_rate,
        progress_pct: progress(s5_alerts, 35),
        governance_note: "Strictly expanding pre-week quantiles (zero lookahead); do NOT judge before >=35 alerts.".to_string(),
    };

    let primary_rules = vec![conf_1, conf_2];
    let secondary_rules = vec![sec_1, sec_2, sec_3, sec_4, sec_5];
    let overall_status = primary_status.to_string();

    let hash_payload = serde_json::json!({
        "protocol_sha256": PROSPECTIVE_PROTOCOL_SHA256,
        "as_of_date": &as_of_str,
        "latest_panel_week": &latest_panel_week,
        "oos_weeks_collected": oos_weeks,
        "primary_rules_m2": &primary_rules,
        "secondary_rules_m5": &secondary_rules,
    });
    let checksum = sha256_hex(hash_payload.to_string().as_bytes());
    let scorecard_id = format!(
        "oos_{}_{}",
        as_of_str.replace('-', ""),
        &Uuid::new_v4().to_string()[..8]
    );

    ProspectiveScorecardReport {
        schema_version: CONTINUOUS_SCHEMA_VERSION,
        scorecard_id,
        evaluated_at_utc: Utc::now(),
        as_of_date: as_of_str,
        protocol_version: PROSPECTIVE_PROTOCOL_VERSION.to_string(),
        protocol_anchor_tag: PROSPECTIVE_ANCHOR_TAG.to_string(),
        protocol_sha256: PROSPECTIVE_PROTOCOL_SHA256.to_string(),
        burn_in_end_week: BURN_IN_END_WEEK.to_string(),
        oos_start_week: OOS_START_WEEK.to_string(),
        primary_readout_date_utc: PRIMARY_READOUT_DATE_UTC.to_string(),
        secondary_readout_date_utc: SECONDARY_READOUT_DATE_UTC.to_string(),
        latest_panel_week,
        oos_weeks_collected: oos_weeks,
        primary_weeks_remaining: primary_rem,
        secondary_weeks_remaining: secondary_rem,
        optional_stopping_forbidden: true,
        overall_readout_status: overall_status,
        primary_rules_m2: primary_rules,
        secondary_rules_m5: secondary_rules,
        scorecard_checksum_sha256: checksum,
        persisted_path: None,
    }
}

fn readout_status(collected: usize, target: usize) -> &'static str {
    if collected == 0 {
        "PRE_OOS_BURN_IN_COMPLETE"
    } else if collected < target {
        "ACCUMULATING_OOS_NO_PEEKING"
    } else {
        "READY_FOR_FIXED_CALENDAR_READOUT"
    }
}

fn progress(done: usize, target: usize) -> f64 {
    if target == 0 {
        return 100.0;
    }
    ((done as f64 / target as f64) * 100.0).min(100.0)
}

#[allow(clippy::too_many_arguments)]
fn build_sec_crash_tracker(
    id: &str,
    status: &str,
    obs: usize,
    triggers: usize,
    diff_bn: Option<f64>,
    label: &str,
    oos_weeks: usize,
    note: &str,
) -> ProspectiveRuleTracker {
    ProspectiveRuleTracker {
        rule_id: id.to_string(),
        family: "secondary_long_horizon_M5".to_string(),
        rule_class: "descriptive_contemporaneous".to_string(),
        readout_status: status.to_string(),
        target_readout_weeks: SECONDARY_READOUT_WEEKS,
        oos_observations_collected: obs,
        trigger_count: triggers,
        hit_count: None,
        running_metric_label: label.to_string(),
        running_metric_value: diff_bn,
        comparator_value: None,
        progress_pct: progress(oos_weeks, SECONDARY_READOUT_WEEKS),
        governance_note: note.to_string(),
    }
}

fn crash_group_diff<FC, FV>(
    rows: &[&ProspectiveWeeklyObs],
    is_crash: FC,
    val_fn: FV,
) -> (usize, usize, Option<f64>)
where
    FC: Fn(&ProspectiveWeeklyObs) -> Option<bool>,
    FV: Fn(&ProspectiveWeeklyObs) -> Option<f64>,
{
    let mut crash_vals = Vec::new();
    let mut non_vals = Vec::new();
    for r in rows {
        if let (Some(c), Some(v)) = (is_crash(r), val_fn(r).filter(|x| x.is_finite())) {
            if c {
                crash_vals.push(v);
            } else {
                non_vals.push(v);
            }
        }
    }
    let obs = crash_vals.len() + non_vals.len();
    let diff = if !crash_vals.is_empty() && !non_vals.is_empty() {
        let mc = crash_vals.iter().sum::<f64>() / crash_vals.len() as f64;
        let mn = non_vals.iter().sum::<f64>() / non_vals.len() as f64;
        Some(mc - mn)
    } else {
        None
    };
    (obs, crash_vals.len(), diff)
}

fn eval_sec5_expanding_classifier(
    all_sorted: &[ProspectiveWeeklyObs],
) -> (usize, usize, usize, Option<f64>, Option<f64>) {
    let mut hist_bil: Vec<f64> = Vec::new();
    let mut hist_gld: Vec<f64> = Vec::new();
    let mut oos_obs = 0usize;
    let mut oos_crashes = 0usize;
    let mut alerts = 0usize;
    let mut hits = 0usize;

    for r in all_sorted {
        let is_oos = r.week_friday.as_str() >= OOS_START_WEEK;
        if is_oos {
            if let (Some(spy), Some(bil), Some(gld)) = (
                r.spy_weekly_return_pct.filter(|x| x.is_finite()),
                r.bil_exact_flow_usd.filter(|x| x.is_finite()),
                r.gld_exact_flow_usd.filter(|x| x.is_finite()),
            ) {
                if hist_bil.len() >= 20 && hist_gld.len() >= 20 {
                    let q80_bil = quantile(&hist_bil, 0.80);
                    let q20_gld = quantile(&hist_gld, 0.20);
                    oos_obs += 1;
                    let crash = spy <= -2.0;
                    if crash {
                        oos_crashes += 1;
                    }
                    if bil > q80_bil && gld < q20_gld {
                        alerts += 1;
                        if crash {
                            hits += 1;
                        }
                    }
                }
            }
        }
        if let Some(b) = r.bil_exact_flow_usd.filter(|x| x.is_finite()) {
            hist_bil.push(b);
        }
        if let Some(g) = r.gld_exact_flow_usd.filter(|x| x.is_finite()) {
            hist_gld.push(g);
        }
    }

    let base_rate = (oos_obs > 0).then(|| oos_crashes as f64 / oos_obs as f64);
    let lift = match (base_rate, alerts > 0) {
        (Some(br), true) if br > 0.0 => Some((hits as f64 / alerts as f64) / br),
        _ => None,
    };
    (oos_obs, alerts, hits, lift, base_rate)
}

fn quantile(vals: &[f64], q: f64) -> f64 {
    let mut sorted = vals.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
