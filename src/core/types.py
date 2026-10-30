"""Core domain types for professional economic and financial data aggregator."""
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Any, Dict, Optional


class AssetClass(str, Enum):
    EQUITIES = "equities"
    CRYPTO = "crypto"
    FOREX = "forex"
    COMMODITIES = "commodities"
    MACRO = "macro"


class InstrumentType(str, Enum):
    SPOT = "spot"
    PERPETUAL = "perpetual"
    FUTURE = "future"
    INDEX = "index"
    INDICATOR = "indicator"


class TimeFrame(str, Enum):
    TICK = "tick"
    S1 = "1s"
    M1 = "1m"
    M5 = "5m"
    M15 = "15m"
    H1 = "1h"
    H4 = "4h"
    D1 = "1d"
    W1 = "1w"


class IngestionMode(str, Enum):
    LIVE_WS = "live_ws"
    REST_BATCH = "rest_batch"
    RECONCILIATION = "reconciliation"


@dataclass(frozen=True)
class InstrumentMapping:
    """Canonical symbology definition bridging multiple external vendors."""
    canonical_symbol: str  # e.g., "BTC/USDT", "EUR/USD", "AAPL"
    name: str
    asset_class: AssetClass
    instrument_type: InstrumentType
    base_currency: str
    quote_currency: str
    vendor_symbols: Dict[str, str]  # {"binance": "BTCUSDT", "yahoo": "BTC-USD"}
    contract_address: Optional[str] = None
    network: Optional[str] = None  # "ethereum", "solana", "native"
    figi: Optional[str] = None
    is_24_7: bool = True
    is_active: bool = True


@dataclass(frozen=True)
class AggregatorCandle:
    """Enterprise-grade OHLCV bar accommodating TradFi, Crypto, and Dual Ingestion."""
    timestamp_utc: datetime
    canonical_symbol: str
    timeframe: TimeFrame
    open: float
    high: float
    low: float
    close: float
    volume_base: float
    volume_quote: float
    source: str
    raw_symbol: str
    ingestion_mode: IngestionMode = IngestionMode.REST_BATCH
    is_settled: bool = True
    sequence_id: Optional[int] = None
    taker_buy_volume_base: Optional[float] = None
    taker_buy_volume_quote: Optional[float] = None
    vwap: Optional[float] = None
    trade_count: Optional[int] = None
    price_type: str = "last"  # "last", "mark", "index"
    ingested_at_utc: Optional[datetime] = None

    @property
    def natural_key(self) -> str:
        """Deterministic composite key for idempotency and deduplication."""
        ts_iso = self.timestamp_utc.isoformat()
        return f"{self.canonical_symbol}:{self.timeframe.value}:{ts_iso}:{self.source}:{self.price_type}"


@dataclass(frozen=True)
class EconomicEventRecord:
    """Standardized macroeconomic calendar event following SDMX/IMF/FRED."""
    event_id: str
    scheduled_utc: datetime
    country: str  # ISO-3166-1 alpha-2 ("US", "ID", "DE")
    currency: str  # ISO-4217 ("USD", "IDR", "EUR")
    category: str
    indicator_code: str  # e.g. "US_CPI_YOY", "US_NFP"
    title: str
    impact_level: int  # 1: Low, 2: Medium, 3: High
    source: str
    unit: str
    actual: Optional[float] = None
    forecast: Optional[float] = None
    previous: Optional[float] = None
    revised_previous: Optional[float] = None
    released_at_utc: Optional[datetime] = None
