export type TradeSignal = 'BUY' | 'HOLD' | 'SELL';

export interface SupplyChainEdge {
  target: string;
  supplier: string;
  relationship: string;
  tier: number;
  notes: string;
  updated_at_utc: string;
}

export interface SupplyChainGraph {
  version: string;
  top5_recommended: string[];
  tech20_targets: string[];
  edges: SupplyChainEdge[];
}

export interface SupplyChainLagRow {
  supplier: string;
  feature: string;
  lag_days: number;
  r_discovery: number;
  r_oos: number;
  newey_west_t: number;
  status: string;
}

export interface SupplyChainSeriesPoint {
  time: string;
  close: number;
  forecast_price: number | null;
  signal: TradeSignal | null;
}

export interface SupplyChainAnalysisResponse {
  target: string;
  as_of: string;
  horizon_days: number;
  latest_price: number;
  signal: TradeSignal;
  signal_reason: string;
  forecast_ret_pct: number;
  forecast_price: number;
  oos_r2_pct: number;
  directional_accuracy_pct: number;
  baseline_up_rate_pct: number;
  survivor_count: number;
  lag_matrix: SupplyChainLagRow[];
  history: SupplyChainSeriesPoint[];
}

const API_BASE = typeof window !== 'undefined' ? '' : 'http://127.0.0.1:8080';

async function scFetch(url: string, init?: RequestInit): Promise<Response> {
  const res = await fetch(url, { credentials: 'same-origin', ...init });
  if (res.status === 401 && typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('ea:unauthorized'));
  }
  return res;
}

export async function fetchSupplyChainGraph(): Promise<SupplyChainGraph> {
  const res = await scFetch(`${API_BASE}/v1/supply-chain`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const body = await res.json();
  return body.graph as SupplyChainGraph;
}

export async function upsertSupplyChainEdge(payload: {
  target: string;
  supplier: string;
  relationship?: string;
  tier?: number;
  notes?: string;
}): Promise<SupplyChainGraph> {
  const res = await scFetch(`${API_BASE}/v1/supply-chain/edges`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) throw new Error(await res.text());
  const body = await res.json();
  return body.graph as SupplyChainGraph;
}

export async function deleteSupplyChainEdge(target: string, supplier: string): Promise<SupplyChainGraph> {
  const params = new URLSearchParams({ target, supplier });
  const res = await scFetch(`${API_BASE}/v1/supply-chain/edges?${params.toString()}`, {
    method: 'DELETE',
  });
  if (!res.ok) throw new Error(await res.text());
  const body = await res.json();
  return body.graph as SupplyChainGraph;
}

export async function fetchSupplyChainAnalysis(
  target: string,
  horizon: number = 21,
  range: string = '1y'
): Promise<SupplyChainAnalysisResponse> {
  const params = new URLSearchParams({ target, horizon: String(horizon), range });
  const res = await scFetch(`${API_BASE}/v1/analytics/supply-chain?${params.toString()}`);
  if (!res.ok) throw new Error(await res.text());
  const body = await res.json();
  return body.analysis as SupplyChainAnalysisResponse;
}
