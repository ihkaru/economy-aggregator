//! Apache Arrow schema definition and RecordBatch conversion for market candles.

use std::sync::Arc;
use arrow::array::{
    ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray,
    TimestampNanosecondArray, UInt32Array, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use crate::core::types::AggregatorCandle;

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
    ];

    RecordBatch::try_new(schema, columns)
}
