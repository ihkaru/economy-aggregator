<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import {
    createChart,
    createSeriesMarkers,
    ColorType,
    LineSeries,
    LineStyle,
    type IChartApi,
    type ISeriesApi,
    type ISeriesMarkersPluginApi,
    type LineData,
    type SeriesMarker,
    type Time,
  } from 'lightweight-charts';
  import {
    deleteSupplyChainEdge,
    fetchSupplyChainAnalysis,
    fetchSupplyChainGraph,
    fetchSupplyChainLeaderboard,
    upsertSupplyChainEdge,
    type SupplyChainAnalysisResponse,
    type SupplyChainGraph,
    type SupplyChainLeaderboardResponse,
  } from '../../lib/supply_chain_api';

  let graph: SupplyChainGraph | null = null;
  let leaderboard: SupplyChainLeaderboardResponse | null = null;
  let analysis: SupplyChainAnalysisResponse | null = null;
  let selectedTarget = 'ADI';
  let horizon = 21;
  let range = '1y';
  let loading = false;
  let errorMsg: string | null = null;

  let newSupplier = '';
  let newRel = 'supplier';
  let newNotes = '';

  let chartContainer: HTMLElement | null = null;
  let chart: IChartApi | null = null;
  let priceSeries: ISeriesApi<'Line'> | null = null;
  let fcSeries: ISeriesApi<'Line'> | null = null;
  let markersPlugin: ISeriesMarkersPluginApi<Time> | null = null;
  let resizeObserver: ResizeObserver | null = null;

  function ensureChart() {
    if (chartContainer && !chart) {
      chart = createChart(chartContainer, {
        layout: { background: { type: ColorType.Solid, color: '#090d16' }, textColor: '#94a3b8', fontSize: 11 },
        grid: { vertLines: { color: 'rgba(148,163,184,0.07)' }, horzLines: { color: 'rgba(148,163,184,0.07)' } },
        rightPriceScale: { borderColor: 'rgba(148,163,184,0.16)' },
        timeScale: { borderColor: 'rgba(148,163,184,0.16)', timeVisible: true },
      });
      priceSeries = chart.addSeries(LineSeries, { color: '#38bdf8', lineWidth: 2, title: 'Target Close' });
      markersPlugin = createSeriesMarkers(priceSeries, []);
      fcSeries = chart.addSeries(LineSeries, { color: '#f59e0b', lineWidth: 2, lineStyle: LineStyle.Dashed, title: 'Supply-Chain Forecast' });
      resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          const { width, height } = entry.contentRect;
          if (width > 0 && height > 0) chart?.applyOptions({ width, height });
        }
      });
      resizeObserver.observe(chartContainer);
    }
  }

  async function loadAnalysis(target = selectedTarget) {
    selectedTarget = target;
    loading = true;
    errorMsg = null;
    try {
      const res = await fetchSupplyChainAnalysis(selectedTarget, horizon, range);
      analysis = res;
      await tick();
      ensureChart();
      if (priceSeries && fcSeries && markersPlugin && chart) {
        const pxData: LineData[] = res.history.map((p) => ({ time: p.time as Time, value: p.close }));
        const fData: LineData[] = [];
        const markers: SeriesMarker<Time>[] = [];
        let prevSig: string | null = null;
        for (const p of res.history) {
          if (p.forecast_price !== null) fData.push({ time: p.time as Time, value: p.forecast_price });
          if (p.signal && p.signal !== prevSig && p.signal !== 'HOLD') {
            markers.push({
              time: p.time as Time,
              position: p.signal === 'BUY' ? 'belowBar' : 'aboveBar',
              color: p.signal === 'BUY' ? '#10b981' : '#f43f5e',
              shape: p.signal === 'BUY' ? 'arrowUp' : 'arrowDown',
              text: p.signal,
            });
          }
          if (p.signal) prevSig = p.signal;
        }
        priceSeries.setData(pxData);
        markersPlugin.setMarkers(markers);
        fcSeries.setData(fData);
        chart.timeScale().fitContent();
      }
    } catch (e: any) {
      errorMsg = e?.message || 'Failed to load supply-chain analysis';
    } finally {
      loading = false;
    }
  }

  async function addSupplier() {
    if (!newSupplier.trim()) return;
    try {
      graph = await upsertSupplyChainEdge({
        target: selectedTarget,
        supplier: newSupplier.trim(),
        relationship: newRel.trim() || 'supplier',
        notes: newNotes.trim(),
      });
      newSupplier = '';
      newNotes = '';
      await loadAnalysis(selectedTarget);
    } catch (e: any) {
      errorMsg = e?.message || 'Failed to add edge';
    }
  }

  async function removeSupplier(sup: string) {
    try {
      graph = await deleteSupplyChainEdge(selectedTarget, sup);
      await loadAnalysis(selectedTarget);
    } catch (e: any) {
      errorMsg = e?.message || 'Failed to remove edge';
    }
  }

  async function loadLeaderboard() {
    try {
      leaderboard = await fetchSupplyChainLeaderboard(horizon);
    } catch {
      // keep existing leaderboard if transient error
    }
  }

  onMount(async () => {
    try {
      const [g, lb] = await Promise.all([
        fetchSupplyChainGraph(),
        fetchSupplyChainLeaderboard(horizon),
      ]);
      graph = g;
      leaderboard = lb;
      if (lb.top5_by_sortino.length > 0) selectedTarget = lb.top5_by_sortino[0];
      else if (g.top5_recommended.length > 0) selectedTarget = g.top5_recommended[0];
    } catch (e: any) {
      errorMsg = e?.message || 'Failed to load supply chain graph';
    }
    await loadAnalysis(selectedTarget);
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    chart?.remove();
  });

  $: currentEdges = graph?.edges.filter((e) => e.target === selectedTarget) ?? [];
  $: sigClass = analysis?.signal === 'BUY' ? 'sig-buy' : analysis?.signal === 'SELL' ? 'sig-sell' : 'sig-hold';
</script>

<div class="sc-shell">
  <div class="sc-header">
    <div>
      <div class="eyebrow">PERSISTENT SUPPLY-CHAIN GRAPH &amp; LEAD-LAG ANALYZER (TOP 5 OF 20 TECH)</div>
      <h2 class="title">Cross-Stock Supply-Chain Signal — {selectedTarget}</h2>
    </div>
    <div class="controls">
      <div class="btn-group">
        {#each (leaderboard?.top5_by_sortino || graph?.top5_recommended || ['ADI', 'NXPI', 'INTC', 'AAPL', 'ORCL']) as t}
          <button class="ctrl-btn top-btn" class:active={selectedTarget === t} on:click={() => loadAnalysis(t)}>
            ★ {t}
          </button>
        {/each}
      </div>
      {#if graph}
        <select class="target-select" bind:value={selectedTarget} on:change={() => loadAnalysis(selectedTarget)}>
          {#each graph.tech20_targets as t}
            <option value={t}>{t}</option>
          {/each}
        </select>
      {/if}
      <div class="btn-group">
        <button class="ctrl-btn" class:active={horizon === 5} on:click={() => { horizon = 5; loadLeaderboard(); loadAnalysis(); }}>5D</button>
        <button class="ctrl-btn" class:active={horizon === 21} on:click={() => { horizon = 21; loadLeaderboard(); loadAnalysis(); }}>21D</button>
      </div>
      <div class="btn-group">
        {#each ['3m', '6m', '1y', '3y', 'all'] as r}
          <button class="ctrl-btn" class:active={range === r} on:click={() => { range = r; loadAnalysis(); }}>{r.toUpperCase()}</button>
        {/each}
      </div>
    </div>
  </div>

  {#if loading}<div class="loading-bar"></div>{/if}
  {#if errorMsg}<div class="err-box">{errorMsg}</div>{/if}

  {#if analysis}
    <div class="decision-banner {sigClass}">
      <div class="signal-pill">{analysis.signal}</div>
      <div class="decision-main">
        <div class="decision-top">
          <span class="price-now">{analysis.target} ${analysis.latest_price.toFixed(2)}</span>
          <span class="tag">POS: {analysis.current_position > 0.5 ? '1.0x LONG' : '0.0x FLAT'} (NO PYRAMIDING)</span>
          <span class="tag">SORTINO: {analysis.strategy_sortino.toFixed(2)} (B&amp;H {analysis.buy_hold_sortino.toFixed(2)})</span>
          <span class="tag">MAX DD: {analysis.strategy_max_dd_pct.toFixed(1)}% (B&amp;H {analysis.buy_hold_max_dd_pct.toFixed(1)}%)</span>
          <span class="tag">TRADES: {analysis.total_trades}</span>
          <span class="tag">OOS R² {analysis.oos_r2_pct.toFixed(2)}%</span>
        </div>
        <div class="decision-reason">{analysis.signal_reason}</div>
      </div>
      <div class="forecast-box">
        <div class="fc-label">{analysis.horizon_days}D SUPPLY-CHAIN TARGET</div>
        <div class="fc-val">${analysis.forecast_price.toFixed(2)} ({analysis.forecast_ret_pct >= 0 ? '+' : ''}{analysis.forecast_ret_pct.toFixed(2)}%)</div>
      </div>
    </div>

    <div class="chart-card">
      <div class="chart-canvas" bind:this={chartContainer}></div>
    </div>

    <div class="bottom-grid">
      <div class="panel-card">
        <div class="p-title">REGISTERED SUPPLIERS FOR {selectedTarget} (AUTO-SUBSCRIBED &amp; PERSISTENT)</div>
        <div class="edge-form">
          <input class="inp" placeholder="Supplier (e.g. 4063.T)" bind:value={newSupplier} />
          <input class="inp" placeholder="Relation (foundry/wafers)" bind:value={newRel} />
          <input class="inp" placeholder="Notes" bind:value={newNotes} />
          <button class="add-btn" on:click={addSupplier}>+ ADD</button>
        </div>
        <div class="edge-list">
          {#each currentEdges as edge}
            <div class="edge-row">
              <span><b>{edge.supplier}</b> <small>({edge.relationship})</small> {edge.notes}</span>
              <button class="del-btn" on:click={() => removeSupplier(edge.supplier)}>×</button>
            </div>
          {/each}
        </div>
      </div>

      <div class="panel-card">
        <div class="p-title">LEAD-LAG STABILITY MATRIX (DISCOVERY &le;2022 vs OOS 2023-2026)</div>
        <div class="table-wrap">
          <table>
            <thead><tr><th>Driver</th><th>Lag</th><th>r_disc</th><th>r_oos</th><th>NW-t</th><th>Status</th></tr></thead>
            <tbody>
              {#each analysis.lag_matrix.slice(0, 10) as row}
                <tr>
                  <td>{row.feature}</td>
                  <td>{row.lag_days}d</td>
                  <td>{row.r_discovery.toFixed(3)}</td>
                  <td>{row.r_oos.toFixed(3)}</td>
                  <td>{row.newey_west_t.toFixed(2)}</td>
                  <td class={row.status === 'SURVIVED' ? 'st-ok' : row.status === 'FLIPPED' ? 'st-bad' : 'st-warn'}>{row.status}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
      <div class="panel-card">
        <div class="p-title">MOST RECENT SIGNALS — {selectedTarget} (STRICT NO-PYRAMIDING 1.0x)</div>
        <div class="table-wrap">
          <table>
            <thead><tr><th>Date</th><th>Action</th><th>Price</th><th>Fcst</th><th>Pos After</th></tr></thead>
            <tbody>
              {#each analysis.recent_signals as ev}
                <tr>
                  <td>{ev.time}</td>
                  <td class={ev.action === 'BUY_ENTRY' ? 'st-ok' : 'st-bad'}>{ev.action}</td>
                  <td>${ev.price.toFixed(2)}</td>
                  <td>{ev.forecast_ret_pct >= 0 ? '+' : ''}{ev.forecast_ret_pct.toFixed(2)}%</td>
                  <td>{ev.position_after.toFixed(1)}x</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

      {#if leaderboard}
        <div class="panel-card">
          <div class="p-title">TOP TECH RANKED BY SORTINO RATIO (NO PYRAMIDING, {leaderboard.horizon_days}D)</div>
          <div class="table-wrap">
            <table>
              <thead><tr><th>#</th><th>Target</th><th>Signal</th><th>Sortino</th><th>B&amp;H Sortino</th><th>MaxDD</th><th>Last Signal</th></tr></thead>
              <tbody>
                {#each leaderboard.rows as r, idx}
                  <tr class="click-row" class:sel-row={r.target === selectedTarget} on:click={() => loadAnalysis(r.target)}>
                    <td>{idx < 5 ? `★${idx + 1}` : idx + 1}</td>
                    <td><b>{r.target}</b></td>
                    <td class={r.signal === 'BUY' ? 'st-ok' : r.signal === 'SELL' ? 'st-bad' : 'st-warn'}>{r.signal}</td>
                    <td><b>{r.strategy_sortino.toFixed(2)}</b></td>
                    <td>{r.buy_hold_sortino.toFixed(2)}</td>
                    <td>{r.strategy_max_dd_pct.toFixed(1)}%</td>
                    <td>{r.last_event_date} ({r.last_event_action})</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sc-shell { padding: 14px 18px; color: #e2e8f0; display: flex; flex-direction: column; gap: 12px; height: 100%; overflow-y: auto; background: #060911; }
  .sc-header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px; }
  .eyebrow { font-size: 10px; letter-spacing: 0.08em; color: #38bdf8; font-weight: 700; }
  .title { margin: 2px 0 0; font-size: 16px; font-weight: 700; color: #f8fafc; }
  .controls { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .btn-group { display: flex; gap: 4px; background: #0f172a; padding: 3px; border-radius: 6px; border: 1px solid #1e293b; }
  .ctrl-btn { background: transparent; color: #94a3b8; border: none; padding: 4px 8px; font-size: 11px; font-weight: 600; border-radius: 4px; cursor: pointer; }
  .ctrl-btn.active, .ctrl-btn:hover { background: #1e293b; color: #f8fafc; }
  .top-btn.active { background: #0ea5e9; color: #082f49; }
  .target-select { background: #0f172a; color: #f8fafc; border: 1px solid #1e293b; padding: 5px 8px; border-radius: 6px; font-size: 11px; font-weight: 600; }
  .loading-bar { height: 2px; background: linear-gradient(90deg, transparent, #38bdf8, transparent); }
  .err-box { background: rgba(244,63,94,0.14); border: 1px solid rgba(244,63,94,0.4); padding: 8px 12px; border-radius: 6px; color: #fda4af; font-size: 12px; }
  .decision-banner { display: flex; flex-wrap: wrap; align-items: center; gap: 14px; padding: 12px 16px; border-radius: 8px; border: 1px solid #1e293b; background: #0b1220; }
  .sig-buy { border-color: rgba(16,185,129,0.5); background: rgba(16,185,129,0.08); }
  .sig-hold { border-color: rgba(245,158,11,0.5); background: rgba(245,158,11,0.08); }
  .sig-sell { border-color: rgba(244,63,94,0.5); background: rgba(244,63,94,0.08); }
  .signal-pill { font-size: 18px; font-weight: 800; padding: 7px 14px; border-radius: 6px; min-width: 82px; text-align: center; }
  .sig-buy .signal-pill { background: #10b981; color: #022c22; }
  .sig-hold .signal-pill { background: #f59e0b; color: #271502; }
  .sig-sell .signal-pill { background: #f43f5e; color: #fff1f2; }
  .decision-main { flex: 1; min-width: 240px; }
  .decision-top { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; font-size: 12px; font-weight: 700; }
  .price-now { font-size: 15px; color: #f8fafc; }
  .tag { background: #1e293b; color: #cbd5e1; padding: 2px 7px; border-radius: 4px; font-size: 10px; }
  .decision-reason { margin-top: 4px; font-size: 12px; color: #cbd5e1; }
  .forecast-box { text-align: right; }
  .fc-label { font-size: 10px; color: #94a3b8; font-weight: 700; }
  .fc-val { font-size: 15px; font-weight: 700; color: #f8fafc; }
  .chart-card { background: #090d16; border: 1px solid #1e293b; border-radius: 8px; padding: 8px; }
  .chart-canvas { width: 100%; height: 290px; }
  .bottom-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 12px; }
  .panel-card { background: #0b1220; border: 1px solid #1e293b; border-radius: 8px; padding: 10px 12px; font-size: 11px; display: flex; flex-direction: column; gap: 8px; }
  .p-title { font-size: 10px; font-weight: 700; color: #38bdf8; letter-spacing: 0.06em; }
  .edge-form { display: flex; flex-wrap: wrap; gap: 6px; }
  .inp { background: #0f172a; border: 1px solid #1e293b; color: #f8fafc; padding: 4px 8px; border-radius: 4px; font-size: 11px; flex: 1; min-width: 90px; }
  .add-btn { background: #0ea5e9; color: #082f49; border: none; padding: 4px 10px; border-radius: 4px; font-weight: 700; cursor: pointer; font-size: 11px; }
  .edge-list { display: flex; flex-direction: column; gap: 4px; max-height: 180px; overflow-y: auto; }
  .edge-row { display: flex; justify-content: space-between; align-items: center; background: #0f172a; padding: 5px 8px; border-radius: 4px; }
  .del-btn { background: transparent; border: none; color: #fb7185; font-size: 14px; cursor: pointer; }
  .table-wrap { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 11px; }
  th, td { text-align: left; padding: 4px 6px; border-bottom: 1px solid #1e293b; }
  th { color: #94a3b8; font-size: 10px; }
  .st-ok { color: #34d399; font-weight: 700; }
  .st-bad { color: #fb7185; font-weight: 700; }
  .st-warn { color: #fbbf24; }
  .click-row { cursor: pointer; }
  .click-row:hover { background: #1e293b; }
  .sel-row { background: rgba(56, 189, 248, 0.14); }
</style>
