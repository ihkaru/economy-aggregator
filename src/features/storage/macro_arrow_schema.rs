//! Apache Arrow schema definition and RecordBatch conversions for Macroeconomic Events.

use std::sync::Arc;
use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray,
    TimestampNanosecondArray, UInt32Array, UInt8Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use chrono::{TimeZone, Utc};
use crate::core::macro_types::{MetricUnitType, RichEconomicEvent, SourceAttribution};

pub fn macro_event_arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("event_id", DataType::Utf8, false),
        Field::new("scheduled_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
        Field::new("country", DataType::Utf8, false),
        Field::new("currency", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new("indicator_code", DataType::Utf8, false),
        Field::new("title", DataType::Utf8, false),
        Field::new("impact_tier", DataType::UInt8, false),
        Field::new("is_all_day", DataType::Boolean, false),
        Field::new("is_tentative", DataType::Boolean, false),
        Field::new("raw_actual", DataType::Utf8, true),
        Field::new("raw_forecast", DataType::Utf8, true),
        Field::new("raw_previous", DataType::Utf8, true),
        Field::new("normalized_actual", DataType::Float64, true),
        Field::new("normalized_forecast", DataType::Float64, true),
        Field::new("normalized_previous", DataType::Float64, true),
        Field::new("normalized_revised_previous", DataType::Float64, true),
        Field::new("unit_type", DataType::Utf8, false),
        Field::new("unit_multiplier", DataType::Float64, false),
        Field::new("unit_symbol", DataType::Utf8, false),
        Field::new("better_than_expected", DataType::Boolean, true),
        Field::new("provider", DataType::Utf8, false),
        Field::new("source_url", DataType::Utf8, true),
        Field::new("agency_name", DataType::Utf8, true),
        Field::new("agency_url", DataType::Utf8, true),
        Field::new("observed_at_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
        Field::new("revision_count", DataType::UInt32, false),
        Field::new("revision_history_json", DataType::Utf8, true),
    ]))
}

pub fn macro_events_to_record_batch(
    events: &[&RichEconomicEvent],
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let len = events.len();
    let schema = macro_event_arrow_schema();

    let mut event_ids = Vec::with_capacity(len);
    let mut scheduled_utcs = Vec::with_capacity(len);
    let mut countries = Vec::with_capacity(len);
    let mut currencies = Vec::with_capacity(len);
    let mut categories = Vec::with_capacity(len);
    let mut indicator_codes = Vec::with_capacity(len);
    let mut titles = Vec::with_capacity(len);
    let mut impact_tiers = Vec::with_capacity(len);
    let mut is_all_days = Vec::with_capacity(len);
    let mut is_tentatives = Vec::with_capacity(len);
    let mut raw_actuals = Vec::with_capacity(len);
    let mut raw_forecasts = Vec::with_capacity(len);
    let mut raw_previouses = Vec::with_capacity(len);
    let mut norm_actuals = Vec::with_capacity(len);
    let mut norm_forecasts = Vec::with_capacity(len);
    let mut norm_previouses = Vec::with_capacity(len);
    let mut norm_rev_previouses = Vec::with_capacity(len);
    let mut unit_types = Vec::with_capacity(len);
    let mut unit_multipliers = Vec::with_capacity(len);
    let mut unit_symbols = Vec::with_capacity(len);
    let mut better_than_expecteds = Vec::with_capacity(len);
    let mut providers = Vec::with_capacity(len);
    let mut source_urls = Vec::with_capacity(len);
    let mut agency_names = Vec::with_capacity(len);
    let mut agency_urls = Vec::with_capacity(len);
    let mut observed_at_utcs = Vec::with_capacity(len);
    let mut revision_counts = Vec::with_capacity(len);
    let mut revision_histories = Vec::with_capacity(len);

    for e in events {
        event_ids.push(e.event_id.as_str());
        scheduled_utcs.push(e.scheduled_utc.timestamp_nanos_opt().unwrap_or(0));
        countries.push(e.country.as_str());
        currencies.push(e.currency.as_str());
        categories.push(e.category.as_str());
        indicator_codes.push(e.indicator_code.as_str());
        titles.push(e.title.as_str());
        impact_tiers.push(e.impact_tier);
        is_all_days.push(e.is_all_day);
        is_tentatives.push(e.is_tentative);
        raw_actuals.push(e.raw_actual.as_deref());
        raw_forecasts.push(e.raw_forecast.as_deref());
        raw_previouses.push(e.raw_previous.as_deref());
        norm_actuals.push(e.normalized_actual);
        norm_forecasts.push(e.normalized_forecast);
        norm_previouses.push(e.normalized_previous);
        norm_rev_previouses.push(e.normalized_revised_previous);
        unit_types.push(e.unit_type.as_str());
        unit_multipliers.push(e.unit_multiplier);
        unit_symbols.push(e.unit_symbol.as_str());
        better_than_expecteds.push(e.better_than_expected);
        providers.push(e.attribution.provider.as_str());
        source_urls.push(e.attribution.source_url.as_deref());
        agency_names.push(e.attribution.agency_name.as_deref());
        agency_urls.push(e.attribution.agency_url.as_deref());
        observed_at_utcs.push(e.attribution.observed_at_utc.timestamp_nanos_opt().unwrap_or(0));
        revision_counts.push(e.revision_count);
        revision_histories.push(e.revision_history_json.as_deref());
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(event_ids)),
        Arc::new(TimestampNanosecondArray::from(scheduled_utcs).with_timezone("UTC")),
        Arc::new(StringArray::from(countries)),
        Arc::new(StringArray::from(currencies)),
        Arc::new(StringArray::from(categories)),
        Arc::new(StringArray::from(indicator_codes)),
        Arc::new(StringArray::from(titles)),
        Arc::new(UInt8Array::from(impact_tiers)),
        Arc::new(BooleanArray::from(is_all_days)),
        Arc::new(BooleanArray::from(is_tentatives)),
        Arc::new(StringArray::from(raw_actuals)),
        Arc::new(StringArray::from(raw_forecasts)),
        Arc::new(StringArray::from(raw_previouses)),
        Arc::new(Float64Array::from(norm_actuals)),
        Arc::new(Float64Array::from(norm_forecasts)),
        Arc::new(Float64Array::from(norm_previouses)),
        Arc::new(Float64Array::from(norm_rev_previouses)),
        Arc::new(StringArray::from(unit_types)),
        Arc::new(Float64Array::from(unit_multipliers)),
        Arc::new(StringArray::from(unit_symbols)),
        Arc::new(BooleanArray::from(better_than_expecteds)),
        Arc::new(StringArray::from(providers)),
        Arc::new(StringArray::from(source_urls)),
        Arc::new(StringArray::from(agency_names)),
        Arc::new(StringArray::from(agency_urls)),
        Arc::new(TimestampNanosecondArray::from(observed_at_utcs).with_timezone("UTC")),
        Arc::new(UInt32Array::from(revision_counts)),
        Arc::new(StringArray::from(revision_histories)),
    ];

    RecordBatch::try_new(schema, columns)
}

pub fn record_batch_to_macro_events(
    batch: &RecordBatch,
) -> Result<Vec<RichEconomicEvent>, arrow::error::ArrowError> {
    let num_rows = batch.num_rows();
    let mut events = Vec::with_capacity(num_rows);

    let event_ids = batch.column(0).as_any().downcast_ref::<StringArray>().unwrap();
    let scheduled_utcs = batch.column(1).as_any().downcast_ref::<TimestampNanosecondArray>().unwrap();
    let countries = batch.column(2).as_any().downcast_ref::<StringArray>().unwrap();
    let currencies = batch.column(3).as_any().downcast_ref::<StringArray>().unwrap();
    let categories = batch.column(4).as_any().downcast_ref::<StringArray>().unwrap();
    let indicator_codes = batch.column(5).as_any().downcast_ref::<StringArray>().unwrap();
    let titles = batch.column(6).as_any().downcast_ref::<StringArray>().unwrap();
    let impact_tiers = batch.column(7).as_any().downcast_ref::<UInt8Array>().unwrap();
    let is_all_days = batch.column(8).as_any().downcast_ref::<BooleanArray>().unwrap();
    let is_tentatives = batch.column(9).as_any().downcast_ref::<BooleanArray>().unwrap();
    let raw_actuals = batch.column(10).as_any().downcast_ref::<StringArray>().unwrap();
    let raw_forecasts = batch.column(11).as_any().downcast_ref::<StringArray>().unwrap();
    let raw_previouses = batch.column(12).as_any().downcast_ref::<StringArray>().unwrap();
    let norm_actuals = batch.column(13).as_any().downcast_ref::<Float64Array>().unwrap();
    let norm_forecasts = batch.column(14).as_any().downcast_ref::<Float64Array>().unwrap();
    let norm_previouses = batch.column(15).as_any().downcast_ref::<Float64Array>().unwrap();
    let norm_rev_previouses = batch.column(16).as_any().downcast_ref::<Float64Array>().unwrap();
    let unit_types = batch.column(17).as_any().downcast_ref::<StringArray>().unwrap();
    let unit_multipliers = batch.column(18).as_any().downcast_ref::<Float64Array>().unwrap();
    let unit_symbols = batch.column(19).as_any().downcast_ref::<StringArray>().unwrap();
    let better_than_expecteds = batch.column(20).as_any().downcast_ref::<BooleanArray>().unwrap();
    let providers = batch.column(21).as_any().downcast_ref::<StringArray>().unwrap();
    let source_urls = batch.column(22).as_any().downcast_ref::<StringArray>().unwrap();
    let agency_names = batch.column(23).as_any().downcast_ref::<StringArray>().unwrap();
    let agency_urls = batch.column(24).as_any().downcast_ref::<StringArray>().unwrap();
    let observed_at_utcs = batch.column(25).as_any().downcast_ref::<TimestampNanosecondArray>().unwrap();
    let revision_counts = batch.column(26).as_any().downcast_ref::<UInt32Array>().unwrap();
    let revision_histories = batch.column(27).as_any().downcast_ref::<StringArray>().unwrap();

    for i in 0..num_rows {
        let sched_nanos = scheduled_utcs.value(i);
        let sched_dt = Utc.timestamp_nanos(sched_nanos);

        let obs_nanos = observed_at_utcs.value(i);
        let obs_dt = Utc.timestamp_nanos(obs_nanos);

        let attribution = SourceAttribution {
            provider: providers.value(i).to_string(),
            original_id: event_ids.value(i).to_string(),
            source_url: if source_urls.is_null(i) { None } else { Some(source_urls.value(i).to_string()) },
            agency_name: if agency_names.is_null(i) { None } else { Some(agency_names.value(i).to_string()) },
            agency_url: if agency_urls.is_null(i) { None } else { Some(agency_urls.value(i).to_string()) },
            observed_at_utc: obs_dt,
        };

        events.push(RichEconomicEvent {
            event_id: event_ids.value(i).to_string(),
            scheduled_utc: sched_dt,
            country: countries.value(i).to_string(),
            currency: currencies.value(i).to_string(),
            category: categories.value(i).to_string(),
            indicator_code: indicator_codes.value(i).to_string(),
            title: titles.value(i).to_string(),
            impact_tier: impact_tiers.value(i),
            is_all_day: is_all_days.value(i),
            is_tentative: is_tentatives.value(i),
            raw_actual: if raw_actuals.is_null(i) { None } else { Some(raw_actuals.value(i).to_string()) },
            raw_forecast: if raw_forecasts.is_null(i) { None } else { Some(raw_forecasts.value(i).to_string()) },
            raw_previous: if raw_previouses.is_null(i) { None } else { Some(raw_previouses.value(i).to_string()) },
            normalized_actual: if norm_actuals.is_null(i) { None } else { Some(norm_actuals.value(i)) },
            normalized_forecast: if norm_forecasts.is_null(i) { None } else { Some(norm_forecasts.value(i)) },
            normalized_previous: if norm_previouses.is_null(i) { None } else { Some(norm_previouses.value(i)) },
            normalized_revised_previous: if norm_rev_previouses.is_null(i) { None } else { Some(norm_rev_previouses.value(i)) },
            unit_type: MetricUnitType::from_str_lenient(unit_types.value(i)),
            unit_multiplier: unit_multipliers.value(i),
            unit_symbol: unit_symbols.value(i).to_string(),
            better_than_expected: if better_than_expecteds.is_null(i) { None } else { Some(better_than_expecteds.value(i)) },
            attribution,
            revision_count: revision_counts.value(i),
            revision_history_json: if revision_histories.is_null(i) { None } else { Some(revision_histories.value(i).to_string()) },
        });
    }

    Ok(events)
}
