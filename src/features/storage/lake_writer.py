"""Writes batches of market data into Hive-partitioned Parquet files."""
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List
import uuid

from src.core.types import OHLCVCandle
from src.features.storage.partition_resolver import PartitionResolver


class LakeWriter:
    """Manages appending and writing partitioned Parquet chunks to the analytical lake."""

    def __init__(self, resolver: PartitionResolver, compression: str = "zstd"):
        self._resolver = resolver
        self._compression = compression

    def write_candles(self, candles: List[OHLCVCandle]) -> List[Path]:
        """Groups candles by partition path and writes individual timestamped chunks."""
        if not candles:
            return []

        partitioned: Dict[Path, List[OHLCVCandle]] = defaultdict(list)
        for candle in candles:
            part_dir = self._resolver.resolve_candle_partition(
                asset_class=candle.asset_class,
                symbol=candle.symbol,
                timeframe=candle.timeframe,
                timestamp_utc=candle.timestamp_utc,
            )
            partitioned[part_dir].append(candle)

        written_files = []
        for part_dir, group in partitioned.items():
            part_dir.mkdir(parents=True, exist_ok=True)
            written_files.append(self._write_partition_chunk(part_dir, group))

        return written_files

    def _write_partition_chunk(self, part_dir: Path, group: List[OHLCVCandle]) -> Path:
        """Serializes group records to a chunk parquet file."""
        chunk_id = uuid.uuid4().hex[:8]
        ts_str = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S")
        chunk_file = part_dir / f"chunk_{ts_str}_{chunk_id}.parquet"

        try:
            import polars as pl
            records = [
                {
                    "timestamp": c.timestamp_utc.isoformat(),
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume,
                    "trade_count": c.trade_count,
                }
                for c in group
            ]
            df = pl.DataFrame(records)
            df.write_parquet(chunk_file, compression=self._compression)
        except ImportError:
            # Metadata stub writer when polars is not yet compiled on environment
            meta_file = part_dir / f"chunk_{ts_str}_{chunk_id}.meta.json"
            import json
            meta_file.write_text(json.dumps([c.__dict__ for c in group], default=str))
            return meta_file

        return chunk_file
