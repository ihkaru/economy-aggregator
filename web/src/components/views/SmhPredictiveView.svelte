<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { fetchSmhPredictive, type SmhPredictiveResponse } from '../../lib/smh_api';
  import { createSmhPriceChart, type SmhChartHandle } from '../../lib/smh_chart';

  let data: SmhPredictiveResponse | null = null;
  let range = '1y';
  let asOf = '';
  let loading = false;
  let errorMsg: string | null = null;

  let chartContainer: HTMLElement | null = null;
  let chartHandle: SmhChartHandle | null = null;
  let resizeObserver: ResizeObserver | null = null;

  async function load(targetAsOf?: string) {
    loading = true;
    errorMsg = null;
    try {
      const res = await fetchSmhPredictive(targetAsOf, range);
      data = res;
      if (!targetAsOf) asOf = res.as_of;
      if (chartHandle) {
        chartHandle.update(res);
      }
    } catch (e: any) {
      errorMsg = e?.message || 'Failed to load SMH predictive model';
    } finally {
      loading = false;
    }
  }

  function shiftDate(days: number) {
    const base = data?.as_of || asOf;
    if (!base) return;
    const d = new Date(`${base}T00:00:00Z`);
    d.setUTCDate(d.getUTCDate() + days);
    const next = d.toISOString().slice(0, 10);
    asOf = next;
    load(next);
  }

  function resetLatest() {
    asOf = '';
    load(undefined);
  }

  function setRange(r: string) {
    range = r;
    load(asOf || undefined);
  }

  onMount(async () => {
    if (chartContainer) {
      chartHandle = createSmhPriceChart(chartContainer);
      resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          const { width, height } = entry.contentRect;
          if (width > 0 && height > 0) chartHandle?.resize(width, height);
        }
      });
      resizeObserver.observe(chartContainer);
    }
    await load();
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    chartHandle?.destroy();
  });

  $: sigClass =
    data?.signal === 'BUY' ? 'sig-buy' : data?.signal === 'SELL' ? 'sig-sell' : 'sig-hold';
</script>

<div class="smh-shell">
  <div class="smh-header">
    <div>
      <div class="eyebrow">VANECK SEMICONDUCTOR ETF (SMH) — CAUSAL 5-DAY LAG-RIDGE + EWMA-GARCH</div>
      <h2 class="title">SMH Predictive Signal &amp; Risk Envelope</h2>
    </div>

    <div class="controls">
      <div class="btn-group">
        {#each ['3m', '6m', '1y', '3y', 'all'] as r}
          <button class="ctrl-btn" class:active={range === r} on:click={() => setRange(r)}>
            {r.toUpperCase()}
          </button>
        {/each}
      </div>
      <div class="btn-group">
        <button class="ctrl-btn" on:click={() => shiftDate(-28)} disabled={loading}>-4W</button>
        <button class="ctrl-btn" on:click={() => shiftDate(-7)} disabled={loading}>-1W</button>
        <button class="ctrl-btn" on:click={() => shiftDate(7)} disabled={loading}>+1W</button>
        <button class="ctrl-btn" on:click={resetLatest} disabled={loading}>LATEST</button>
      </div>
    </div>
  </div>

  {#if loading}
    <div class="loading-bar"></div>
  {/if}

  {#if errorMsg}
    <div class="err-box">{errorMsg}</div>
  {/if}

  {#if data}
    <div class="decision-banner {sigClass}">
      <div class="signal-pill">{data.signal}</div>
      <div class="decision-main">
        <div class="decision-top">
          <span class="price-now">SMH ${data.latest_price.toFixed(2)}</span>
          <span class="asof-tag">AS OF {data.as_of}</span>
          <span class="conf-tag">CONFIDENCE {data.signal_confidence_pct.toFixed(0)}%</span>
          <span class="alloc-tag">ALLOCATION {data.recommended_allocation_pct.toFixed(0)}%</span>
        </div>
        <div class="decision-reason">{data.signal_reason}</div>
      </div>
      <div class="forecast-box">
        <div class="fc-label">5D TARGET / 95% ENVELOPE</div>
        <div class="fc-val">
          ${data.forecast_5d_price.toFixed(2)}
          <span class:pos={data.forecast_5d_ret_pct >= 0} class:neg={data.forecast_5d_ret_pct < 0}>
            ({data.forecast_5d_ret_pct >= 0 ? '+' : ''}{data.forecast_5d_ret_pct.toFixed(2)}%)
          </span>
        </div>
        <div class="fc-band">
          ${data.lower_95_5d_price.toFixed(2)} .. ${data.upper_95_5d_price.toFixed(2)} · Vol {data.ann_vol_pct.toFixed(1)}%
        </div>
      </div>
    </div>

    <div class="chart-card">
      <div class="chart-legend">
        <span><i class="dot dot-blue"></i>SMH Close</span>
        <span><i class="dot dot-amber"></i>5d Lag-Ridge Forecast + Forward Cone</span>
        <span><i class="dot dot-green"></i>Upper 95% EWMA-GARCH</span>
        <span><i class="dot dot-rose"></i>Lower 95% EWMA-GARCH</span>
      </div>
      <div class="chart-canvas" bind:this={chartContainer}></div>
    </div>

    <div class="metrics-grid">
      <div class="metric-card">
        <div class="m-title">VALIDATED LAG DRIVERS</div>
        <div class="m-row"><span>^VXN − ^VIX (lag 2d)</span><b>{data.drivers.vxn_minus_vix_lag2.toFixed(2)}</b></div>
        <div class="m-row"><span>ZT=F 2Y Z63 (lag 42d)</span><b>{data.drivers.ztf_z63_lag42.toFixed(2)}</b></div>
        <div class="m-row"><span>^FVX 5Y Z63 (lag 42d)</span><b>{data.drivers.fvx_z63_lag42.toFixed(2)}</b></div>
        <div class="m-row"><span>^MOVE Stress Z63</span><b>{data.drivers.move_z63.toFixed(2)}</b></div>
        <div class="m-row"><span>SMH/ASML Z63 (lag 21d)</span><b>{data.drivers.smh_vs_asml_z63_lag21.toFixed(2)}</b></div>
      </div>

      <div class="metric-card">
        <div class="m-title">WALK-FORWARD OOS SCORECARD (ZERO LOOKAHEAD)</div>
        <div class="m-row"><span>OOS Samples (5d)</span><b>{data.oos_metrics.oos_samples}</b></div>
        <div class="m-row"><span>OOS R² vs Mean</span><b>{data.oos_metrics.oos_r2_vs_mean_pct.toFixed(2)}%</b></div>
        <div class="m-row"><span>Directional Accuracy</span><b>{data.oos_metrics.directional_accuracy_pct.toFixed(1)}% (Base {data.oos_metrics.baseline_up_rate_pct.toFixed(1)}%)</b></div>
        <div class="m-row"><span>Strategy Sharpe vs B&amp;H</span><b>{data.oos_metrics.strategy_sharpe.toFixed(2)} vs {data.oos_metrics.buy_hold_sharpe.toFixed(2)}</b></div>
        <div class="m-row"><span>MaxDD Strategy vs B&amp;H</span><b>{data.oos_metrics.strategy_max_dd_pct.toFixed(1)}% vs {data.oos_metrics.buy_hold_max_dd_pct.toFixed(1)}%</b></div>
      </div>

      <div class="metric-card">
        <div class="m-title">SIGNAL RULES (BUY / HOLD / SELL)</div>
        <div class="m-rule"><b class="c-buy">BUY</b>: 5d forecast &ge; +0.35%, risk-adj z &ge; +0.10, ^MOVE Z63 &lt; 2.20</div>
        <div class="m-rule"><b class="c-hold">HOLD</b>: Neutral band (-0.15% .. +0.35%) — hold vol-targeted size</div>
        <div class="m-rule"><b class="c-sell">SELL</b>: 5d forecast &le; -0.15% or ^MOVE Z63 &ge; 2.20 (cash / defensive)</div>
      </div>
    </div>
  {/if}
</div>

<style>
  .smh-shell { padding: 14px 18px; color: #e2e8f0; display: flex; flex-direction: column; gap: 12px; height: 100%; overflow-y: auto; background: #060911; }
  .smh-header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px; }
  .eyebrow { font-size: 10px; letter-spacing: 0.08em; color: #38bdf8; font-weight: 700; }
  .title { margin: 2px 0 0; font-size: 16px; font-weight: 700; color: #f8fafc; }
  .controls { display: flex; flex-wrap: wrap; gap: 8px; }
  .btn-group { display: flex; gap: 4px; background: #0f172a; padding: 3px; border-radius: 6px; border: 1px solid #1e293b; }
  .ctrl-btn { background: transparent; color: #94a3b8; border: none; padding: 4px 9px; font-size: 11px; font-weight: 600; border-radius: 4px; cursor: pointer; }
  .ctrl-btn.active, .ctrl-btn:hover { background: #1e293b; color: #f8fafc; }
  .loading-bar { height: 2px; background: linear-gradient(90deg, transparent, #38bdf8, transparent); }
  .err-box { background: rgba(244, 63, 94, 0.14); border: 1px solid rgba(244, 63, 94, 0.4); padding: 8px 12px; border-radius: 6px; color: #fda4af; font-size: 12px; }
  .decision-banner { display: flex; flex-wrap: wrap; align-items: center; gap: 14px; padding: 12px 16px; border-radius: 8px; border: 1px solid #1e293b; background: #0b1220; }
  .sig-buy { border-color: rgba(16, 185, 129, 0.5); background: rgba(16, 185, 129, 0.08); }
  .sig-hold { border-color: rgba(245, 158, 11, 0.5); background: rgba(245, 158, 11, 0.08); }
  .sig-sell { border-color: rgba(244, 63, 94, 0.5); background: rgba(244, 63, 94, 0.08); }
  .signal-pill { font-size: 20px; font-weight: 800; letter-spacing: 0.06em; padding: 8px 16px; border-radius: 6px; min-width: 92px; text-align: center; }
  .sig-buy .signal-pill { background: #10b981; color: #022c22; }
  .sig-hold .signal-pill { background: #f59e0b; color: #271502; }
  .sig-sell .signal-pill { background: #f43f5e; color: #fff1f2; }
  .decision-main { flex: 1; min-width: 240px; }
  .decision-top { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; font-size: 12px; font-weight: 700; }
  .price-now { font-size: 15px; color: #f8fafc; }
  .asof-tag, .conf-tag, .alloc-tag { background: #1e293b; color: #cbd5e1; padding: 2px 7px; border-radius: 4px; font-size: 10px; }
  .decision-reason { margin-top: 4px; font-size: 12px; color: #cbd5e1; }
  .forecast-box { text-align: right; min-width: 190px; }
  .fc-label { font-size: 10px; color: #94a3b8; font-weight: 700; }
  .fc-val { font-size: 15px; font-weight: 700; color: #f8fafc; }
  .fc-band { font-size: 11px; color: #94a3b8; }
  .pos { color: #34d399; }
  .neg { color: #fb7185; }
  .chart-card { background: #090d16; border: 1px solid #1e293b; border-radius: 8px; padding: 8px 10px; display: flex; flex-direction: column; gap: 6px; }
  .chart-legend { display: flex; flex-wrap: wrap; gap: 14px; font-size: 11px; color: #94a3b8; }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 5px; }
  .dot-blue { background: #38bdf8; }
  .dot-amber { background: #f59e0b; }
  .dot-green { background: #10b981; }
  .dot-rose { background: #f43f5e; }
  .chart-canvas { width: 100%; height: 340px; }
  .metrics-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 10px; }
  .metric-card { background: #0b1220; border: 1px solid #1e293b; border-radius: 8px; padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; font-size: 11px; }
  .m-title { font-size: 10px; font-weight: 700; color: #38bdf8; letter-spacing: 0.06em; margin-bottom: 2px; }
  .m-row { display: flex; justify-content: space-between; color: #cbd5e1; }
  .m-rule { color: #cbd5e1; line-height: 1.4; }
  .c-buy { color: #34d399; }
  .c-hold { color: #fbbf24; }
  .c-sell { color: #fb7185; }
</style>
