pub mod kalshi;
pub mod polymarket;

pub use kalshi::{KalshiFetcher, KalshiRawEvent, KalshiRawMarket};
pub use polymarket::{PolymarketFetcher, PolymarketRawEvent, PolymarketRawMarket};

use std::fs;
use chrono::{Datelike, Utc};
use crate::core::prediction_types::PredictionMarketCandle;
use crate::features::storage::partition::PartitionResolver;
use crate::features::storage::prediction_arrow_schema::prediction_candles_to_record_batch;

/// Persists prediction market probability candles into partitioned Parquet lakehouse.
pub fn persist_prediction_candles_to_lake(
    candles: &[PredictionMarketCandle],
    resolver: &PartitionResolver,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    if candles.is_empty() {
        return Ok(0);
    }

    // Group by venue, market_id, timeframe, year, month
    let mut grouped: std::collections::HashMap<
        (String, String, String, i32, u32),
        Vec<&PredictionMarketCandle>,
    > = std::collections::HashMap::new();

    for candle in candles {
        let key = (
            candle.venue.clone(),
            candle.market_id.clone(),
            candle.timeframe.clone(),
            candle.timestamp_utc.year(),
            candle.timestamp_utc.month(),
        );
        grouped.entry(key).or_default().push(candle);
    }

    let mut total_saved = 0;
    for ((venue, market_id, timeframe, year, month), slice) in grouped {
        let dir = resolver.resolve_prediction_candles_partition(
            &venue,
            &market_id,
            &timeframe,
            year,
            month,
        );
        fs::create_dir_all(&dir)?;

        let batch = prediction_candles_to_record_batch(&slice)?;
        let file_path = dir.join(format!("candles_{}.parquet", Utc::now().timestamp_millis()));
        let file = fs::File::create(&file_path)?;

        let props = parquet::file::properties::WriterProperties::builder()
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();

        let mut writer = parquet::arrow::ArrowWriter::try_new(file, batch.schema(), Some(props))?;
        writer.write(&batch)?;
        writer.close()?;

        total_saved += slice.len();
    }

    Ok(total_saved)
}
