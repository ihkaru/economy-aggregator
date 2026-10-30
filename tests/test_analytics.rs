use std::collections::HashMap;
use chrono::NaiveDate;
use economy_aggregator::features::analytics::{
    DayCountConvention, FxNormalizer, PortfolioEngine,
    SyntheticBondAccumulator, TradingCalendarAligner,
};

#[test]
fn test_calendar_aligner_forward_fill() {
    let mut series_map = HashMap::new();

    // Asset A: trades on Oct 1, Oct 2, Oct 4 (holiday on Oct 3)
    let a_pts = vec![
        (NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(), 100.0),
        (NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(), 102.0),
        (NaiveDate::from_ymd_opt(2026, 10, 4).unwrap(), 104.0),
    ];
    series_map.insert("ASSET_A".to_string(), a_pts);

    // Asset B: trades on Oct 1, Oct 3, Oct 4 (holiday on Oct 2)
    let b_pts = vec![
        (NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(), 50.0),
        (NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(), 55.0),
        (NaiveDate::from_ymd_opt(2026, 10, 4).unwrap(), 52.0),
    ];
    series_map.insert("ASSET_B".to_string(), b_pts);

    let panel = TradingCalendarAligner::align(&series_map, true).expect("Alignment failed");

    // Unified dates: Oct 1, 2, 3, 4 -> 4 dates
    assert_eq!(panel.dates.len(), 4);
    assert_eq!(panel.returns.len(), 3);

    let a_idx = panel.symbol_index("ASSET_A").unwrap();
    let b_idx = panel.symbol_index("ASSET_B").unwrap();

    // Check Oct 3 (index 2): Asset A should forward-fill 102.0, Asset B should be 55.0
    assert_eq!(panel.prices[2][a_idx], 102.0);
    assert_eq!(panel.prices[2][b_idx], 55.0);

    // Return on Oct 3 for Asset A should be 0.0 (no change due to forward-fill)
    assert_eq!(panel.returns[1][a_idx], 0.0);
}

#[test]
fn test_fx_normalizer_multiplication_and_division() {
    let asset_usd = vec![
        (NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(), 100.0),
        (NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(), 105.0),
    ];

    let usd_idr = vec![
        (NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(), 10_000.0),
        (NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(), 10_100.0),
    ];

    let norm_idr = FxNormalizer::normalize_by_multiplication(&asset_usd, &usd_idr);
    assert_eq!(norm_idr.len(), 2);
    assert_eq!(norm_idr[0].1, 1_000_000.0);
    assert_eq!(norm_idr[1].1, 105.0 * 10_100.0);

    let back_to_usd = FxNormalizer::normalize_by_division(&norm_idr, &usd_idr);
    assert!((back_to_usd[0].1 - 100.0).abs() < 1e-6);
    assert!((back_to_usd[1].1 - 105.0).abs() < 1e-6);
}

#[test]
fn test_synthetic_bond_accumulator_compounding() {
    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
    let yield_rate = 0.072; // 7.2% annual
    let base = 100.0;

    let tri = SyntheticBondAccumulator::generate_constant_yield_tri(
        start,
        end,
        yield_rate,
        base,
        DayCountConvention::Actual365,
    );

    assert_eq!(tri.len(), 365);
    let final_val = tri.last().unwrap().1;
    // Compounded after 364 daily steps: base * (1 + 0.072)^(364/365) ~ 107.177
    assert!((final_val - 107.177).abs() < 0.1);
}

#[test]
fn test_portfolio_engine_metrics_and_correlation() {
    let mut series_map = HashMap::new();

    // Construct series with non-constant varying returns
    let deltas_a = vec![0.02, -0.01, 0.03, -0.02, 0.01, 0.04, -0.03];
    let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

    let mut p1 = Vec::new();
    let mut p2 = Vec::new();
    let mut v1 = 100.0;
    let mut v2 = 100.0;

    p1.push((start, v1));
    p2.push((start, v2));

    for (i, d_a) in deltas_a.iter().enumerate() {
        let d = start + chrono::Duration::days(i as i64 + 1);
        v1 *= 1.0 + d_a;
        v2 *= 1.0 - d_a; // exactly opposite returns
        p1.push((d, v1));
        p2.push((d, v2));
    }

    series_map.insert("ASSET_UP".to_string(), p1);
    series_map.insert("ASSET_DOWN".to_string(), p2);

    let panel = TradingCalendarAligner::align(&series_map, true).unwrap();

    let mut weights = HashMap::new();
    weights.insert("ASSET_UP".to_string(), 0.5);
    weights.insert("ASSET_DOWN".to_string(), 0.5);

    let metrics = PortfolioEngine::evaluate(&panel, &weights, 0.0).expect("Evaluation failed");

    // Negative correlation test: ASSET_UP and ASSET_DOWN should have corr close to -1.0
    let corr_matrix = &metrics.correlation_matrix;
    let corr_up_down = corr_matrix["ASSET_UP"]["ASSET_DOWN"];
    assert!((corr_up_down - (-1.0)).abs() < 1e-4);
}
