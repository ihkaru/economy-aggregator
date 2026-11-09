import type {
  AggregatorCandle,
  InstrumentSummary,
  InstrumentMetadata,
  RichEconomicEvent,
  PredictionMarketContract,
  PredictionMarketCandle,
} from './types';

const API_BASE = ''; // relative to serving host

export async function fetchInstruments(): Promise<InstrumentSummary[]> {
  try {
    const res = await fetch(`${API_BASE}/v1/market/catalog`);
    if (res.ok) {
      return await res.json();
    }
  } catch {
    // fallback if catalog endpoint is not available
  }

  // Fallback to core instruments detected in lakehouse
  return [
    { symbol: 'BBCA', canonical_symbol: 'BBCA', name: 'Bank Central Asia', asset_class: 'equities', currency: 'IDR', price_precision: 0, min_tick: 25.0, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'GSPC', canonical_symbol: 'GSPC', name: 'S&P 500 Index', asset_class: 'equities', currency: 'USD', price_precision: 2, min_tick: 0.01, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'JKSE', canonical_symbol: 'JKSE', name: 'Jakarta Composite Index', asset_class: 'equities', currency: 'IDR', price_precision: 2, min_tick: 0.01, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'N225', canonical_symbol: 'N225', name: 'Nikkei 225 Index', asset_class: 'equities', currency: 'JPY', price_precision: 2, min_tick: 0.01, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'TLT', canonical_symbol: 'TLT', name: 'iShares 20+ Year Treasury Bond', asset_class: 'equities', currency: 'USD', price_precision: 2, min_tick: 0.01, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'IEF', canonical_symbol: 'IEF', name: 'iShares 7-10 Year Treasury Bond', asset_class: 'equities', currency: 'USD', price_precision: 2, min_tick: 0.01, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'LQD', canonical_symbol: 'LQD', name: 'iShares Investment Grade Corporate Bond', asset_class: 'equities', currency: 'USD', price_precision: 2, min_tick: 0.01, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'XAU_USD', canonical_symbol: 'XAU/USD', name: 'Gold Spot / US Dollar', asset_class: 'commodities', currency: 'USD', price_precision: 2, min_tick: 0.05, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'USD_IDR', canonical_symbol: 'USD/IDR', name: 'US Dollar / Indonesian Rupiah', asset_class: 'forex', currency: 'IDR', price_precision: 2, min_tick: 0.5, supported_timeframes: ['1d', '1w', '1M'], timeframes: ['1d', '1w', '1M'], default_timeframe: '1d', source: 'yahoo' },
    { symbol: 'BTC_USDT', canonical_symbol: 'BTC/USDT', name: 'Bitcoin / Tether USD', asset_class: 'crypto', currency: 'USDT', price_precision: 2, min_tick: 0.01, supported_timeframes: ['1m', '5m', '15m', '1h', '1d', '1w', '1M'], timeframes: ['1m', '5m', '15m', '1h', '1d', '1w', '1M'], default_timeframe: '1d', source: 'binance' },
  ];
}

export async function fetchMetadata(symbol: string, assetClass?: string): Promise<InstrumentMetadata> {
  const params = new URLSearchParams({ symbol });
  if (assetClass) params.set('asset_class', assetClass);

  const res = await fetch(`${API_BASE}/v1/market/metadata?${params.toString()}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch metadata: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchCandles(
  symbol: string,
  assetClass: string,
  timeframe: string = '1d',
  limit: number = 2000
): Promise<AggregatorCandle[]> {
  const params = new URLSearchParams({
    symbol,
    asset_class: assetClass,
    timeframe,
    limit: limit.toString(),
  });

  const res = await fetch(`${API_BASE}/v1/market/candles?${params.toString()}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch candles: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchMacroCalendar(
  country: string = 'US',
  importance?: string,
  limit: number = 50
): Promise<RichEconomicEvent[]> {
  const params = new URLSearchParams({ country, limit: limit.toString() });
  if (importance) params.set('importance', importance);

  const res = await fetch(`${API_BASE}/v1/macro/calendar?${params.toString()}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch macro calendar: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchPredictionMarkets(
  venue?: string,
  limit: number = 50
): Promise<PredictionMarketContract[]> {
  const params = new URLSearchParams({ limit: limit.toString() });
  if (venue) params.set('venue', venue);

  const res = await fetch(`${API_BASE}/v1/prediction-markets/events?${params.toString()}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch prediction markets: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchPredictionCandles(
  marketId: string,
  timeframe: string = '1m',
  limit: number = 500
): Promise<PredictionMarketCandle[]> {
  const params = new URLSearchParams({ timeframe, limit: limit.toString() });
  const res = await fetch(
    `${API_BASE}/v1/prediction-markets/markets/${encodeURIComponent(marketId)}/candles?${params.toString()}`
  );
  if (!res.ok) {
    throw new Error(`Failed to fetch prediction candles: HTTP ${res.status}`);
  }
  return await res.json();
}
