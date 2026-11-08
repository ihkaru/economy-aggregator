export type AssetClass = 'equities' | 'crypto' | 'commodities' | 'forex' | 'macro';

export type TimeframeOption = '1m' | '5m' | '15m' | '1h' | '1d' | '1w' | '1M';

export interface AggregatorCandle {
  time: number; // Numeric UTC timestamp in seconds (Unix epoch)
  timestamp_utc: string; // ISO 8601 UTC string
  canonical_symbol: string;
  timeframe: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume_base: number;
  volume_quote: number;
  source: string;
  raw_symbol: string;
  ingestion_mode: string;
  is_settled: boolean;
  vwap?: number | null;
  trade_count?: number | null;
  price_type: string;
}

export interface InstrumentSummary {
  symbol: string;
  canonical_symbol: string;
  name: string;
  asset_class: AssetClass;
  currency?: string;
  price_precision?: number;
  min_tick?: number;
  supported_timeframes?: string[];
  default_timeframe?: string;
  source: string;
  timeframes?: string[];
  candle_count?: number;
  last_price?: number;
  last_updated?: string;
}

export interface InstrumentMetadata {
  symbol: string;
  canonical_symbol: string;
  name: string;
  asset_class: AssetClass;
  currency: string;
  price_precision: number;
  min_tick: number;
  supported_timeframes: TimeframeOption[];
  default_timeframe: TimeframeOption;
  has_intraday: boolean;
  has_daily: boolean;
  has_weekly_and_monthly: boolean;
  total_bars: number;
  first_date?: string | null;
  last_date?: string | null;
}

export interface ChartLegendData {
  time: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  change: number;
  changePercent: number;
  isUp: boolean;
}
