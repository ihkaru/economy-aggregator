use chrono::{TimeZone, Utc};
use tempfile::tempdir;

use economy_aggregator::core::types::{
    AggregatorCandle, AssetClass, IngestionMode, TimeFrame,
};
use economy_aggregator::features::storage::{
    LakeWriter, MicroBatchBuffer, PartitionResolver,
};

fn create_sample_candle(
    symbol: &str,
    is_settled: bool,
    source: &str,
) -> AggregatorCandle {
    AggregatorCandle {
        timestamp_utc: Utc.with_ymd_and_hms(2026, 10, 30, 14, 0, 0).unwrap(),
        canonical_symbol: symbol.to_string(),
        timeframe: TimeFrame::M1,
        open: 100.0,
        high: 105.0,
        low: 99.0,
        close: 104.0,
        volume_base: 10.0,
        volume_quote: 1040.0,
        source: source.to_string(),
        raw_symbol: symbol.replace('/', ""),
        ingestion_mode: if is_settled {
            IngestionMode::RestBatch
        } else {
            IngestionMode::LiveWs
        },
        is_settled,
        sequence_id: Some(101),
        taker_buy_volume_base: Some(6.0),
        taker_buy_volume_quote: Some(624.0),
        vwap: Some(103.5),
        trade_count: Some(50),
        price_type: "last".to_string(),
    }
}

#[test]
fn test_partition_resolver_path() {
    let tmp = tempdir().unwrap();
    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let dt = Utc.with_ymd_and_hms(2026, 10, 30, 8, 30, 0).unwrap();

    let path = resolver.resolve_candle_partition(
        AssetClass::Crypto,
        "BTC/USDT",
        TimeFrame::M1,
        dt,
    );

    let expected = tmp
        .path()
        .join("market")
        .join("asset_class=crypto")
        .join("symbol=BTC_USDT")
        .join("timeframe=1m")
        .join("year=2026")
        .join("month=10");
    assert_eq!(path, expected);
}

#[test]
fn test_micro_batch_buffer_threshold() {
    let buffer = MicroBatchBuffer::new(60, 3);

    assert!(buffer.add(create_sample_candle("BTC/USDT", false, "binance")).is_none());
    assert!(buffer.add(create_sample_candle("BTC/USDT", false, "binance")).is_none());
    assert_eq!(buffer.pending_count(), 2);

    // 3rd item triggers flush
    let flushed = buffer.add(create_sample_candle("BTC/USDT", false, "binance"));
    assert!(flushed.is_some());
    assert_eq!(flushed.unwrap().len(), 3);
    assert_eq!(buffer.pending_count(), 0);
}

#[test]
fn test_lake_writer_deduplication_settled_precedence() {
    let tmp = tempdir().unwrap();
    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let writer = LakeWriter::new(resolver);

    let provisional = create_sample_candle("BTC/USDT", false, "binance");
    let settled = create_sample_candle("BTC/USDT", true, "binance");

    let deduped = writer.deduplicate(&[provisional, settled]);
    assert_eq!(deduped.len(), 1);
    assert!(deduped[0].is_settled);
}

#[test]
fn test_lake_writer_writes_files() {
    let tmp = tempdir().unwrap();
    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let writer = LakeWriter::new(resolver);

    let c1 = create_sample_candle("BTC/USDT", true, "binance");
    let c2 = create_sample_candle("ETH/USDT", true, "binance");

    let files = writer.write_candles(&[c1, c2], AssetClass::Crypto).unwrap();
    assert_eq!(files.len(), 2);
    for f in files {
        assert!(f.exists());
    }
}
