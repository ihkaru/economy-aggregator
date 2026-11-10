export type AssetClass = 'equities' | 'crypto' | 'commodities' | 'forex' | 'macro' | 'prediction_market';

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

export interface SourceAttribution {
  provider: string;
  original_id: string;
  source_url?: string | null;
  agency_name?: string | null;
  agency_url?: string | null;
  observed_at_utc: string;
}

export interface RichEconomicEvent {
  event_id: string;
  scheduled_utc: string;
  country: string;
  currency: string;
  category: string;
  indicator_code: string;
  title: string;
  impact_tier: number; // 0: None, 1: Low, 2: Medium, 3: High
  is_all_day: boolean;
  is_tentative: boolean;
  raw_actual?: string | null;
  raw_forecast?: string | null;
  raw_previous?: string | null;
  normalized_actual?: number | null;
  normalized_forecast?: number | null;
  normalized_previous?: number | null;
  unit_type: string;
  unit_multiplier: number;
  unit_symbol: string;
  better_than_expected?: boolean | null;
  attribution: SourceAttribution;
  revision_count: number;
}

export interface ResolutionOracleSpec {
  oracle_type: string;
  source_name: string;
  source_url: string;
  resolution_criteria: string;
}

export interface PredictionMarketContract {
  market_id: string;
  venue: string;
  venue_market_id: string;
  parent_event_id: string;
  title: string;
  outcome_label: string;
  status: string;
  is_active: boolean;
  market_type: string;
  book_model: string;
  outcomes: string[];
  winning_outcome?: string | null;
  face_value: number;
  tick_size: number;
  close_time_utc: string;
  settlement_time_utc?: string | null;
  linked_macro_indicator?: string | null;
  oracle: ResolutionOracleSpec;
  venue_url?: string | null;
  implied_probability?: number | null;
  last_price?: number | null;
  best_bid?: number | null;
  best_ask?: number | null;
  volume_usd?: number | null;
  open_interest_usd?: number | null;
}

export interface PredictionMarketCandle {
  time: number;
  timestamp_utc: string;
  market_id: string;
  outcome: string;
  timeframe: string;
  prob_open?: number | null;
  prob_high?: number | null;
  prob_low?: number | null;
  prob_close?: number | null;
  book_bid_close?: number | null;
  book_ask_close?: number | null;
  volume_contracts?: number | null;
  volume_usd?: number | null;
  open_interest_usd?: number | null;
  trade_count?: number | null;
  price_source: string;
  venue: string;
}

export type AppViewId = 'terminal' | 'macro' | 'predictions' | 'consensus' | 'lakehouse' | 'integrity' | 'subscriptions';

export interface SubscriptionItem {
  source: string;
  symbol: string;
  asset_class: string;
  timeframes: string[];
  enabled: boolean;
  poll_interval_secs?: number | null;
  created_at_utc: string;
}

export interface OrchestratorStatus {
  is_running: boolean;
  boot_timestamp_utc: string;
  last_catch_up_utc: string | null;
  total_candles_ingested_session: number;
  active_subscriptions_count: number;
  active_streams: string[];
  active_pollers: string[];
}

export interface DiscoverySearchResult {
  source: string;
  raw_symbol: string;
  canonical_symbol: string;
  name: string;
  asset_class: string;
  exchange?: string | null;
}

export interface PartitionItem {
  domain: string;
  identifier: string;
  timeframe: string;
  year: number;
  month: number;
  file_name: string;
  size_bytes: number;
  row_count: number;
}

export interface IntegrityAuditResult {
  passed: boolean;
  checked_instruments_count: number;
  checked_candles_count: number;
  inverted_high_low_violations: number;
  negative_price_violations: number;
  volume_violations: number;
  temporal_ordering_violations: number;
  hampel_outliers_flagged: number;
  cross_source_drift_pct: number;
  status_summary: string;
  audit_timestamp_utc: string;
}

export interface MacroPredictionDivergence {
  indicator_code: string;
  title: string;
  country: string;
  scheduled_utc: string;
  economist_forecast?: number | null;
  previous_value?: number | null;
  unit_symbol: string;
  linked_markets_count: number;
  average_implied_prob_yes?: number | null;
  venues_represented: string[];
  actual_released_value?: number | null;
  surprise_direction?: string | null;
}
