<script lang="ts">
  import type { AggregatorCandle, InstrumentSummary } from '../lib/types';
  import Icons from './Icons.svelte';

  export let instrument: InstrumentSummary;
  export let candles: AggregatorCandle[] = [];
  export let timeframe: string;
  export let isLoading: boolean = false;

  function formatTimestamp(c: AggregatorCandle | undefined): string {
    if (!c) return '-';
    if (typeof c.time === 'number' && c.time > 0) {
      return new Date(c.time * 1000).toISOString().split('T')[0];
    }
    if (c.timestamp_utc) {
      return c.timestamp_utc.split('T')[0];
    }
    return '-';
  }

  $: count = candles.length;
  $: firstDate = candles.length > 0 ? formatTimestamp(candles[0]) : '-';
  $: lastDate = candles.length > 0 ? formatTimestamp(candles[candles.length - 1]) : '-';
  $: settledCount = candles.filter((c) => c.is_settled).length;
  $: settledPct = count > 0 ? Math.round((settledCount / count) * 100) : 0;
  $: activeSource = (candles.length > 0 && candles[0].source && candles[0].source !== 'parquet_lake')
    ? candles[0].source
    : (instrument.source || 'YAHOO');
</script>

<footer class="inspection-bar tabular-nums">
  <!-- Left: Data counts and date span -->
  <div class="left-stats">
    <div class="stat-pill">
      <span class="icon"><Icons name="database" size={12} /></span>
      <span class="label">LAKEBARS:</span>
      <span class="value">{count}</span>
    </div>

    <div class="stat-pill">
      <span class="icon"><Icons name="calendar" size={12} /></span>
      <span class="label">RANGE:</span>
      <span class="value">{firstDate} .. {lastDate}</span>
    </div>

    <div class="stat-pill">
      <span class="icon"><Icons name="activity" size={12} /></span>
      <span class="label">SETTLED:</span>
      <span class="value">{settledCount}/{count} ({settledPct}%)</span>
    </div>
  </div>

  <!-- Center: Shortcuts guide -->
  <div class="center-guide">
    <span class="shortcut">[/] SEARCH</span>
    <span class="shortcut-dot">·</span>
    <span class="shortcut">[D/W/M] TIMEFRAME</span>
    <span class="shortcut-dot">·</span>
    <span class="shortcut">[R] REFRESH</span>
  </div>

  <!-- Right: Storage & Environment info -->
  <div class="right-stats">
    {#if isLoading}
      <span class="sync-tag">
        <span class="sync-dot"></span>
        SYNCING LAKE...
      </span>
    {/if}
    <span class="env-tag">PARQUET L1</span>
    <span class="source-tag">{activeSource.toUpperCase()}</span>
    <span class="tf-tag">{timeframe.toUpperCase()}</span>
  </div>
</footer>

<style>
  .inspection-bar {
    height: 24px;
    background: var(--bg-panel);
    border-top: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px;
    font-size: 10px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .left-stats, .right-stats, .center-guide {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .stat-pill {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .icon {
    color: var(--text-muted);
  }

  .label {
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.5px;
  }

  .value {
    color: var(--text-bright);
    font-weight: 600;
  }

  .center-guide {
    font-size: 9.5px;
    color: #555e75;
    letter-spacing: 0.5px;
  }

  .shortcut {
    font-weight: 500;
  }

  .shortcut-dot {
    color: var(--border-contrast);
  }

  .right-stats {
    gap: 6px;
  }

  .sync-tag {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: rgba(41, 98, 255, 0.15);
    color: #60a5fa;
    border: 1px solid rgba(41, 98, 255, 0.4);
    padding: 1px 5px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
  }

  .sync-dot {
    width: 5px;
    height: 5px;
    background: #3b82f6;
    animation: sync-pulse 1s infinite;
  }

  @keyframes sync-pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.3; transform: scale(0.8); }
  }

  .env-tag {
    background: rgba(59, 130, 246, 0.1);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.25);
    padding: 1px 4px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
  }

  .source-tag {
    background: var(--bg-surface);
    color: var(--text-main);
    border: 1px solid var(--border-subtle);
    padding: 1px 4px;
    font-size: 9px;
    font-weight: 600;
  }

  .tf-tag {
    background: var(--bg-surface);
    color: var(--accent-amber);
    border: 1px solid var(--border-subtle);
    padding: 1px 4px;
    font-size: 9px;
    font-weight: 700;
  }
</style>
