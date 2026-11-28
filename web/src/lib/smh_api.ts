const API_BASE =
  typeof window !== 'undefined'
    ? ''
    : typeof process !== 'undefined' && process.env?.API_BASE
      ? process.env.API_BASE
      : 'http://127.0.0.1:8080';

export type TradeSignal = 'BUY' | 'HOLD' | 'SELL';

export interface SmhSeriesPoint {
  time: string;
  close: number;
  predicted_5d_price: number | null;
  upper_95_price: number | null;
  lower_95_price: number | null;
  predicted_5d_ret_pct: number | null;
  signal: TradeSignal | null;
  ann_vol_pct: number | null;
  vxn_minus_vix: number | null;
  ztf_z63_lag42: number | null;
  fvx_z63_lag42: number | null;
  move_z63: number | null;
}

export interface SmhForwardConePoint {
  time: string;
  projected_price: number;
  upper_95: number;
  lower_95: number;
}

export interface SmhOosMetrics {
  oos_samples: number;
  oos_r2_vs_mean_pct: number;
  directional_accuracy_pct: number;
  baseline_up_rate_pct: number;
  strategy_sharpe: number;
  buy_hold_sharpe: number;
  strategy_max_dd_pct: number;
  buy_hold_max_dd_pct: number;
}

export interface SmhDriversSnapshot {
  vxn_minus_vix_lag2: number;
  ztf_z63_lag42: number;
  fvx_z63_lag42: number;
  move_z63: number;
  smh_vs_asml_z63_lag21: number;
}

export interface SmhPredictiveResponse {
  symbol: string;
  as_of: string;
  latest_price: number;
  signal: TradeSignal;
  signal_confidence_pct: number;
  signal_reason: string;
  recommended_allocation_pct: number;
  forecast_5d_ret_pct: number;
  forecast_5d_price: number;
  upper_95_5d_price: number;
  lower_95_5d_price: number;
  ann_vol_pct: number;
  drivers: SmhDriversSnapshot;
  oos_metrics: SmhOosMetrics;
  history: SmhSeriesPoint[];
  forward_cone: SmhForwardConePoint[];
}

export async function fetchSmhPredictive(
  asOf?: string,
  range: string = '1y'
): Promise<SmhPredictiveResponse> {
  const params = new URLSearchParams({ range });
  if (asOf && asOf.trim()) {
    params.set('as_of', asOf.trim());
  }
  const res = await fetch(`${API_BASE}/v1/analytics/smh-predictive?${params.toString()}`, {
    credentials: 'same-origin',
  });
  if (res.status === 401 && typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('ea:unauthorized'));
  }
  if (!res.ok) {
    const errText = await res.text().catch(() => '');
    throw new Error(`SMH predictive request failed (${res.status}): ${errText}`);
  }
  const body = await res.json();
  if (!body?.success || !body?.predictive) {
    throw new Error(body?.error || 'Invalid SMH predictive payload');
  }
  return body.predictive as SmhPredictiveResponse;
}
