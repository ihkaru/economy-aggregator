<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import type { IChartApi, Time } from 'lightweight-charts';
  import Icons from '../Icons.svelte';
  import { formatLayerBadge, formatMetricVal, type AtlasTank } from '../../lib/atlas_types';
  import {
    addAtlasSeriesToChart,
    createTankChart,
    findPointAtOrBefore,
    normalizeChartTime,
    type TankSeriesApi,
  } from '../../lib/atlas_chart';

  export let tank: AtlasTank;
  export let asOfDate: string = '';
  export let windowWeeks: number = 4;
  export let syncedTime: string | null = null;
  export let onHoverTime: (time: string | null) => void = () => {};

  let chartContainer: HTMLDivElement;
  let chart: IChartApi | null = null;
  let primaryApi: TankSeriesApi | null = null;
  let overlayApi: TankSeriesApi | null = null;

  let activeOverlayIdx: number | null = 0;
  let showLineageDetails = false;
  let isSelfHovering = false;
  let lastTankId = '';

  $: if (tank.tank_id !== lastTankId) {
    lastTankId = tank.tank_id;
    activeOverlayIdx = tank.overlay_series.length > 0 ? 0 : null;
  } else if (activeOverlayIdx !== null && activeOverlayIdx >= tank.overlay_series.length) {
    activeOverlayIdx = tank.overlay_series.length > 0 ? 0 : null;
  }

  $: activeOverlay =
    activeOverlayIdx !== null && tank.overlay_series[activeOverlayIdx]
      ? tank.overlay_series[activeOverlayIdx]
      : null;

  function computeWinStart(dateStr: string, weeks: number): string | undefined {
    if (!dateStr) return undefined;
    const d = new Date(`${dateStr}T00:00:00Z`);
    if (Number.isNaN(d.getTime())) return undefined;
    d.setUTCDate(d.getUTCDate() - weeks * 7);
    return d.toISOString().slice(0, 10);
  }

  function rebuildChartSeries(targetTank: AtlasTank, ovIdx: number | null, cutoff: string, weeks: number) {
    if (!chart) return;
    if (primaryApi) {
      chart.removeSeries(primaryApi);
      primaryApi = null;
    }
    if (overlayApi) {
      chart.removeSeries(overlayApi);
      overlayApi = null;
    }
    const winStart = computeWinStart(cutoff, weeks);
    const primaryScale: 'right' | 'left' = targetTank.primary_series.price_scale === 'left' ? 'left' : 'right';
    primaryApi = addAtlasSeriesToChart(chart, targetTank.primary_series, primaryScale, false, cutoff || undefined, winStart);

    const ov = ovIdx !== null && targetTank.overlay_series[ovIdx] ? targetTank.overlay_series[ovIdx] : null;
    if (ov) {
      const ovScale: 'right' | 'left' = primaryScale === 'right' ? 'left' : 'right';
      chart.applyOptions({ leftPriceScale: { visible: true, borderColor: '#1c202d' } });
      overlayApi = addAtlasSeriesToChart(chart, ov, ovScale, true, cutoff || undefined, winStart);
    } else {
      chart.applyOptions({ leftPriceScale: { visible: false } });
    }
    chart.timeScale().fitContent();
  }

  onMount(() => {
    chart = createTankChart(chartContainer);
    chart.subscribeCrosshairMove((param) => {
      if (!isSelfHovering) return;
      if (!param.point || param.point.x < 0 || param.point.y < 0) {
        onHoverTime(null);
        return;
      }
      onHoverTime(normalizeChartTime(param.time));
    });
    rebuildChartSeries(tank, activeOverlayIdx, asOfDate, windowWeeks);
  });

  onDestroy(() => {
    if (chart) {
      chart.remove();
      chart = null;
    }
  });

  $: if (chart && tank) {
    rebuildChartSeries(tank, activeOverlayIdx, asOfDate, windowWeeks);
  }

  $: if (chart && primaryApi && !isSelfHovering) {
    if (syncedTime) {
      const m = findPointAtOrBefore(tank.primary_series.points, syncedTime);
      if (m) {
        try {
          chart.setCrosshairPosition(m.point.value, m.point.time as Time, primaryApi);
        } catch {
          // Ignore when synced timestamp precedes this tank's visible range
        }
      }
    } else {
      chart.clearCrosshairPosition();
    }
  }

  $: primaryLayer = formatLayerBadge(tank.primary_series.data_layer);
  $: hoverPrimary = syncedTime ? findPointAtOrBefore(tank.primary_series.points, syncedTime) : null;
  $: hoverOverlay = syncedTime && activeOverlay ? findPointAtOrBefore(activeOverlay.points, syncedTime) : null;
</script>

<article class="tank-card">
  <header class="tank-head">
    <div class="head-top">
      <div class="title-block">
        <h3 class="tank-title">{tank.title}</h3>
        <p class="tank-sub">{tank.subtitle}</p>
      </div>
      <div class="head-badges">
        <span class="layer-badge {primaryLayer.tone}" title="Data transformation layer">
          {primaryLayer.label}
        </span>
        <button
          type="button"
          class="scope-toggle"
          class:active={showLineageDetails}
          on:click={() => (showLineageDetails = !showLineageDetails)}
          title="Inspect source lineage and accounting boundary"
          aria-expanded={showLineageDetails}
        >
          <Icons name="info" size={13} />
          <span>SCOPE</span>
        </button>
      </div>
    </div>

    {#if tank.staleness_warning}
      <div class="lag-banner" role="status">
        <Icons name="alert-triangle" size={12} />
        <span>{tank.staleness_warning}</span>
      </div>
    {/if}
  </header>

  <!-- Primary + Overlay Summary Readout Grid -->
  <div class="series-strip tabular-nums">
    <div class="metric-cell primary-cell">
      <div class="metric-label">
        <span class="swatch" style="background: {tank.primary_series.color}"></span>
        <span>{tank.primary_series.label}</span>
      </div>
      <div class="metric-nums">
        <span class="val-main">
          {#if hoverPrimary}
            {formatMetricVal(hoverPrimary.point.value, tank.primary_series.unit)}
            <span class="cursor-date">({hoverPrimary.point.time})</span>
          {:else}
            {formatMetricVal(tank.primary_series.latest_value, tank.primary_series.unit)}
          {/if}
        </span>
        {#if tank.primary_series.window_delta !== null}
          <span
            class="val-delta"
            class:bull={tank.primary_series.window_delta >= 0}
            class:bear={tank.primary_series.window_delta < 0}
          >
            {windowWeeks}W: {formatMetricVal(tank.primary_series.window_delta, tank.primary_series.unit, true)}
          </span>
        {/if}
      </div>
    </div>

    {#if tank.overlay_series.length > 0}
      <div class="overlay-pills">
        {#each tank.overlay_series as ov, idx}
          {@const isSelected = activeOverlayIdx === idx}
          <button
            type="button"
            class="ov-pill"
            class:selected={isSelected}
            style="--ov-color: {ov.color}"
            on:click={() => (activeOverlayIdx = isSelected ? null : idx)}
            title="{ov.label} | source: {ov.source} | latest: {ov.latest_date || 'N/A'}"
          >
            <span class="ov-head">
              <span class="swatch" style="background: {ov.color}"></span>
              <span class="ov-name">{ov.label}</span>
            </span>
            <span class="ov-vals">
              {#if isSelected && hoverOverlay}
                <span class="ov-cur">{formatMetricVal(hoverOverlay.point.value, ov.unit)}</span>
              {:else}
                <span class="ov-cur">{formatMetricVal(ov.latest_value, ov.unit)}</span>
              {/if}
              {#if ov.stale_lag_days !== null && ov.stale_lag_days > 10}
                <span class="ov-lag">-{ov.stale_lag_days}d</span>
              {/if}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Synchronized Lightweight-Charts Canvas -->
  <div
    class="chart-stage"
    on:mouseenter={() => (isSelfHovering = true)}
    on:mouseleave={() => {
      isSelfHovering = false;
      onHoverTime(null);
    }}
    role="region"
    aria-label="{tank.title} chart"
  >
    {#if tank.primary_series.points.length === 0}
      <div class="no-points-overlay">
        <span>NO OBSERVATIONS IN WINDOW (&lt;= AS_OF)</span>
      </div>
    {/if}
    <div class="chart-canvas" bind:this={chartContainer}></div>
  </div>

  {#if showLineageDetails}
    <footer class="scope-drawer tabular-nums">
      <p class="scope-note">{tank.accounting_scope_note}</p>
      <div class="lineage-rows">
        {#each [tank.primary_series, ...tank.overlay_series] as s}
          <div class="lineage-row">
            <span class="lin-id">{s.series_id}</span>
            <span class="lin-meta">
              src:{s.source} | layer:{s.data_layer} | latest:{s.latest_date || 'N/A'} ({s.stale_lag_days ?? '-'}d lag) | {s.points.length} pts
            </span>
          </div>
        {/each}
      </div>
    </footer>
  {/if}
</article>

<style>
  .tank-card { background: var(--bg-panel); border: 1px solid var(--border-subtle); display: flex; flex-direction: column; min-width: 0; }
  .tank-head { padding: 8px 10px 6px; border-bottom: 1px solid var(--border-subtle); display: flex; flex-direction: column; gap: 5px; }
  .head-top { display: flex; justify-content: space-between; align-items: flex-start; gap: 8px; }
  .title-block { min-width: 0; flex: 1; }
  .tank-title { font-size: 12.5px; font-weight: 700; color: var(--text-bright); line-height: 1.25; overflow-wrap: anywhere; }
  .tank-sub { font-size: 10px; color: #94a3b8; margin-top: 1px; line-height: 1.3; overflow-wrap: anywhere; }
  .head-badges { display: flex; align-items: center; gap: 5px; flex-shrink: 0; }
  .layer-badge { font-size: 9px; font-weight: 700; padding: 2px 5px; border: 1px solid var(--border-contrast); letter-spacing: 0.4px; }
  .layer-badge.bull { color: #34d399; border-color: rgba(8, 153, 129, 0.5); background: rgba(8, 153, 129, 0.12); }
  .layer-badge.blue { color: #93c5fd; border-color: rgba(59, 130, 246, 0.5); background: rgba(59, 130, 246, 0.12); }
  .layer-badge.amber { color: #fbbf24; border-color: rgba(245, 158, 11, 0.5); background: rgba(245, 158, 11, 0.12); }
  .layer-badge.purple { color: #c4b5fd; border-color: rgba(168, 85, 247, 0.5); background: rgba(168, 85, 247, 0.12); }
  .layer-badge.slate { color: #cbd5e1; border-color: #334155; background: #1e293b; }
  .scope-toggle { background: var(--bg-surface); border: 1px solid var(--border-contrast); color: #94a3b8; height: 22px; padding: 0 6px; display: inline-flex; align-items: center; gap: 4px; font-size: 9.5px; font-weight: 700; cursor: pointer; }
  .scope-toggle:hover, .scope-toggle.active { color: var(--text-bright); border-color: #3b82f6; background: var(--bg-hover); }
  .scope-toggle:focus-visible, .ov-pill:focus-visible { outline: 2px solid #3b82f6; outline-offset: 1px; }
  .lag-banner { display: flex; align-items: center; gap: 5px; padding: 3px 7px; font-size: 9.5px; font-weight: 600; color: #fbbf24; background: rgba(245, 158, 11, 0.1); border: 1px solid rgba(245, 158, 11, 0.35); overflow-wrap: anywhere; }
  .series-strip { padding: 6px 10px; background: #0c0e15; border-bottom: 1px solid var(--border-subtle); display: flex; flex-direction: column; gap: 5px; }
  .primary-cell { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 6px; }
  .metric-label { display: inline-flex; align-items: center; gap: 5px; font-size: 10.5px; font-weight: 600; color: var(--text-main); }
  .swatch { width: 8px; height: 8px; flex-shrink: 0; }
  .metric-nums { display: inline-flex; align-items: center; gap: 8px; font-size: 11px; }
  .val-main { font-weight: 700; color: var(--text-bright); }
  .cursor-date { color: #60a5fa; font-size: 9.5px; font-weight: 500; margin-left: 3px; }
  .val-delta { font-size: 10px; font-weight: 700; padding: 1px 5px; background: rgba(255, 255, 255, 0.04); }
  .val-delta.bull { color: #34d399; }
  .val-delta.bear { color: #f87171; }
  .overlay-pills { display: flex; flex-wrap: wrap; gap: 4px; }
  .ov-pill { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: #94a3b8; padding: 3px 6px; display: inline-flex; align-items: center; gap: 6px; font-size: 9.5px; cursor: pointer; text-align: left; }
  .ov-pill:hover { background: var(--bg-hover); color: var(--text-main); border-color: var(--border-contrast); }
  .ov-pill.selected { border-color: var(--ov-color); background: rgba(59, 130, 246, 0.09); color: var(--text-bright); }
  .ov-head, .ov-vals { display: inline-flex; align-items: center; gap: 4px; }
  .ov-cur { font-weight: 700; color: var(--text-main); }
  .ov-lag { color: #fbbf24; font-weight: 700; }
  .chart-stage { position: relative; height: 225px; width: 100%; background: #090b10; }
  .chart-canvas { width: 100%; height: 100%; }
  .no-points-overlay { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; z-index: 5; pointer-events: none; font-size: 10px; font-weight: 700; color: #94a3b8; background: rgba(9, 11, 16, 0.72); }
  .scope-drawer { padding: 8px 10px; background: #0a0c12; border-top: 1px solid var(--border-contrast); display: flex; flex-direction: column; gap: 6px; }
  .scope-note { font-size: 10.5px; color: #cbd5e1; line-height: 1.45; overflow-wrap: anywhere; }
  .lineage-rows { display: flex; flex-direction: column; gap: 3px; }
  .lineage-row { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 4px 10px; font-size: 9.5px; color: #94a3b8; border-top: 1px solid #161924; padding-top: 3px; }
  .lin-id { font-family: var(--font-mono); color: #93c5fd; font-weight: 600; overflow-wrap: anywhere; }
  .lin-meta { font-family: var(--font-mono); overflow-wrap: anywhere; }
  @media (max-width: 768px) { .scope-toggle, .ov-pill { min-height: 44px; padding: 4px 8px; } }
</style>
