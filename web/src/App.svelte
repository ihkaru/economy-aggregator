<script lang="ts">
  import { onMount } from 'svelte';
  import HeaderBar from './components/HeaderBar.svelte';
  import CandleChart from './components/CandleChart.svelte';
  import TickerModal from './components/TickerModal.svelte';
  import InspectionBar from './components/InspectionBar.svelte';
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

  <div class="chart-viewport">
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
