<script lang="ts">
  import { onMount } from 'svelte';
  import HeaderBar from './components/HeaderBar.svelte';
  import CandleChart from './components/CandleChart.svelte';
  import TickerModal from './components/TickerModal.svelte';
  import InspectionBar from './components/InspectionBar.svelte';
  import Icons from './components/Icons.svelte';
  import { fetchInstruments, fetchCandles } from './lib/api';
  import type { InstrumentSummary, TimeframeOption, AggregatorCandle, ChartLegendData } from './lib/types';

  let instruments: InstrumentSummary[] = [];
  let selectedInstrument: InstrumentSummary = {
    symbol: 'BBCA',
    canonical_symbol: 'BBCA',
    name: 'Bank Central Asia',
    asset_class: 'equities',
    source: 'yahoo',
    timeframes: ['1d'],
  };
  let selectedTimeframe: TimeframeOption = '1d';
  let candles: AggregatorCandle[] = [];
  let legendData: ChartLegendData | null = null;
  let isModalOpen: boolean = false;
  let isLoading: boolean = false;
  let errorMsg: string | null = null;

  $: supportedTimeframes = (selectedInstrument.supported_timeframes as TimeframeOption[]) || ['1d', '1w', '1M'];
  $: pricePrecision = typeof selectedInstrument.price_precision === 'number' ? selectedInstrument.price_precision : 2;
  $: minMove = typeof selectedInstrument.min_tick === 'number' ? selectedInstrument.min_tick : 0.01;

  onMount(async () => {
    try {
      instruments = await fetchInstruments();
      if (instruments.length > 0) {
        selectedInstrument = instruments[0];
      }
      await loadCandles();
    } catch (e: any) {
      errorMsg = e.message || 'Failed to initialize terminal';
    }

    // Global keyboard listener
    const handleKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === 'r' || e.key === 'R') {
        loadCandles();
      } else if (e.key === 'd' || e.key === 'D') {
        handleTimeframeChange('1d');
      } else if (e.key === 'w' || e.key === 'W') {
        handleTimeframeChange('1w');
      } else if (e.key === 'm' || e.key === 'M') {
        handleTimeframeChange('1M');
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  });

  async function loadCandles() {
    isLoading = true;
    errorMsg = null;
    try {
      candles = await fetchCandles(
        selectedInstrument.canonical_symbol || selectedInstrument.symbol,
        selectedInstrument.asset_class,
        selectedTimeframe
      );
    } catch (err: any) {
      errorMsg = err.message || 'Error loading candle series';
    } finally {
      isLoading = false;
    }
  }

  function handleSelectInstrument(inst: InstrumentSummary) {
    selectedInstrument = inst;
    const supported = (inst.supported_timeframes as TimeframeOption[]) || ['1d', '1w', '1M'];
    if (!supported.includes(selectedTimeframe)) {
      selectedTimeframe = (inst.default_timeframe as TimeframeOption) || '1d';
    }
    loadCandles();
  }

  function handleTimeframeChange(tf: TimeframeOption) {
    if (!supportedTimeframes.includes(tf)) return;
    selectedTimeframe = tf;
    loadCandles();
  }
</script>

<main class="terminal-container">
  <HeaderBar
    {selectedInstrument}
    {selectedTimeframe}
    {supportedTimeframes}
    {pricePrecision}
    {legendData}
    {isLoading}
    onSelectTimeframe={handleTimeframeChange}
    onOpenSearch={() => (isModalOpen = true)}
    onRefresh={loadCandles}
  />

  <div class="chart-viewport" class:dimmed={isLoading}>
    {#if isLoading}
      <div class="progress-bar-container">
        <div class="progress-bar-indicator"></div>
      </div>
      <div class="loading-hud-overlay">
        <div class="loading-hud-box tabular-nums">
          <div class="hud-spinner">
            <Icons name="loader" size={15} classNames="spin-icon" />
          </div>
          <div class="hud-content">
            <span class="hud-title">SYNCING LAKEHOUSE</span>
            <span class="hud-meta">
              {selectedInstrument.symbol} · {selectedTimeframe.toUpperCase()} [PARQUET PARTITIONS]
            </span>
          </div>
        </div>
      </div>
    {/if}

    {#if errorMsg}
      <div class="error-banner">
        <span>ERROR: {errorMsg}</span>
        <button on:click={loadCandles} class="retry-btn">RETRY</button>
      </div>
    {/if}

    <CandleChart
      {candles}
      {pricePrecision}
      {minMove}
      timeframe={selectedTimeframe}
      onCrosshairData={(data) => (legendData = data)}
    />
  </div>

  <InspectionBar
    instrument={selectedInstrument}
    {candles}
    timeframe={selectedTimeframe}
    {isLoading}
  />

  <TickerModal
    {instruments}
    isOpen={isModalOpen}
    onSelect={handleSelectInstrument}
    onClose={() => (isModalOpen = false)}
  />
</main>

<style>
  .terminal-container {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    background-color: var(--bg-main);
    overflow: hidden;
  }

  .chart-viewport {
    position: relative;
    flex: 1;
    width: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .chart-viewport.dimmed :global(.chart-canvas) {
    opacity: 0.55;
    pointer-events: none;
    transition: opacity 0.15s ease-out;
  }

  :global(.chart-canvas) {
    transition: opacity 0.15s ease-out;
  }

  /* Indeterminate Scan-Line Progress Bar */
  .progress-bar-container {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 2px;
    background: rgba(41, 98, 255, 0.12);
    z-index: 50;
    overflow: hidden;
  }

  .progress-bar-indicator {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    width: 35%;
    background: linear-gradient(90deg, transparent, #2962ff, #089981, #2962ff, transparent);
    animation: scan-line 1.2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }

  @keyframes scan-line {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(350%); }
  }

  /* Central High-Density Terminal Loading HUD */
  .loading-hud-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 40;
    pointer-events: none;
  }

  .loading-hud-box {
    display: flex;
    align-items: center;
    gap: 12px;
    background: rgba(8, 9, 13, 0.88);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
    border: 1px solid #2962ff;
    padding: 8px 14px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.7);
  }

  .hud-spinner {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #2962ff;
  }

  :global(.spin-icon) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .hud-content {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .hud-title {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.8px;
    color: #60a5fa;
  }

  .hud-meta {
    font-size: 9.5px;
    font-weight: 500;
    color: var(--text-muted);
    letter-spacing: 0.4px;
  }

  .error-banner {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(242, 54, 69, 0.9);
    color: white;
    padding: 4px 12px;
    font-size: 11px;
    font-weight: 600;
    z-index: 100;
    display: flex;
    align-items: center;
    gap: 12px;
    border: 1px solid #f87171;
  }

  .retry-btn {
    background: #000;
    color: #fff;
    border: 1px solid #fff;
    padding: 2px 6px;
    font-size: 10px;
    cursor: pointer;
  }
</style>
