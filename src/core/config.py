"""Storage and runtime configuration."""
import os
from pathlib import Path
from dataclasses import dataclass


@dataclass(frozen=True)
class LakehouseConfig:
    """Configuration for local columnar lakehouse storage."""
    base_dir: Path = Path("data")
    lake_dir: Path = Path("data/lake")
    hot_buffer_dir: Path = Path("data/hot_buffer")
    micro_batch_flush_seconds: float = 5.0
    micro_batch_max_rows: int = 10_000
    row_group_size: int = 32_768
    compression: str = "zstd"

    def ensure_directories(self) -> None:
        self.lake_dir.mkdir(parents=True, exist_ok=True)
        self.hot_buffer_dir.mkdir(parents=True, exist_ok=True)


def get_default_config() -> LakehouseConfig:
    base = Path(os.getenv("ECONOMY_DATA_DIR", "data"))
    return LakehouseConfig(
        base_dir=base,
        lake_dir=base / "lake",
        hot_buffer_dir=base / "hot_buffer",
    )
