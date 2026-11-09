<script lang="ts">
  import type { InstrumentSummary, TimeframeOption, ChartLegendData } from '../lib/types';
  import Icons from './Icons.svelte';

  export let selectedInstrument: InstrumentSummary;
  export let selectedTimeframe: TimeframeOption;
  export let supportedTimeframes: TimeframeOption[] = ['1d', '1w', '1M'];
  export let pricePrecision: number = 2;
  export let legendData: ChartLegendData | null = null;
  export let isLoading: boolean = false;
  export let onSelectTimeframe: (tf: TimeframeOption) => void;
  export let onOpenSearch: () => void;
  export let onRefresh: () => void;
  export let onOpenCalendar: () => void = () => {};
  export let onOpenPredictions: () => void = () => {};

  const timeframes: TimeframeOption[] = ['1m', '5m', '15m', '1h', '1d', '1w', '1M'];

  function formatNum(val: number | undefined, decimals: number = 2): string {
    if (val === undefined || isNaN(val)) return '-';
    return val.toLocaleString('en-US', {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    });
  }

  function formatVolume(val: number | undefined): string {
    if (val === undefined || isNaN(val)) return '-';
    if (val >= 1_000_000_000) return (val / 1_000_000_000).toFixed(2) + 'B';
    if (val >= 1_000_000) return (val / 1_000_000).toFixed(2) + 'M';
    if (val >= 1_000) return (val / 1_000).toFixed(2) + 'K';
    return val.toFixed(0);
  }
</script>

<header class="header-bar">
  <!-- Left: Symbol Selector & Asset Class -->
  <div class="left-section">
    <button class="ticker-btn" on:click={onOpenSearch} title="Select Ticker (Shortcut: / or Ctrl+K)">
      <Icons name="search" size={14} classNames="icon" />
      <span class="ticker-symbol">{selectedInstrument.canonical_symbol || selectedInstrument.symbol}</span>
      <span class="ticker-name">{selectedInstrument.name}</span>
      <span class="asset-badge">[{selectedInstrument.asset_class.toUpperCase()}]</span>
    </button>

    <div class="divider"></div>

    <!-- Timeframe Selector -->
    <div class="timeframe-group">
      {#each timeframes as tf}
        {@const isSupported = supportedTimeframes.includes(tf)}
        <button
          class="tf-btn"
          class:active={selectedTimeframe === tf}
          class:disabled={!isSupported}
          disabled={!isSupported}
          title={isSupported ? `Switch to ${tf}` : `${tf} not supported for ${selectedInstrument.symbol}`}
          on:click={() => isSupported && onSelectTimeframe(tf)}
        >
          {tf}
        </button>
      {/each}
    </div>
  </div>

  <!-- Center: Crosshair / Candle Confirmation Metrics -->
  <div class="center-section">
    {#if legendData}
      <div class="ohlcv-legend tabular-nums">
        <span class="metric-label">TIME</span>
        <span class="metric-val">{legendData.time}</span>
        <span class="metric-label">O</span>
        <span class="metric-val">{formatNum(legendData.open, pricePrecision)}</span>
        <span class="metric-label">H</span>
        <span class="metric-val">{formatNum(legendData.high, pricePrecision)}</span>
        <span class="metric-label">L</span>
        <span class="metric-val">{formatNum(legendData.low, pricePrecision)}</span>
        <span class="metric-label">C</span>
        <span class="metric-val" class:bull={legendData.isUp} class:bear={!legendData.isUp}>
          {formatNum(legendData.close, pricePrecision)}
        </span>
        <span class="change-tag" class:bull={legendData.isUp} class:bear={!legendData.isUp}>
          {legendData.change >= 0 ? '+' : ''}{formatNum(legendData.change, pricePrecision)} ({legendData.changePercent >= 0 ? '+' : ''}{formatNum(legendData.changePercent, 2)}%)
        </span>
        <span class="metric-label">VOL</span>
        <span class="metric-val">{formatVolume(legendData.volume)}</span>
      </div>
    {:else}
      <div class="legend-placeholder">
        <Icons name="layers" size={13} classNames="icon-dim" />
        <span>PARQUET LAKEHOUSE CONFIRMED SERIES</span>
      </div>
    {/if}
  </div>

  <!-- Right: Integrity Badge & Controls -->
  <div class="right-section">
    <button class="nav-tool-btn" on:click={onOpenCalendar} title="Macroeconomic Calendar (SDMX / Point-in-Time)">
      <Icons name="calendar" size={12} />
      <span>CALENDAR</span>
    </button>

    <button class="nav-tool-btn" on:click={onOpenPredictions} title="Prediction Markets (Kalshi / Polymarket)">
      <Icons name="activity" size={12} />
      <span>PREDICTIONS</span>
    </button>

    <div class="status-badge">
      <Icons name="check" size={13} classNames="badge-icon" />
      <span class="badge-text">LAKE SETTLED</span>
    </div>

    <button
      class="action-btn"
      class:spinning={isLoading}
      on:click={onRefresh}
      title="Reload lakehouse candles"
    >
      <Icons name="refresh" size={13} />
    </button>
  </div>
</header>

<style>
  .header-bar {
    height: 38px;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    gap: 8px;
    font-size: 11px;
    flex-shrink: 0;
  }

  .left-section, .right-section, .center-section {
    display: flex;
    align-items: center;
    height: 100%;
  }

  .left-section {
    gap: 6px;
    flex-shrink: 0;
  }

  .ticker-btn {
    background: var(--bg-surface);
    border: 1px solid var(--border-contrast);
    color: var(--text-bright);
    height: 26px;
    padding: 0 8px;
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    outline: none;
    transition: background 0.1s, border-color 0.1s;
  }

  .ticker-btn:hover {
    background: var(--bg-hover);
    border-color: var(--border-active);
  }

  .ticker-symbol {
    font-weight: 700;
    color: var(--text-bright);
    letter-spacing: 0.5px;
  }

  .ticker-name {
    color: var(--text-muted);
    font-size: 10px;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .asset-badge {
    color: var(--accent-amber);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.5px;
  }

  .divider {
    width: 1px;
    height: 18px;
    background: var(--border-subtle);
    margin: 0 2px;
  }

  .timeframe-group {
    display: flex;
    border: 1px solid var(--border-subtle);
    height: 26px;
  }

  .tf-btn {
    background: var(--bg-panel);
    border: none;
    border-right: 1px solid var(--border-subtle);
    color: var(--text-muted);
    height: 100%;
    padding: 0 7px;
    font-size: 10.5px;
    cursor: pointer;
    font-weight: 500;
    outline: none;
  }

  .tf-btn:last-child {
    border-right: none;
  }

  .tf-btn:hover {
    background: var(--bg-hover);
    color: var(--text-main);
  }

  .tf-btn.active {
    background: var(--bg-active);
    color: var(--text-bright);
    font-weight: 700;
  }

  .tf-btn.disabled {
    opacity: 0.25;
    cursor: not-allowed;
    color: var(--text-muted);
  }

  .center-section {
    flex: 1;
    overflow: hidden;
    padding: 0 8px;
  }

  .ohlcv-legend {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    white-space: nowrap;
    overflow: hidden;
  }

  .metric-label {
    color: var(--text-muted);
    font-weight: 600;
  }

  .metric-val {
    color: var(--text-bright);
    font-weight: 500;
  }

  .bull {
    color: var(--bull-color) !important;
  }

  .bear {
    color: var(--bear-color) !important;
  }

  .change-tag {
    font-weight: 600;
    padding: 1px 4px;
    background: rgba(255, 255, 255, 0.04);
  }

  .legend-placeholder {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
    font-size: 10px;
    letter-spacing: 0.5px;
  }

  .right-section {
    gap: 6px;
    flex-shrink: 0;
  }

  .status-badge {
    background: rgba(8, 153, 129, 0.1);
    border: 1px solid rgba(8, 153, 129, 0.3);
    color: var(--bull-color);
    height: 24px;
    padding: 0 6px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.5px;
  }

  .nav-tool-btn {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    height: 26px;
    padding: 0 7px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    border-radius: 0px;
    outline: none;
  }

  .nav-tool-btn:hover {
    background: var(--bg-hover);
    color: var(--text-bright);
    border-color: var(--border-contrast);
  }

  .action-btn {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    outline: none;
  }

  .action-btn:hover {
    background: var(--bg-hover);
    color: var(--text-bright);
    border-color: var(--border-contrast);
  }

  .spinning :global(svg) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
