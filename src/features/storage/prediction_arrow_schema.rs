//! Apache Arrow schema definition and RecordBatch conversions for Prediction Markets.

use std::sync::Arc;
use arrow::array::{
    Array, ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray,
    TimestampNanosecondArray, UInt32Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use chrono::{TimeZone, Utc};
use crate::core::prediction_types::{
    PredictionMarketCandle, ProbabilityPriceSource,
};

pub fn prediction_candle_arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("time", DataType::Int64, false),
        Field::new("timestamp_utc", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false),
        Field::new("market_id", DataType::Utf8, false),
        Field::new("outcome", DataType::Utf8, false),
        Field::new("timeframe", DataType::Utf8, false),
        Field::new("prob_open", DataType::Float64, true),
        Field::new("prob_high", DataType::Float64, true),
        Field::new("prob_low", DataType::Float64, true),
        Field::new("prob_close", DataType::Float64, true),
        Field::new("book_bid_close", DataType::Float64, true),
        Field::new("book_ask_close", DataType::Float64, true),
        Field::new("volume_contracts", DataType::Float64, true),
        Field::new("volume_usd", DataType::Float64, true),
        Field::new("open_interest_usd", DataType::Float64, true),
        Field::new("trade_count", DataType::UInt32, true),
        Field::new("price_source", DataType::Utf8, false),
        Field::new("venue", DataType::Utf8, false),
    ]))
}

pub fn prediction_candles_to_record_batch(
    candles: &[&PredictionMarketCandle],
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let len = candles.len();
    let schema = prediction_candle_arrow_schema();

    let mut times = Vec::with_capacity(len);
    let mut timestamps = Vec::with_capacity(len);
    let mut market_ids = Vec::with_capacity(len);
    let mut outcomes = Vec::with_capacity(len);
    let mut timeframes = Vec::with_capacity(len);
    let mut opens = Vec::with_capacity(len);
    let mut highs = Vec::with_capacity(len);
    let mut lows = Vec::with_capacity(len);
    let mut closes = Vec::with_capacity(len);
    let mut bids = Vec::with_capacity(len);
    let mut asks = Vec::with_capacity(len);
    let mut vol_contracts = Vec::with_capacity(len);
    let mut vol_usds = Vec::with_capacity(len);
    let mut ois = Vec::with_capacity(len);
    let mut counts = Vec::with_capacity(len);
    let mut sources = Vec::with_capacity(len);
    let mut venues = Vec::with_capacity(len);

    for c in candles {
        times.push(c.time);
        timestamps.push(c.timestamp_utc.timestamp_nanos_opt().unwrap_or(0));
        market_ids.push(c.market_id.as_str());
        outcomes.push(c.outcome.as_str());
        timeframes.push(c.timeframe.as_str());
        opens.push(c.prob_open);
        highs.push(c.prob_high);
        lows.push(c.prob_low);
        closes.push(c.prob_close);
        bids.push(c.book_bid_close);
        asks.push(c.book_ask_close);
        vol_contracts.push(c.volume_contracts);
        vol_usds.push(c.volume_usd);
        ois.push(c.open_interest_usd);
        counts.push(c.trade_count);
        sources.push(c.price_source.as_str());
        venues.push(c.venue.as_str());
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(times)),
        Arc::new(TimestampNanosecondArray::from(timestamps).with_timezone("UTC")),
        Arc::new(StringArray::from(market_ids)),
        Arc::new(StringArray::from(outcomes)),
        Arc::new(StringArray::from(timeframes)),
        Arc::new(Float64Array::from(opens)),
        Arc::new(Float64Array::from(highs)),
        Arc::new(Float64Array::from(lows)),
        Arc::new(Float64Array::from(closes)),
        Arc::new(Float64Array::from(bids)),
        Arc::new(Float64Array::from(asks)),
        Arc::new(Float64Array::from(vol_contracts)),
        Arc::new(Float64Array::from(vol_usds)),
        Arc::new(Float64Array::from(ois)),
        Arc::new(UInt32Array::from(counts)),
        Arc::new(StringArray::from(sources)),
        Arc::new(StringArray::from(venues)),
    ];

    RecordBatch::try_new(schema, columns)
}

pub fn record_batch_to_prediction_candles(
    batch: &RecordBatch,
) -> Result<Vec<PredictionMarketCandle>, arrow::error::ArrowError> {
    let num_rows = batch.num_rows();
    let mut candles = Vec::with_capacity(num_rows);

    let times = batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap();
    let timestamps = batch.column(1).as_any().downcast_ref::<TimestampNanosecondArray>().unwrap();
    let market_ids = batch.column(2).as_any().downcast_ref::<StringArray>().unwrap();
    let outcomes = batch.column(3).as_any().downcast_ref::<StringArray>().unwrap();
    let timeframes = batch.column(4).as_any().downcast_ref::<StringArray>().unwrap();
    let opens = batch.column(5).as_any().downcast_ref::<Float64Array>().unwrap();
    let highs = batch.column(6).as_any().downcast_ref::<Float64Array>().unwrap();
    let lows = batch.column(7).as_any().downcast_ref::<Float64Array>().unwrap();
    let closes = batch.column(8).as_any().downcast_ref::<Float64Array>().unwrap();
    let bids = batch.column(9).as_any().downcast_ref::<Float64Array>().unwrap();
    let asks = batch.column(10).as_any().downcast_ref::<Float64Array>().unwrap();
    let vol_contracts = batch.column(11).as_any().downcast_ref::<Float64Array>().unwrap();
    let vol_usds = batch.column(12).as_any().downcast_ref::<Float64Array>().unwrap();
    let ois = batch.column(13).as_any().downcast_ref::<Float64Array>().unwrap();
    let counts = batch.column(14).as_any().downcast_ref::<UInt32Array>().unwrap();
    let sources = batch.column(15).as_any().downcast_ref::<StringArray>().unwrap();
    let venues = batch.column(16).as_any().downcast_ref::<StringArray>().unwrap();

    for i in 0..num_rows {
        let nanos = timestamps.value(i);
        let dt = Utc.timestamp_nanos(nanos);
        let src_str = sources.value(i);
        let price_source = match src_str {
            "bid_ask_mid" => ProbabilityPriceSource::BidAskMid,
            "sampled_mid" => ProbabilityPriceSource::SampledMid,
            "sampled_last" => ProbabilityPriceSource::SampledLast,
            _ => ProbabilityPriceSource::Trade,
        };

        candles.push(PredictionMarketCandle {
            time: times.value(i),
            timestamp_utc: dt,
            market_id: market_ids.value(i).to_string(),
            outcome: outcomes.value(i).to_string(),
            timeframe: timeframes.value(i).to_string(),
            prob_open: if opens.is_null(i) { None } else { Some(opens.value(i)) },
            prob_high: if highs.is_null(i) { None } else { Some(highs.value(i)) },
            prob_low: if lows.is_null(i) { None } else { Some(lows.value(i)) },
            prob_close: if closes.is_null(i) { None } else { Some(closes.value(i)) },
            book_bid_close: if bids.is_null(i) { None } else { Some(bids.value(i)) },
            book_ask_close: if asks.is_null(i) { None } else { Some(asks.value(i)) },
            volume_contracts: if vol_contracts.is_null(i) { None } else { Some(vol_contracts.value(i)) },
            volume_usd: if vol_usds.is_null(i) { None } else { Some(vol_usds.value(i)) },
            open_interest_usd: if ois.is_null(i) { None } else { Some(ois.value(i)) },
            trade_count: if counts.is_null(i) { None } else { Some(counts.value(i)) },
            price_source,
            venue: venues.value(i).to_string(),
        });
    }

    Ok(candles)
}
