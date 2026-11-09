//! Integration test suite for Macroeconomic Calendar and Prediction Market Lakehouse & Analytics.

use chrono::{TimeZone, Utc};
use tempfile::tempdir;

use economy_aggregator::core::macro_types::{MetricUnitType, RichEconomicEvent, SourceAttribution};
use economy_aggregator::core::prediction_types::{
    PredictionMarketCandle, PredictionMarketContract, ProbabilityPriceSource, ResolutionOracleSpec,
};
use economy_aggregator::features::analytics::macro_prediction_bridge::MacroPredictionBridge;
use economy_aggregator::features::ingestion::economic_calendar::EconomicCalendarFetcher;
use economy_aggregator::features::storage::macro_arrow_schema::{
    macro_events_to_record_batch, record_batch_to_macro_events,
};
use economy_aggregator::features::storage::partition::PartitionResolver;
use economy_aggregator::features::storage::prediction_arrow_schema::{
    prediction_candles_to_record_batch, record_batch_to_prediction_candles,
};

#[test]
fn test_macro_event_arrow_roundtrip() {
    let dt = Utc.with_ymd_and_hms(2026, 11, 10, 13, 30, 0).unwrap();
    let event = RichEconomicEvent {
        event_id: "macro:US:cpi_m_m:20261110".to_string(),
        scheduled_utc: dt,
        country: "US".to_string(),
        currency: "USD".to_string(),
        category: "inflation".to_string(),
        indicator_code: "cpi_m_m".to_string(),
        title: "CPI m/m".to_string(),
        impact_tier: 3,
        is_all_day: false,
        is_tentative: false,
        raw_actual: Some("0.5%".to_string()),
        raw_forecast: Some("0.4%".to_string()),
        raw_previous: Some("0.3%".to_string()),
        normalized_actual: Some(0.005),
        normalized_forecast: Some(0.004),
        normalized_previous: Some(0.003),
        normalized_revised_previous: None,
        unit_type: MetricUnitType::Percentage,
        unit_multiplier: 0.01,
        unit_symbol: "%".to_string(),
        better_than_expected: Some(false),
        attribution: SourceAttribution {
            provider: "biquote".to_string(),
            original_id: "mql5:277493".to_string(),
            source_url: Some("https://www.bls.gov".to_string()),
            agency_name: Some("U.S. Bureau of Labor Statistics".to_string()),
            agency_url: Some("https://www.bls.gov".to_string()),
            observed_at_utc: dt,
        },
        revision_count: 0,
        revision_history_json: None,
    };

    assert_eq!(
        event.natural_key(),
        "US:cpi_m_m:2026-11-10T13:30:00+00:00:biquote"
    );

    // Convert to Arrow RecordBatch
    let batch = macro_events_to_record_batch(&[&event]).expect("Arrow batch creation failed");
    assert_eq!(batch.num_rows(), 1);
    assert_eq!(batch.num_columns(), 28);

    // Round-trip back to domain models
    let restored = record_batch_to_macro_events(&batch).expect("RecordBatch deserialization failed");
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].event_id, event.event_id);
    assert_eq!(restored[0].title, "CPI m/m");
    assert_eq!(restored[0].normalized_actual, Some(0.005));
    assert_eq!(restored[0].attribution.agency_name, Some("U.S. Bureau of Labor Statistics".to_string()));
}

#[test]
fn test_prediction_candle_arrow_roundtrip() {
    let dt = Utc.with_ymd_and_hms(2026, 11, 9, 12, 0, 0).unwrap();
    let candle = PredictionMarketCandle {
        time: dt.timestamp(),
        timestamp_utc: dt,
        market_id: "kalshi:KXELONMARS-99".to_string(),
        outcome: "YES".to_string(),
        timeframe: "1h".to_string(),
        prob_open: Some(0.60),
        prob_high: Some(0.65),
        prob_low: Some(0.58),
        prob_close: Some(0.62),
        book_bid_close: Some(0.61),
        book_ask_close: Some(0.63),
        volume_contracts: Some(15000.0),
        volume_usd: Some(9300.0),
        open_interest_usd: Some(45000.0),
        trade_count: Some(120),
        price_source: ProbabilityPriceSource::Trade,
        venue: "kalshi".to_string(),
    };

    let batch = prediction_candles_to_record_batch(&[&candle]).expect("Arrow batch failed");
    assert_eq!(batch.num_rows(), 1);

    let restored = record_batch_to_prediction_candles(&batch).expect("Deserialization failed");
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].market_id, "kalshi:KXELONMARS-99");
    assert_eq!(restored[0].prob_close, Some(0.62));
    assert_eq!(restored[0].price_source, ProbabilityPriceSource::Trade);
}

#[test]
fn test_partition_paths() {
    let temp = tempdir().unwrap();
    let resolver = PartitionResolver::new(temp.path().to_path_buf());

    let macro_path = resolver.resolve_macro_partition("US", 2026, 11);
    assert!(macro_path.ends_with("macro/calendar/country=US/year=2026/month=11"));

    let pred_path = resolver.resolve_prediction_candles_partition("kalshi", "KXELONMARS-99", "1h", 2026, 11);
    assert!(pred_path.ends_with("prediction_markets/candles/venue=kalshi/market=KXELONMARS-99/timeframe=1h/year=2026/month=11"));
}

#[test]
fn test_macro_prediction_bridge_divergence() {
    let dt = Utc.with_ymd_and_hms(2026, 11, 10, 13, 30, 0).unwrap();
    let event = RichEconomicEvent {
        event_id: "macro:US:cpi:20261110".to_string(),
        scheduled_utc: dt,
        country: "US".to_string(),
        currency: "USD".to_string(),
        category: "inflation".to_string(),
        indicator_code: "cpi".to_string(),
        title: "Consumer Price Index".to_string(),
        impact_tier: 3,
        is_all_day: false,
        is_tentative: false,
        raw_actual: Some("3.2%".to_string()),
        raw_forecast: Some("3.0%".to_string()),
        raw_previous: Some("2.9%".to_string()),
        normalized_actual: Some(0.032),
        normalized_forecast: Some(0.030),
        normalized_previous: Some(0.029),
        normalized_revised_previous: None,
        unit_type: MetricUnitType::Percentage,
        unit_multiplier: 0.01,
        unit_symbol: "%".to_string(),
        better_than_expected: Some(false),
        attribution: SourceAttribution {
            provider: "biquote".to_string(),
            original_id: "cpi_id".to_string(),
            source_url: Some("https://www.bls.gov".to_string()),
            agency_name: Some("U.S. Bureau of Labor Statistics".to_string()),
            agency_url: Some("https://www.bls.gov".to_string()),
            observed_at_utc: dt,
        },
        revision_count: 0,
        revision_history_json: None,
    };

    let contract = PredictionMarketContract {
        market_id: "kalshi:KXCPI-26NOV".to_string(),
        venue: "kalshi".to_string(),
        venue_market_id: "KXCPI-26NOV".to_string(),
        parent_event_id: "kalshi:KXCPI".to_string(),
        title: "Will CPI YoY be above 3.0% in November?".to_string(),
        outcome_label: "YES".to_string(),
        status: "open".to_string(),
        is_active: true,
        market_type: "binary".to_string(),
        book_model: "shared_complement".to_string(),
        outcomes: vec!["YES".to_string(), "NO".to_string()],
        winning_outcome: None,
        face_value: 1.0,
        tick_size: 0.001,
        close_time_utc: dt,
        settlement_time_utc: None,
        linked_macro_indicator: Some("macro:US:cpi".to_string()),
        oracle: ResolutionOracleSpec {
            oracle_type: "cftc_rules".to_string(),
            source_name: "BLS".to_string(),
            source_url: "https://www.bls.gov".to_string(),
            resolution_criteria: "Resolves per BLS release".to_string(),
        },
        venue_url: None,
    };

    let divergence = MacroPredictionBridge::compute_divergence(&event, &[contract]);
    assert_eq!(divergence.indicator_code, "cpi");
    assert_eq!(divergence.linked_markets_count, 1);
    assert_eq!(divergence.surprise_direction.as_deref(), Some("above_consensus"));
    assert_eq!(divergence.economist_forecast, Some(0.030));
    assert_eq!(divergence.actual_released_value, Some(0.032));
}

#[tokio::test]
async fn test_live_macro_fetch_and_lake_persist() {
    let temp = tempdir().unwrap();
    let resolver = PartitionResolver::new(temp.path().to_path_buf());

    let fetcher = EconomicCalendarFetcher::default();
    let events = fetcher
        .fetch_rich_events(Some("US"), Some("high"))
        .await
        .expect("Failed to fetch live macro events");

    assert!(!events.is_empty(), "Live macro events list should not be empty");

    // Persist to isolated lakehouse partition
    let saved_count = EconomicCalendarFetcher::persist_to_lake(&events, &resolver)
        .expect("Failed to persist macro events to lake");

    assert_eq!(saved_count, events.len());
}
