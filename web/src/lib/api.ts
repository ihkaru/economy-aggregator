import type { AggregatorCandle, InstrumentSummary } from './types';

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
    { symbol: 'BBCA', canonical_symbol: 'BBCA', name: 'Bank Central Asia', asset_class: 'equities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'GSPC', canonical_symbol: 'GSPC', name: 'S&P 500 Index', asset_class: 'equities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'JKSE', canonical_symbol: 'JKSE', name: 'Jakarta Composite Index', asset_class: 'equities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'N225', canonical_symbol: 'N225', name: 'Nikkei 225 Index', asset_class: 'equities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'TLT', canonical_symbol: 'TLT', name: 'iShares 20+ Year Treasury Bond', asset_class: 'equities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'IEF', canonical_symbol: 'IEF', name: 'iShares 7-10 Year Treasury Bond', asset_class: 'equities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'LQD', canonical_symbol: 'LQD', name: 'iShares Investment Grade Corporate Bond', asset_class: 'equities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'XAU_USD', canonical_symbol: 'XAU/USD', name: 'Gold Spot / US Dollar', asset_class: 'commodities', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'USD_IDR', canonical_symbol: 'USD/IDR', name: 'US Dollar / Indonesian Rupiah', asset_class: 'forex', source: 'yahoo', timeframes: ['1d'] },
    { symbol: 'BTC_USDT', canonical_symbol: 'BTC/USDT', name: 'Bitcoin / Tether USD', asset_class: 'crypto', source: 'binance', timeframes: ['1d', '1m'] },
  ];
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
