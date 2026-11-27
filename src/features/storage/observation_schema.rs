//! Apache Arrow schema & RecordBatch conversions for universal bitemporal MetricObservations (Phase 309).

use std::sync::Arc;
use arrow::array::{
    Array, ArrayRef, Float64Array, RecordBatch, StringArray, TimestampNanosecondArray, UInt32Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use chrono::{TimeZone, Utc};

use crate::core::observation_types::{
    MetricObservation, ObservationFrequency, SourceProvenance, SourceTier,
};

pub fn observation_arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("observation_ts_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
        Field::new("known_at_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
        Field::new("revision_seq", DataType::UInt32, false),
        Field::new("series_id", DataType::Utf8, false),
        Field::new("domain", DataType::Utf8, false),
        Field::new("compartment", DataType::Utf8, true),
        Field::new("entity_id", DataType::Utf8, false),
        Field::new("metric_name", DataType::Utf8, false),
        Field::new("frequency", DataType::Utf8, false),
        Field::new("value", DataType::Float64, false),
        Field::new("raw_value", DataType::Float64, true),
        Field::new("unit", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("source_series_id", DataType::Utf8, false),
        Field::new("source_url", DataType::Utf8, false),
        Field::new("source_tier", DataType::Utf8, false),
        Field::new("source_checksum", DataType::Utf8, true),
        Field::new("batch_id", DataType::Utf8, false),
        Field::new("extra_json", DataType::Utf8, true),
    ]))
}

pub fn observations_to_record_batch(
    rows: &[&MetricObservation],
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let n = rows.len();
    let mut obs_ts = Vec::with_capacity(n);
    let mut known_ts = Vec::with_capacity(n);
    let mut revs = Vec::with_capacity(n);
    let mut series_ids = Vec::with_capacity(n);
    let mut domains = Vec::with_capacity(n);
    let mut compartments = Vec::with_capacity(n);
    let mut entities = Vec::with_capacity(n);
    let mut metrics = Vec::with_capacity(n);
    let mut freqs = Vec::with_capacity(n);
    let mut values = Vec::with_capacity(n);
    let mut raw_values = Vec::with_capacity(n);
    let mut units = Vec::with_capacity(n);
    let mut sources = Vec::with_capacity(n);
    let mut src_ids = Vec::with_capacity(n);
    let mut src_urls = Vec::with_capacity(n);
    let mut src_tiers = Vec::with_capacity(n);
    let mut src_checks = Vec::with_capacity(n);
    let mut batch_ids = Vec::with_capacity(n);
    let mut extras = Vec::with_capacity(n);

    for r in rows {
        obs_ts.push(r.observation_ts_utc.timestamp_nanos_opt().unwrap_or(0));
        known_ts.push(r.known_at_utc.timestamp_nanos_opt().unwrap_or(0));
        revs.push(r.revision_seq);
        series_ids.push(r.series_id.as_str());
        domains.push(r.domain.as_str());
        compartments.push(r.compartment.as_deref());
        entities.push(r.entity_id.as_str());
        metrics.push(r.metric_name.as_str());
        freqs.push(r.frequency.as_str());
        values.push(r.value);
        raw_values.push(r.raw_value);
        units.push(r.unit.as_str());
        sources.push(r.provenance.source.as_str());
        src_ids.push(r.provenance.source_series_id.as_str());
        src_urls.push(r.provenance.source_url.as_str());
        src_tiers.push(r.provenance.source_tier.as_str());
        src_checks.push(r.provenance.source_checksum.as_deref());
        batch_ids.push(r.provenance.batch_id.as_str());
        extras.push(r.provenance.extra_json.as_deref());
    }

    let cols: Vec<ArrayRef> = vec![
        Arc::new(TimestampNanosecondArray::from(obs_ts).with_timezone("UTC")),
        Arc::new(TimestampNanosecondArray::from(known_ts).with_timezone("UTC")),
        Arc::new(UInt32Array::from(revs)),
        Arc::new(StringArray::from(series_ids)),
        Arc::new(StringArray::from(domains)),
        Arc::new(StringArray::from(compartments)),
        Arc::new(StringArray::from(entities)),
        Arc::new(StringArray::from(metrics)),
        Arc::new(StringArray::from(freqs)),
        Arc::new(Float64Array::from(values)),
        Arc::new(Float64Array::from(raw_values)),
        Arc::new(StringArray::from(units)),
        Arc::new(StringArray::from(sources)),
        Arc::new(StringArray::from(src_ids)),
        Arc::new(StringArray::from(src_urls)),
        Arc::new(StringArray::from(src_tiers)),
        Arc::new(StringArray::from(src_checks)),
        Arc::new(StringArray::from(batch_ids)),
        Arc::new(StringArray::from(extras)),
    ];

    RecordBatch::try_new(observation_arrow_schema(), cols)
}

fn col_str<'a>(batch: &'a RecordBatch, idx: usize) -> Result<&'a StringArray, arrow::error::ArrowError> {
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| arrow::error::ArrowError::SchemaError(format!("Column {} is not StringArray", idx)))
}

fn col_ts<'a>(batch: &'a RecordBatch, idx: usize) -> Result<&'a TimestampNanosecondArray, arrow::error::ArrowError> {
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<TimestampNanosecondArray>()
        .ok_or_else(|| arrow::error::ArrowError::SchemaError(format!("Column {} is not TimestampNanosecondArray", idx)))
}

fn col_f64<'a>(batch: &'a RecordBatch, idx: usize) -> Result<&'a Float64Array, arrow::error::ArrowError> {
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| arrow::error::ArrowError::SchemaError(format!("Column {} is not Float64Array", idx)))
}

fn col_u32<'a>(batch: &'a RecordBatch, idx: usize) -> Result<&'a UInt32Array, arrow::error::ArrowError> {
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<UInt32Array>()
        .ok_or_else(|| arrow::error::ArrowError::SchemaError(format!("Column {} is not UInt32Array", idx)))
}

pub fn record_batch_to_observations(
    batch: &RecordBatch,
) -> Result<Vec<MetricObservation>, arrow::error::ArrowError> {
    let obs_ts = col_ts(batch, 0)?;
    let known_ts = col_ts(batch, 1)?;
    let revs = col_u32(batch, 2)?;
    let series_ids = col_str(batch, 3)?;
    let domains = col_str(batch, 4)?;
    let compartments = col_str(batch, 5)?;
    let entities = col_str(batch, 6)?;
    let metrics = col_str(batch, 7)?;
    let freqs = col_str(batch, 8)?;
    let values = col_f64(batch, 9)?;
    let raw_values = col_f64(batch, 10)?;
    let units = col_str(batch, 11)?;
    let sources = col_str(batch, 12)?;
    let src_ids = col_str(batch, 13)?;
    let src_urls = col_str(batch, 14)?;
    let src_tiers = col_str(batch, 15)?;
    let src_checks = col_str(batch, 16)?;
    let batch_ids = col_str(batch, 17)?;
    let extras = col_str(batch, 18)?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for i in 0..batch.num_rows() {
        out.push(MetricObservation {
            observation_ts_utc: Utc.timestamp_nanos(obs_ts.value(i)),
            known_at_utc: Utc.timestamp_nanos(known_ts.value(i)),
            revision_seq: revs.value(i),
            series_id: series_ids.value(i).to_string(),
            domain: domains.value(i).to_string(),
            compartment: (!compartments.is_null(i)).then(|| compartments.value(i).to_string()),
            entity_id: entities.value(i).to_string(),
            metric_name: metrics.value(i).to_string(),
            frequency: ObservationFrequency::from_str_lenient(freqs.value(i)),
            value: values.value(i),
            raw_value: (!raw_values.is_null(i)).then(|| raw_values.value(i)),
            unit: units.value(i).to_string(),
            provenance: SourceProvenance {
                source: sources.value(i).to_string(),
                source_series_id: src_ids.value(i).to_string(),
                source_url: src_urls.value(i).to_string(),
                source_tier: SourceTier::from_str_lenient(src_tiers.value(i)),
                source_checksum: (!src_checks.is_null(i)).then(|| src_checks.value(i).to_string()),
                batch_id: batch_ids.value(i).to_string(),
                extra_json: (!extras.is_null(i)).then(|| extras.value(i).to_string()),
            },
        });
    }
    Ok(out)
}
