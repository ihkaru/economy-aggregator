use std::fs::File;
use chrono::{TimeZone, Utc};
use parquet::file::reader::{FileReader, SerializedFileReader};
use tempfile::tempdir;

use economy_aggregator::core::types::{
    AggregatorCandle, AssetClass, IngestionMode, TimeFrame,
};
use economy_aggregator::features::storage::{
    candle_arrow_schema, candles_to_record_batch,
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
fn test_arrow_record_batch_generation() {
    let c = create_sample_candle("BTC/USDT", true, "binance");
    let batch = candles_to_record_batch(&[&c]).unwrap();
    let schema = candle_arrow_schema();

    assert_eq!(batch.num_rows(), 1);
    assert_eq!(batch.num_columns(), schema.fields().len());
}

#[test]
fn test_micro_batch_buffer_threshold() {
    let buffer = MicroBatchBuffer::new(60, 3);

    assert!(buffer.add(create_sample_candle("BTC/USDT", false, "binance")).is_none());
    assert!(buffer.add(create_sample_candle("BTC/USDT", false, "binance")).is_none());
    assert_eq!(buffer.pending_count(), 2);

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

    let batch = vec![provisional, settled];
    let deduped = writer.deduplicate(&batch);
    assert_eq!(deduped.len(), 1);
    assert!(deduped[0].is_settled);
}

#[test]
fn test_lake_writer_writes_real_parquet_files() {
    let tmp = tempdir().unwrap();
    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let writer = LakeWriter::new(resolver);

    let c1 = create_sample_candle("BTC/USDT", true, "binance");
    let c2 = create_sample_candle("ETH/USDT", true, "binance");

    let files = writer.write_candles(&[c1, c2], AssetClass::Crypto).unwrap();
    assert_eq!(files.len(), 2);

    for f in &files {
        assert!(f.exists());
        assert_eq!(f.extension().unwrap(), "parquet");

        // Verify with actual Parquet reader
        let file = File::open(f).unwrap();
        let reader = SerializedFileReader::new(file).unwrap();
        assert_eq!(reader.metadata().file_metadata().num_rows(), 1);
    }
}

#[test]
fn test_symbology_registry_and_persistence() {
    use std::collections::HashMap;
    use economy_aggregator::core::types::InstrumentType;
    use economy_aggregator::features::storage::{InstrumentMapping, InstrumentRegistry};

    let tmp = tempdir().unwrap();
    let meta_file = tmp.path().join("meta").join("instruments.json");

    let mut registry = InstrumentRegistry::new();
    let mut vendors = HashMap::new();
    vendors.insert("binance".to_string(), "BTCUSDT".to_string());
    vendors.insert("yahoo".to_string(), "BTC-USD".to_string());

    registry.register(InstrumentMapping {
        canonical_symbol: "BTC/USDT".to_string(),
        name: "Bitcoin / Tether".to_string(),
        asset_class: AssetClass::Crypto,
        instrument_type: InstrumentType::Spot,
        base_currency: "BTC".to_string(),
        quote_currency: "USDT".to_string(),
        market_timezone: "UTC".to_string(),
        is_24_7: true,
        vendor_symbols: vendors,
        contract_address: None,
        network: None,
    });

    assert_eq!(registry.resolve_vendor("binance", "btcusdt"), Some("BTC/USDT"));
    assert_eq!(registry.resolve_vendor("yahoo", "btc-usd"), Some("BTC/USDT"));
    assert_eq!(registry.resolve_vendor("dukascopy", "btc/usd"), None);

    // Test persistence
    registry.save_to_file(&meta_file).unwrap();
    assert!(meta_file.exists());

    let mut loaded = InstrumentRegistry::new();
    loaded.load_from_file(&meta_file).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.resolve_vendor("binance", "BTCUSDT"), Some("BTC/USDT"));
}

#[test]
fn test_lake_compactor_merges_chunks_and_deduplicates() {
    use economy_aggregator::features::storage::LakeCompactor;

    let tmp = tempdir().unwrap();
    let resolver = PartitionResolver::new(tmp.path().to_path_buf());
    let writer = LakeWriter::new(resolver.clone());
    let compactor = LakeCompactor::new();

    // Create 2 chunks: one provisional candle, then another settled candle with the same key
    let dt = Utc.with_ymd_and_hms(2026, 10, 30, 14, 0, 0).unwrap();
    let mut provisional = create_sample_candle("BTC/USDT", false, "binance");
    provisional.timestamp_utc = dt;
    provisional.close = 100.0;

    let mut settled = create_sample_candle("BTC/USDT", true, "binance");
    settled.timestamp_utc = dt;
    settled.close = 105.0; // Settled final price

    let dt2 = Utc.with_ymd_and_hms(2026, 10, 30, 14, 1, 0).unwrap();
    let mut candle2 = create_sample_candle("BTC/USDT", true, "binance");
    candle2.timestamp_utc = dt2;

    // Write chunk 1
    let files1 = writer.write_candles(&[provisional], AssetClass::Crypto).unwrap();
    assert_eq!(files1.len(), 1);

    // Write chunk 2
    let files2 = writer.write_candles(&[settled, candle2], AssetClass::Crypto).unwrap();
    assert_eq!(files2.len(), 1);

    let part_dir = resolver.resolve_candle_partition(
        AssetClass::Crypto,
        "BTC/USDT",
        TimeFrame::M1,
        dt,
    );

    // Perform compaction
    let result = compactor.compact_partition(&part_dir, 2).unwrap();
    assert_eq!(result.files_compacted, 2);
    assert_eq!(result.total_rows_before, 3);
    assert_eq!(result.total_rows_after, 2); // 1 duplicate was merged (settled won)

    let compacted_file = result.compacted_file.expect("compacted file exists");
    assert!(compacted_file.exists());

    // Verify old micro-chunks were removed
    assert!(!files1[0].exists());
    assert!(!files2[0].exists());

    // Verify data inside compacted Parquet file
    let file = File::open(&compacted_file).unwrap();
    let reader = SerializedFileReader::new(file).unwrap();
    assert_eq!(reader.metadata().file_metadata().num_rows(), 2);
}
