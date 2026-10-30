"""Partition path resolution for Hive-style columnar storage."""
from datetime import datetime, timezone
from pathlib import Path
from src.core.types import AssetClass, TimeFrame


class PartitionResolver:
    """Computes deterministic Hive partition paths for the analytical lake."""

    def __init__(self, base_lake_dir: Path):
        self._base = base_lake_dir

    def resolve_candle_partition(
        self,
        asset_class: AssetClass,
        symbol: str,
        timeframe: TimeFrame,
        timestamp_utc: datetime,
    ) -> Path:
        """Returns the partition directory:

        base/asset_class=<val>/symbol=<val>/timeframe=<val>/year=<YYYY>/month=<MM>
        """
        clean_symbol = symbol.upper().replace("/", "_").replace(":", "_")
        year_str = timestamp_utc.strftime("%Y")
        month_str = timestamp_utc.strftime("%m")

        return (
            self._base
            / f"asset_class={asset_class.value}"
            / f"symbol={clean_symbol}"
            / f"timeframe={timeframe.value}"
            / f"year={year_str}"
            / f"month={month_str}"
        )

    def resolve_glob_pattern(
        self,
        asset_class: AssetClass,
        symbol: str,
        timeframe: TimeFrame,
        year: str = "*",
        month: str = "*",
    ) -> str:
        """Returns a glob string for lazy partition scanning."""
        clean_symbol = symbol.upper().replace("/", "_").replace(":", "_")
        target_dir = (
            self._base
            / f"asset_class={asset_class.value}"
            / f"symbol={clean_symbol}"
            / f"timeframe={timeframe.value}"
            / f"year={year}"
            / f"month={month}"
        )
        return str(target_dir / "*.parquet")
