import type {
  AggregatorCandle,
  InstrumentSummary,
  InstrumentMetadata,
  RichEconomicEvent,
  PredictionMarketContract,
  PredictionMarketCandle,
  PartitionItem,
  IntegrityAuditResult,
  MacroPredictionDivergence,
} from './types';

const API_BASE = ''; // relative to serving host

export async function fetchInstruments(): Promise<InstrumentSummary[]> {
  const res = await fetch(`${API_BASE}/v1/market/catalog`);
  if (!res.ok) {
    throw new Error(`Failed to fetch catalog: HTTP ${res.status}`);
  }
  return await res.json();
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
  limit: number = 300,
  to?: number
): Promise<AggregatorCandle[]> {
  const params = new URLSearchParams({
    symbol,
    asset_class: assetClass,
    timeframe,
    limit: limit.toString(),
  });
  if (to !== undefined) {
    params.set('to', to.toString());
  }

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

export async function fetchStoragePartitions(): Promise<PartitionItem[]> {
  const res = await fetch(`${API_BASE}/v1/storage/partitions`);
  if (!res.ok) {
    throw new Error(`Failed to fetch lakehouse partitions: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchConsensusDivergence(): Promise<MacroPredictionDivergence[]> {
  const res = await fetch(`${API_BASE}/v1/analytics/divergence`);
  if (!res.ok) {
    throw new Error(`Failed to fetch consensus divergence: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchIntegrityAudit(): Promise<IntegrityAuditResult> {
  const res = await fetch(`${API_BASE}/v1/integrity/audit`);
  if (!res.ok) {
    throw new Error(`Failed to fetch integrity audit: HTTP ${res.status}`);
  }
  return await res.json();
}
