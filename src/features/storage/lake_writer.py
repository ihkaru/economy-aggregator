"""Writes batches of market data into Hive-partitioned Parquet files with deduplication."""
from collections import defaultdict
from datetime import datetime, timezone
import json
from pathlib import Path
from typing import Dict, List
import uuid

from src.core.types import AggregatorCandle, AssetClass
from src.features.storage.partition_resolver import PartitionResolver


class LakeWriter:
    """Manages appending and writing partitioned Parquet chunks to the analytical lake."""

    def __init__(self, resolver: PartitionResolver, compression: str = "zstd"):
        self._resolver = resolver
        self._compression = compression

    def write_candles(
        self,
        candles: List[AggregatorCandle],
        asset_class: AssetClass = AssetClass.CRYPTO,
    ) -> List[Path]:
        """Groups candles by partition path, deduplicates, and writes individual chunk files."""
        if not candles:
            return []

        # Deduplicate within batch by natural_key (prioritizing settled records)
        deduped = self._deduplicate_batch(candles)

        partitioned: Dict[Path, List[AggregatorCandle]] = defaultdict(list)
        for candle in deduped:
            part_dir = self._resolver.resolve_candle_partition(
                asset_class=asset_class,
                canonical_symbol=candle.canonical_symbol,
                timeframe=candle.timeframe,
                timestamp_utc=candle.timestamp_utc,
            )
            partitioned[part_dir].append(candle)

        written_files = []
        for part_dir, group in partitioned.items():
            part_dir.mkdir(parents=True, exist_ok=True)
            written_files.append(self._write_partition_chunk(part_dir, group))

        return written_files

    def _deduplicate_batch(self, candles: List[AggregatorCandle]) -> List[AggregatorCandle]:
        """Ensures highest fidelity: settled records overwrite provisional ones."""
        seen: Dict[str, AggregatorCandle] = {}
        for c in candles:
            key = c.natural_key
            if key not in seen:
                seen[key] = c
            else:
                # If current is settled and existing is provisional, overwrite
                if c.is_settled and not seen[key].is_settled:
                    seen[key] = c
        return list(seen.values())

    def _write_partition_chunk(self, part_dir: Path, group: List[AggregatorCandle]) -> Path:
        """Serializes group records to a chunk parquet or metadata json file."""
        chunk_id = uuid.uuid4().hex[:8]
        ts_str = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S")
        chunk_file = part_dir / f"chunk_{ts_str}_{chunk_id}.parquet"

        try:
            import polars as pl
            records = [
                {
                    "timestamp": c.timestamp_utc.isoformat(),
                    "canonical_symbol": c.canonical_symbol,
                    "timeframe": c.timeframe.value,
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume_base": c.volume_base,
                    "volume_quote": c.volume_quote,
                    "taker_buy_volume_base": c.taker_buy_volume_base,
                    "taker_buy_volume_quote": c.taker_buy_volume_quote,
                    "source": c.source,
                    "raw_symbol": c.raw_symbol,
                    "ingestion_mode": c.ingestion_mode.value,
                    "is_settled": c.is_settled,
                    "sequence_id": c.sequence_id,
                    "vwap": c.vwap,
                    "trade_count": c.trade_count,
                    "price_type": c.price_type,
                }
                for c in group
            ]
            df = pl.DataFrame(records)
            df.write_parquet(chunk_file, compression=self._compression)
        except ImportError:
            meta_file = part_dir / f"chunk_{ts_str}_{chunk_id}.meta.json"
            meta_file.write_text(json.dumps([c.__dict__ for c in group], default=str))
            return meta_file

        return chunk_file
