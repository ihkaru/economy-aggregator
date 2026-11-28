import type {
  LiquidityAtlasResponse,
  ProspectiveScorecardReport,
  RegimeTransitionEvent,
} from './atlas_types';

const API_BASE =
  typeof window !== 'undefined'
    ? ''
    : typeof process !== 'undefined' && process.env?.API_BASE
      ? process.env.API_BASE
      : 'http://127.0.0.1:8080';

async function atlasFetch(url: string): Promise<Response> {
  const res = await fetch(url, { credentials: 'same-origin' });
  if (res.status === 401 && typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('ea:unauthorized'));
  }
  return res;
}

export async function fetchLiquidityAtlas(opts: {
  asOf?: string;
  range?: string;
  windowWeeks?: number;
  includeSpillover?: boolean;
}): Promise<LiquidityAtlasResponse> {
  const params = new URLSearchParams();
  if (opts.asOf) params.set('as_of', opts.asOf);
  if (opts.range) params.set('range', opts.range);
  if (opts.windowWeeks) params.set('window_weeks', String(opts.windowWeeks));
  if (opts.includeSpillover !== undefined) {
    params.set('include_spillover', opts.includeSpillover ? 'true' : 'false');
  }
  const res = await atlasFetch(`${API_BASE}/v1/analytics/liquidity-atlas?${params.toString()}`);
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(text || `Failed to fetch Liquidity Atlas: HTTP ${res.status}`);
  }
  const body = await res.json();
  return body.atlas || body;
}

export async function fetchLiquidityTransitions(limit: number = 15): Promise<RegimeTransitionEvent[]> {
  const params = new URLSearchParams({ limit: String(limit) });
  const res = await atlasFetch(`${API_BASE}/v1/analytics/liquidity-tree/transitions?${params.toString()}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch transitions: HTTP ${res.status}`);
  }
  const body = await res.json();
  return body.transitions || [];
}

export async function fetchProspectiveScorecard(asOf?: string): Promise<ProspectiveScorecardReport> {
  const params = new URLSearchParams();
  if (asOf) params.set('as_of', asOf);
  const res = await atlasFetch(`${API_BASE}/v1/analytics/prospective-scorecard?${params.toString()}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch prospective scorecard: HTTP ${res.status}`);
  }
  const body = await res.json();
  return body.scorecard || body;
}
