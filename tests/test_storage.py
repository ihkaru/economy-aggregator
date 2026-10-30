"""Hermetic unit tests for the storage slice with AggregatorCandle and deduplication."""
from datetime import datetime, timezone
from pathlib import Path
import tempfile
from typing import List
import unittest

from src.core.types import (
    AssetClass,
    AggregatorCandle,
    IngestionMode,
    TimeFrame,
)
from src.features.storage.partition_resolver import PartitionResolver
from src.features.storage.micro_batch_buffer import MicroBatchBuffer
from src.features.storage.lake_writer import LakeWriter


class TestStorageSlice(unittest.TestCase):
    def setUp(self):
        self._temp_dir = tempfile.TemporaryDirectory()
        self.tmp_path = Path(self._temp_dir.name)

    def tearDown(self):
        self._temp_dir.cleanup()

    def _create_sample_candle(
        self,
        symbol: str = "BTC/USDT",
        dt: datetime = None,
        is_settled: bool = True,
        source: str = "binance",
    ) -> AggregatorCandle:
        if dt is None:
            dt = datetime(2026, 10, 30, 14, 0, 0, tzinfo=timezone.utc)
        return AggregatorCandle(
            timestamp_utc=dt,
            canonical_symbol=symbol,
            timeframe=TimeFrame.M1,
            open=100.0,
            high=105.0,
            low=99.0,
            close=104.0,
            volume_base=10.0,
            volume_quote=1040.0,
            source=source,
            raw_symbol=symbol.replace("/", ""),
            ingestion_mode=IngestionMode.LIVE_WS if not is_settled else IngestionMode.REST_BATCH,
            is_settled=is_settled,
        )

    def test_partition_resolver_deterministic_paths(self):
        resolver = PartitionResolver(base_lake_dir=self.tmp_path)
        dt = datetime(2026, 10, 30, 8, 30, 0, tzinfo=timezone.utc)

        path = resolver.resolve_candle_partition(
            asset_class=AssetClass.CRYPTO,
            canonical_symbol="BTC/USDT",
            timeframe=TimeFrame.M1,
            timestamp_utc=dt,
        )

        expected = (
            self.tmp_path
            / "market"
            / "asset_class=crypto"
            / "symbol=BTC_USDT"
            / "timeframe=1m"
            / "year=2026"
            / "month=10"
        )
        self.assertEqual(path, expected)

    def test_micro_batch_buffer_flushes_on_threshold(self):
        flushed_records: List[AggregatorCandle] = []

        def on_flush(items: List[AggregatorCandle]):
            flushed_records.extend(items)

        buffer = MicroBatchBuffer(flush_callback=on_flush, max_rows=3, flush_seconds=60.0)

        self.assertFalse(buffer.add(self._create_sample_candle()))
        self.assertFalse(buffer.add(self._create_sample_candle()))
        self.assertEqual(buffer.pending_count, 2)
        self.assertEqual(len(flushed_records), 0)

        # 3rd item triggers threshold flush
        self.assertTrue(buffer.add(self._create_sample_candle()))
        self.assertEqual(buffer.pending_count, 0)
        self.assertEqual(len(flushed_records), 3)

    def test_lake_writer_deduplication_settled_overwrites_provisional(self):
        resolver = PartitionResolver(base_lake_dir=self.tmp_path)
        writer = LakeWriter(resolver=resolver)

        dt = datetime(2026, 10, 30, 10, 0, tzinfo=timezone.utc)
        provisional = self._create_sample_candle("BTC/USDT", dt, is_settled=False)
        settled = self._create_sample_candle("BTC/USDT", dt, is_settled=True)

        # Both present in batch: settled should win
        deduped = writer._deduplicate_batch([provisional, settled])
        self.assertEqual(len(deduped), 1)
        self.assertTrue(deduped[0].is_settled)

    def test_lake_writer_writes_partitions(self):
        resolver = PartitionResolver(base_lake_dir=self.tmp_path)
        writer = LakeWriter(resolver=resolver)

        c1 = self._create_sample_candle("BTC/USDT", datetime(2026, 10, 30, 10, 0, tzinfo=timezone.utc))
        c2 = self._create_sample_candle("ETH/USDT", datetime(2026, 10, 30, 10, 0, tzinfo=timezone.utc))

        files = writer.write_candles([c1, c2], asset_class=AssetClass.CRYPTO)
        self.assertEqual(len(files), 2)
        for f in files:
            self.assertTrue(f.exists())


if __name__ == "__main__":
    unittest.main()
