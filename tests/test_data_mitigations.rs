use chrono::{Datelike, NaiveDate, TimeZone, Utc, Weekday};
use economy_aggregator::features::analytics::{
    CorporateActionAdjuster, DividendEvent, ForexSessionNormalizer,
    FuturesRollEngine, FuturesRollEvent, MacroVintageRecord, PointInTimeEngine,
    RollMethod, SplitEvent, VintageStage,
};

#[test]
fn test_corporate_actions_split_and_dividend_adjustment() {
    let d1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
    let d3 = NaiveDate::from_ymd_opt(2026, 1, 3).unwrap(); // Split 1:4 occurs here
    let d4 = NaiveDate::from_ymd_opt(2026, 1, 4).unwrap(); // $2.00 dividend occurs here
    let d5 = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

    // Raw unadjusted prices: on d3 stock splits 1:4, dropping from 400.0 to 100.0
    let raw = vec![
        (d1, 390.0),
        (d2, 400.0),
        (d3, 100.0), // Split 4-for-1: 400 becomes 100
        (d4, 98.0),  // $2.00 dividend paid
        (d5, 100.0),
    ];

    let splits = vec![SplitEvent {
        date: d3,
        from_shares: 1.0,
        to_shares: 4.0, // 4-for-1
    }];

    let dividends = vec![DividendEvent {
        ex_date: d4,
        cash_amount: 2.0,
    }];

    let adjusted = CorporateActionAdjuster::adjust_backward(&raw, &splits, &dividends);
    assert_eq!(adjusted.len(), 5);

    // Latest price (d5) factor is 1.0 -> adjusted_close == raw_close
    assert_eq!(adjusted[4].adjusted_close, 100.0);

    // Pre-split prices (d1, d2) should be scaled down by approx 1/4 factor
    // d2 adjusted close should be close to 100.0 (continuity with d3's 100.0)
    let d2_adj = adjusted[1].adjusted_close;
    let d3_adj = adjusted[2].adjusted_close;

    // Verify continuity: return on split date should be near 0% instead of -75% crash!
    let split_day_continuous = CorporateActionAdjuster::is_return_continuous(d2_adj, d3_adj, 0.05);
    assert!(split_day_continuous, "Split adjustment failed to produce continuous price series!");
}

#[test]
fn test_bitemporal_macro_point_in_time_lookahead_prevention() {
    let t_preliminary = Utc.with_ymd_and_hms(2024, 4, 25, 12, 30, 0).unwrap();
    let t_revised = Utc.with_ymd_and_hms(2024, 5, 30, 12, 30, 0).unwrap();
    let t_final = Utc.with_ymd_and_hms(2024, 6, 27, 12, 30, 0).unwrap();

    let records = vec![
        MacroVintageRecord {
            indicator_code: "US_GDP_QOQ".to_string(),
            country: "US".to_string(),
            reference_period: "2024-Q1".to_string(),
            release_timestamp_utc: t_preliminary,
            reported_value: 1.6, // Advance Estimate
            vintage_stage: VintageStage::Preliminary,
            revision_number: 1,
        },
        MacroVintageRecord {
            indicator_code: "US_GDP_QOQ".to_string(),
            country: "US".to_string(),
            reference_period: "2024-Q1".to_string(),
            release_timestamp_utc: t_revised,
            reported_value: 1.3, // Second Estimate (revised down)
            vintage_stage: VintageStage::SecondEstimate,
            revision_number: 2,
        },
        MacroVintageRecord {
            indicator_code: "US_GDP_QOQ".to_string(),
            country: "US".to_string(),
            reference_period: "2024-Q1".to_string(),
            release_timestamp_utc: t_final,
            reported_value: 1.4, // Third / Final Estimate
            vintage_stage: VintageStage::FinalRevised,
            revision_number: 3,
        },
    ];

    // Simulated backtest on May 10, 2024 (between preliminary and revised)
    let sim_time_may10 = Utc.with_ymd_and_hms(2024, 5, 10, 0, 0, 0).unwrap();
    let query_may10 = PointInTimeEngine::query_as_of(
        &records,
        "US_GDP_QOQ",
        "2024-Q1",
        sim_time_may10,
    );

    assert!(query_may10.is_some());
    let v_may10 = query_may10.unwrap();
    // Must strictly see 1.6% (Advance Estimate), NOT 1.3% or 1.4% from the future!
    assert_eq!(v_may10.reported_value, 1.6);
    assert_eq!(v_may10.vintage_stage, VintageStage::Preliminary);

    // Simulated backtest on June 1, 2024 (after second estimate)
    let sim_time_june1 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
    let query_june1 = PointInTimeEngine::query_as_of(
        &records,
        "US_GDP_QOQ",
        "2024-Q1",
        sim_time_june1,
    );

    assert!(query_june1.is_some());
    let v_june1 = query_june1.unwrap();
    // Sees the updated 1.3% value
    assert_eq!(v_june1.reported_value, 1.3);

    // Verify revision spread calculation
    let spread = PointInTimeEngine::compute_revision_spread(&records, "US_GDP_QOQ", "2024-Q1").unwrap();
    assert_eq!(spread.0, 1.6); // initial
    assert_eq!(spread.1, 1.4); // final
    assert!((spread.2 - (-0.2)).abs() < 1e-6); // -0.2 revision delta
}

#[test]
fn test_futures_roll_panama_and_additive_smoothing() {
    let d1 = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();
    let roll_date = NaiveDate::from_ymd_opt(2026, 3, 3).unwrap();
    let d4 = NaiveDate::from_ymd_opt(2026, 3, 4).unwrap();

    // Raw series stitched naively: contract expires on d3 jumping from 50 to 55 (contango gap of +5.0)
    let raw = vec![
        (d1, 48.0),
        (d2, 50.0), // Old contract close before roll
        (roll_date, 55.0), // New contract close after roll
        (d4, 56.0),
    ];

    let rolls = vec![FuturesRollEvent {
        roll_date,
        old_contract_price: 50.0,
        new_contract_price: 55.0,
    }];

    // 1. Panama Ratio Method
    let cont_panama = FuturesRollEngine::build_continuous_series(&raw, &rolls, RollMethod::PanamaRatio);
    assert_eq!(cont_panama.len(), 4);
    // Pre-roll prices are scaled by (55/50) = 1.10
    assert!((cont_panama[1].1 - 55.0).abs() < 1e-6);
    assert_eq!(cont_panama[2].1, 55.0);

    // 2. Additive Difference Method
    let cont_additive = FuturesRollEngine::build_continuous_series(&raw, &rolls, RollMethod::AdditiveDifference);
    assert_eq!(cont_additive.len(), 4);
    // Pre-roll prices are shifted by (55 - 50) = +5.0
    assert_eq!(cont_additive[1].1, 55.0);
    assert_eq!(cont_additive[2].1, 55.0);
}

#[test]
fn test_forex_weekend_session_normalization() {
    let sunday = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
    assert_eq!(sunday.weekday(), Weekday::Sun);

    // Sunday at 14:00 (before 17:00 institutional open) -> remains Sunday (outside trading)
    let pre_open = ForexSessionNormalizer::normalize_sunday_to_monday(sunday, 14);
    assert_eq!(pre_open, sunday);
    assert!(!ForexSessionNormalizer::is_institutional_forex_window(Weekday::Sun, 14));

    // Sunday at 18:00 (institutional Asian open) -> normalized into Monday trading session
    let post_open = ForexSessionNormalizer::normalize_sunday_to_monday(sunday, 18);
    let monday = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
    assert_eq!(post_open, monday);
    assert!(ForexSessionNormalizer::is_institutional_forex_window(Weekday::Sun, 18));

    // Saturday is strictly outside institutional FX window
    assert!(!ForexSessionNormalizer::is_institutional_forex_window(Weekday::Sat, 12));
}
