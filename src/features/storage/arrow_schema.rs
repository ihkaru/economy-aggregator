//! Apache Arrow schema definition and RecordBatch conversion for market candles.

use std::sync::Arc;
use chrono::{DateTime, Utc};
use arrow::array::{
    Array, ArrayRef, BooleanArray, DictionaryArray, Float64Array, LargeStringArray, RecordBatch,
    StringArray, TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    UInt32Array, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use crate::core::types::{AggregatorCandle, IngestionMode, TimeFrame};

pub fn candle_arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
        Field::new("canonical_symbol", DataType::Utf8, false),
        Field::new("timeframe", DataType::Utf8, false),
        Field::new("open", DataType::Float64, false),
        Field::new("high", DataType::Float64, false),
        Field::new("low", DataType::Float64, false),
        Field::new("close", DataType::Float64, false),
        Field::new("volume_base", DataType::Float64, false),
        Field::new("volume_quote", DataType::Float64, false),
        Field::new("taker_buy_volume_base", DataType::Float64, true),
        Field::new("taker_buy_volume_quote", DataType::Float64, true),
        Field::new("source", DataType::Utf8, false),
        Field::new("raw_symbol", DataType::Utf8, false),
        Field::new("ingestion_mode", DataType::Utf8, false),
        Field::new("is_settled", DataType::Boolean, false),
        Field::new("sequence_id", DataType::UInt64, true),
        Field::new("vwap", DataType::Float64, true),
        Field::new("trade_count", DataType::UInt32, true),
        Field::new("price_type", DataType::Utf8, false),
        Field::new("raw_metadata", DataType::Utf8, true),
    ]))
}

pub fn candles_to_record_batch(
    candles: &[&AggregatorCandle],
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let len = candles.len();
    let schema = candle_arrow_schema();

    let mut timestamps = Vec::with_capacity(len);
    let mut symbols = Vec::with_capacity(len);
    let mut timeframes = Vec::with_capacity(len);
    let mut opens = Vec::with_capacity(len);
    let mut highs = Vec::with_capacity(len);
    let mut lows = Vec::with_capacity(len);
    let mut closes = Vec::with_capacity(len);
    let mut vol_bases = Vec::with_capacity(len);
    let mut vol_quotes = Vec::with_capacity(len);
    let mut taker_vol_bases = Vec::with_capacity(len);
    let mut taker_vol_quotes = Vec::with_capacity(len);
    let mut sources = Vec::with_capacity(len);
    let mut raw_symbols = Vec::with_capacity(len);
    let mut ingestion_modes = Vec::with_capacity(len);
    let mut is_settled_vec = Vec::with_capacity(len);
    let mut sequence_ids = Vec::with_capacity(len);
    let mut vwaps = Vec::with_capacity(len);
    let mut trade_counts = Vec::with_capacity(len);
    let mut price_types = Vec::with_capacity(len);
    let mut raw_metadatas = Vec::with_capacity(len);

    for c in candles {
        timestamps.push(c.timestamp_utc.timestamp_nanos_opt().unwrap_or(0));
        symbols.push(c.canonical_symbol.as_str());
        timeframes.push(c.timeframe.as_str());
        opens.push(c.open);
        highs.push(c.high);
        lows.push(c.low);
        closes.push(c.close);
        vol_bases.push(c.volume_base);
        vol_quotes.push(c.volume_quote);
        taker_vol_bases.push(c.taker_buy_volume_base);
        taker_vol_quotes.push(c.taker_buy_volume_quote);
        sources.push(c.source.as_str());
        raw_symbols.push(c.raw_symbol.as_str());
        ingestion_modes.push(c.ingestion_mode.as_str());
        is_settled_vec.push(c.is_settled);
        sequence_ids.push(c.sequence_id);
        vwaps.push(c.vwap);
        trade_counts.push(c.trade_count);
        price_types.push(c.price_type.as_str());
        raw_metadatas.push(c.raw_metadata.as_deref());
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(TimestampNanosecondArray::from(timestamps).with_timezone("UTC")),
        Arc::new(StringArray::from(symbols)),
        Arc::new(StringArray::from(timeframes)),
        Arc::new(Float64Array::from(opens)),
        Arc::new(Float64Array::from(highs)),
        Arc::new(Float64Array::from(lows)),
        Arc::new(Float64Array::from(closes)),
        Arc::new(Float64Array::from(vol_bases)),
        Arc::new(Float64Array::from(vol_quotes)),
        Arc::new(Float64Array::from(taker_vol_bases)),
        Arc::new(Float64Array::from(taker_vol_quotes)),
        Arc::new(StringArray::from(sources)),
        Arc::new(StringArray::from(raw_symbols)),
        Arc::new(StringArray::from(ingestion_modes)),
        Arc::new(BooleanArray::from(is_settled_vec)),
        Arc::new(UInt64Array::from(sequence_ids)),
        Arc::new(Float64Array::from(vwaps)),
        Arc::new(UInt32Array::from(trade_counts)),
        Arc::new(StringArray::from(price_types)),
        Arc::new(StringArray::from(raw_metadatas)),
    ];

    RecordBatch::try_new(schema, columns)
}

/// Converts an Arrow RecordBatch into domain AggregatorCandle structs.
pub fn record_batch_to_candles(
    batch: &RecordBatch,
) -> Result<Vec<AggregatorCandle>, Box<dyn std::error::Error + Send + Sync>> {
    let ts_col = batch.column(0);
    let ts_micros: Vec<i64> = match ts_col.data_type() {
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            let arr = ts_col.as_any().downcast_ref::<TimestampNanosecondArray>().ok_or("Failed downcast ts nanos")?;
            (0..batch.num_rows()).map(|i| arr.value(i) / 1_000).collect()
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            let arr = ts_col.as_any().downcast_ref::<TimestampMicrosecondArray>().ok_or("Failed downcast ts micros")?;
            (0..batch.num_rows()).map(|i| arr.value(i)).collect()
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            let arr = ts_col.as_any().downcast_ref::<TimestampMillisecondArray>().ok_or("Failed downcast ts millis")?;
            (0..batch.num_rows()).map(|i| arr.value(i) * 1_000).collect()
        }
        dt => return Err(format!("Unexpected timestamp data type: {:?}", dt).into()),
    };

    let extract_str = |col_idx: usize, row_idx: usize| -> String {
        let col = batch.column(col_idx);
        if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
            return arr.value(row_idx).to_string();
        }
        if let Some(arr) = col.as_any().downcast_ref::<LargeStringArray>() {
            return arr.value(row_idx).to_string();
        }
        if let Some(dict) = col.as_any().downcast_ref::<DictionaryArray<arrow::datatypes::Int32Type>>() {
            if let Some(values) = dict.values().as_any().downcast_ref::<StringArray>() {
                if let Some(key) = dict.key(row_idx) {
                    return values.value(key).to_string();
                }
            }
        }
        String::new()
    };

    let open_array = batch.column(3).as_any().downcast_ref::<Float64Array>().ok_or("Invalid open col")?;
    let high_array = batch.column(4).as_any().downcast_ref::<Float64Array>().ok_or("Invalid high col")?;
    let low_array = batch.column(5).as_any().downcast_ref::<Float64Array>().ok_or("Invalid low col")?;
    let close_array = batch.column(6).as_any().downcast_ref::<Float64Array>().ok_or("Invalid close col")?;
    let vol_base_array = batch.column(7).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vol_base col")?;
    let vol_quote_array = batch.column(8).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vol_quote col")?;
    let taker_base_array = batch.column(9).as_any().downcast_ref::<Float64Array>().ok_or("Invalid taker_base col")?;
    let taker_quote_array = batch.column(10).as_any().downcast_ref::<Float64Array>().ok_or("Invalid taker_quote col")?;
    let is_settled_array = batch.column(14).as_any().downcast_ref::<BooleanArray>().ok_or("Invalid is_settled col")?;
    let sequence_id_array = batch.column(15).as_any().downcast_ref::<UInt64Array>().ok_or("Invalid seq col")?;
    let vwap_array = batch.column(16).as_any().downcast_ref::<Float64Array>().ok_or("Invalid vwap col")?;
    let trade_count_array = batch.column(17).as_any().downcast_ref::<UInt32Array>().ok_or("Invalid trade_count col")?;

    let mut candles = Vec::with_capacity(batch.num_rows());
    for i in 0..batch.num_rows() {
        let ts = DateTime::from_timestamp_micros(ts_micros[i]).unwrap_or_else(Utc::now);
        let tf_str = extract_str(2, i);
        let timeframe = TimeFrame::from_str_opt(&tf_str).unwrap_or(TimeFrame::M1);

        let mode_str = extract_str(13, i);
        let ingestion_mode = match mode_str.as_str() {
            "rest_batch" => IngestionMode::RestBatch,
            "reconciliation" => IngestionMode::Reconciliation,
            _ => IngestionMode::LiveWs,
        };

        candles.push(AggregatorCandle {
            time: ts.timestamp(),
            timestamp_utc: ts,
            canonical_symbol: extract_str(1, i),
            timeframe,
            open: open_array.value(i),
            high: high_array.value(i),
            low: low_array.value(i),
            close: close_array.value(i),
            volume_base: vol_base_array.value(i),
            volume_quote: vol_quote_array.value(i),
            taker_buy_volume_base: if taker_base_array.is_null(i) { None } else { Some(taker_base_array.value(i)) },
            taker_buy_volume_quote: if taker_quote_array.is_null(i) { None } else { Some(taker_quote_array.value(i)) },
            source: extract_str(11, i),
            raw_symbol: extract_str(12, i),
            ingestion_mode,
            is_settled: is_settled_array.value(i),
            sequence_id: if sequence_id_array.is_null(i) { None } else { Some(sequence_id_array.value(i)) },
            vwap: if vwap_array.is_null(i) { None } else { Some(vwap_array.value(i)) },
            trade_count: if trade_count_array.is_null(i) { None } else { Some(trade_count_array.value(i)) },
            price_type: extract_str(18, i),
            raw_metadata: if batch.num_columns() > 19 && !batch.column(19).is_null(i) {
                let s = extract_str(19, i);
                if s.is_empty() { None } else { Some(s) }
            } else {
                None
            },
        });
    }

    Ok(candles)
}
