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
  SubscriptionItem,
  OrchestratorStatus,
  DiscoverySearchResult,
} from './types';

const API_BASE = typeof window !== 'undefined' ? '' : (typeof process !== 'undefined' && process.env?.API_BASE ? process.env.API_BASE : 'http://127.0.0.1:8080');

export interface AuthSessionState {
  authenticated: boolean;
  auth_required: boolean;
  csrf_token?: string | null;
  expires_at?: number | null;
  user?: { sub: string; name?: string | null; auth_method: string } | null;
}

// Held strictly in JS memory (never localStorage/sessionStorage) per OWASP SPA guidelines.
let memoryCsrfToken: string | null = null;

export function setMemoryCsrfToken(token: string | null): void {
  memoryCsrfToken = token;
}

export async function fetchAuthSession(): Promise<AuthSessionState> {
  const res = await fetch(`${API_BASE}/v1/auth/session`, {
    credentials: 'same-origin',
  });
  if (!res.ok) {
    throw new Error(`Session check failed: HTTP ${res.status}`);
  }
  const data: AuthSessionState = await res.json();
  memoryCsrfToken = data.csrf_token || null;
  return data;
}

export async function loginWithAccount(username: string, password: string): Promise<AuthSessionState> {
  const res = await fetch(`${API_BASE}/v1/auth/login`, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    throw new Error(body.error || `Authentication failed: HTTP ${res.status}`);
  }
  memoryCsrfToken = body.csrf_token || null;
  return await fetchAuthSession();
}

export async function logoutSession(): Promise<void> {
  const headers = new Headers();
  if (memoryCsrfToken) {
    headers.set('X-CSRF-Token', memoryCsrfToken);
  }
  await fetch(`${API_BASE}/v1/auth/logout`, {
    method: 'POST',
    credentials: 'same-origin',
    headers,
  });
  memoryCsrfToken = null;
}

async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const headers = new Headers(init?.headers || {});
  const method = (init?.method || 'GET').toUpperCase();
  if (memoryCsrfToken && ['POST', 'PUT', 'DELETE', 'PATCH'].includes(method)) {
    headers.set('X-CSRF-Token', memoryCsrfToken);
  }
  const res = await fetch(input, {
    ...init,
    credentials: 'same-origin',
    headers,
  });
  if (res.status === 401 && typeof window !== 'undefined') {
    memoryCsrfToken = null;
    window.dispatchEvent(new CustomEvent('ea:unauthorized'));
  }
  return res;
}

export async function fetchInstruments(): Promise<InstrumentSummary[]> {
  try {
    const res = await apiFetch(`${API_BASE}/v1/market/catalog`);
    if (!res.ok) {
      throw new Error(`Failed to fetch catalog: HTTP ${res.status}`);
    }
    const data = await res.json();
    return data.map((item: any) => ({
      ...item,
      timeframes: item.supported_timeframes || item.timeframes || ['1d'],
    }));
  } catch (err) {
    // Resilient fallback for initial boot or network partition
    return [
      {
        symbol: 'BBCA',
        canonical_symbol: 'BBCA',
        name: 'Bank Central Asia',
        asset_class: 'equities',
        source: 'yahoo',
        timeframes: ['1d'],
      },
      {
        symbol: 'GSPC',
        canonical_symbol: 'GSPC',
        name: 'S&P 500 Index',
        asset_class: 'equities',
        source: 'yahoo',
        timeframes: ['1d'],
      },
      {
        symbol: 'BTC_USDT',
        canonical_symbol: 'BTC/USDT',
        name: 'Bitcoin / Tether USD',
        asset_class: 'crypto',
        source: 'binance',
        timeframes: ['1d', '1h', '15m', '1m'],
      },
    ];
  }
}

export async function fetchMetadata(symbol: string, assetClass?: string): Promise<InstrumentMetadata> {
  const params = new URLSearchParams({ symbol });
  if (assetClass) params.set('asset_class', assetClass);

  const res = await apiFetch(`${API_BASE}/v1/market/metadata?${params.toString()}`);
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

  const res = await apiFetch(`${API_BASE}/v1/market/candles?${params.toString()}`);
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

  const res = await apiFetch(`${API_BASE}/v1/macro/calendar?${params.toString()}`);
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

  const res = await apiFetch(`${API_BASE}/v1/prediction-markets/events?${params.toString()}`);
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
  const res = await apiFetch(
    `${API_BASE}/v1/prediction-markets/markets/${encodeURIComponent(marketId)}/candles?${params.toString()}`
  );
  if (!res.ok) {
    throw new Error(`Failed to fetch prediction candles: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchStoragePartitions(): Promise<PartitionItem[]> {
  const res = await apiFetch(`${API_BASE}/v1/storage/partitions`);
  if (!res.ok) {
    throw new Error(`Failed to fetch lakehouse partitions: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchConsensusDivergence(): Promise<MacroPredictionDivergence[]> {
  const res = await apiFetch(`${API_BASE}/v1/analytics/divergence`);
  if (!res.ok) {
    throw new Error(`Failed to fetch consensus divergence: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchIntegrityAudit(): Promise<IntegrityAuditResult> {
  const res = await apiFetch(`${API_BASE}/v1/integrity/audit`);
  if (!res.ok) {
    throw new Error(`Failed to fetch integrity audit: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchSubscriptions(): Promise<SubscriptionItem[]> {
  const res = await apiFetch(`${API_BASE}/v1/subscriptions`);
  if (!res.ok) {
    throw new Error(`Failed to fetch subscriptions: HTTP ${res.status}`);
  }
  const body = await res.json();
  return body.data || body;
}

export async function addSubscription(item: Partial<SubscriptionItem>): Promise<any> {
  const res = await apiFetch(`${API_BASE}/v1/subscriptions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(item),
  });
  if (!res.ok) {
    throw new Error(`Failed to add subscription: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function deleteSubscription(symbol: string, source?: string): Promise<any> {
  const params = source ? `?source=${encodeURIComponent(source)}` : '';
  const res = await apiFetch(`${API_BASE}/v1/subscriptions/${encodeURIComponent(symbol)}${params}`, {
    method: 'DELETE',
  });
  if (!res.ok) {
    throw new Error(`Failed to delete subscription: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function fetchIngestionStatus(): Promise<OrchestratorStatus> {
  const res = await apiFetch(`${API_BASE}/v1/ingestion/status`);
  if (!res.ok) {
    throw new Error(`Failed to fetch ingestion status: HTTP ${res.status}`);
  }
  const body = await res.json();
  return body.data || body;
}

export async function triggerManualCatchUp(): Promise<any> {
  const res = await apiFetch(`${API_BASE}/v1/ingestion/catch-up`, {
    method: 'POST',
  });
  if (!res.ok) {
    throw new Error(`Failed to trigger catch-up: HTTP ${res.status}`);
  }
  return await res.json();
}

export async function searchSymbols(query: string, source: string = 'all'): Promise<DiscoverySearchResult[]> {
  const params = new URLSearchParams({ q: query, source });
  const res = await apiFetch(`${API_BASE}/v1/discovery/search?${params.toString()}`);
  if (!res.ok) {
    throw new Error(`Failed searching symbols: HTTP ${res.status}`);
  }
  const body = await res.json();
  return body.data || body;
}
