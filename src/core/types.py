"""Core domain types for economy and financial analysis platform."""
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Optional


class AssetClass(str, Enum):
    EQUITIES = "equities"
    CRYPTO = "crypto"
    FOREX = "forex"
    COMMODITIES = "commodities"
    MACRO = "macro"


class TimeFrame(str, Enum):
    TICK = "tick"
    M1 = "1m"
    M5 = "5m"
    M15 = "15m"
    H1 = "1h"
    H4 = "4h"
    D1 = "1d"
    W1 = "1w"


@dataclass(frozen=True)
class MarketTick:
    """Represents a single granular market trade or quote event."""
    symbol: str
    asset_class: AssetClass
    timestamp_ns: int  # Epoch nanoseconds (UTC)
    price: float
    volume: float
    side: Optional[str] = None  # "buy" or "sell"


@dataclass(frozen=True)
class OHLCVCandle:
    """Standardized candlestick aggregate."""
    symbol: str
    asset_class: AssetClass
    timeframe: TimeFrame
    timestamp_utc: datetime
    open: float
    high: float
    low: float
    close: float
    volume: float
    trade_count: int = 0
