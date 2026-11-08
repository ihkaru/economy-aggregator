<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    createChart,
    ColorType,
    CrosshairMode,
    type IChartApi,
    type ISeriesApi,
    type CandlestickData,
    type HistogramData,
    type Time,
  } from 'lightweight-charts';
  import type { AggregatorCandle, ChartLegendData } from '../lib/types';

  export let candles: AggregatorCandle[] = [];
  export let onCrosshairData: (data: ChartLegendData | null) => void;

  let chartContainer: HTMLDivElement;
  let chart: IChartApi | null = null;
  let candleSeries: ISeriesApi<'Candlestick'> | null = null;
  let volumeSeries: ISeriesApi<'Histogram'> | null = null;
  let resizeObserver: ResizeObserver | null = null;

  onMount(() => {
    initChart();
    if (candles.length > 0) {
      updateSeriesData();
    }
  });

  onDestroy(() => {
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
    if (chart) {
      chart.remove();
    }
  });

  $: if (candles && candleSeries) {
    updateSeriesData();
  }

  function initChart() {
    if (!chartContainer) return;

    chart = createChart(chartContainer, {
      layout: {
        background: { type: ColorType.Solid, color: '#08090d' },
        textColor: '#828ba2',
        fontSize: 11,
        fontFamily: "'Google Sans', 'Google Sans Text', sans-serif",
      },
      grid: {
        vertLines: { color: '#161924', style: 2 },
        horzLines: { color: '#161924', style: 2 },
      },
      crosshair: {
        mode: CrosshairMode.Normal,
        vertLine: {
          color: '#3d445c',
          width: 1,
          style: 3,
          labelBackgroundColor: '#1b1f2e',
        },
        horzLine: {
          color: '#3d445c',
          width: 1,
          style: 3,
          labelBackgroundColor: '#1b1f2e',
        },
      },
      timeScale: {
        borderColor: '#1c202d',
        timeVisible: true,
        secondsVisible: false,
        borderVisible: true,
      },
      rightPriceScale: {
        borderColor: '#1c202d',
        autoScale: true,
        borderVisible: true,
        scaleMargins: {
          top: 0.1,
          bottom: 0.2, // leave 20% margin at bottom for volume
        },
      },
      handleScale: {
        mouseWheel: true,
        pinch: true,
        axisPressedMouseMove: true,
      },
      handleScroll: {
        mouseWheel: true,
        pressedMouseMove: true,
        horzTouchDrag: true,
        vertTouchDrag: false,
      },
    });

    candleSeries = chart.addCandlestickSeries({
      upColor: '#089981',
      downColor: '#f23645',
      borderVisible: false,
      wickUpColor: '#089981',
      wickDownColor: '#f23645',
    });

    volumeSeries = chart.addHistogramSeries({
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume', // separate internal price scale for volume overlay
    });

    chart.priceScale('volume').applyOptions({
      scaleMargins: {
        top: 0.82, // position volume strictly in lower 18% of chart
        bottom: 0,
      },
    });

    // Crosshair listener for legend update
    chart.subscribeCrosshairMove((param) => {
      if (!param || !param.time || !param.seriesData || !candleSeries) {
        onCrosshairData(null);
        return;
      }

      const barData = param.seriesData.get(candleSeries) as CandlestickData<Time> | undefined;
      const volData = volumeSeries ? (param.seriesData.get(volumeSeries) as HistogramData<Time> | undefined) : undefined;

      if (barData && 'open' in barData) {
        const change = barData.close - barData.open;
        const changePercent = barData.open !== 0 ? (change / barData.open) * 100 : 0;
        const dateStr = typeof param.time === 'string'
          ? param.time
          : new Date((param.time as number) * 1000).toISOString().split('T')[0];

        onCrosshairData({
          time: dateStr,
          open: barData.open,
          high: barData.high,
          low: barData.low,
          close: barData.close,
          volume: volData && 'value' in volData ? volData.value : 0,
          change,
          changePercent,
          isUp: barData.close >= barData.open,
        });
      } else {
        onCrosshairData(null);
      }
    });

    // Resize handling
    resizeObserver = new ResizeObserver((entries) => {
      if (entries.length === 0 || !entries[0].contentRect) return;
      const { width, height } = entries[0].contentRect;
      chart?.applyOptions({ width, height });
    });
    resizeObserver.observe(chartContainer);
  }

  function updateSeriesData() {
    if (!candleSeries || !volumeSeries || candles.length === 0) return;

    const seenSecs = new Set<number>();
    const validPairs: { bar: CandlestickData<Time>; vol: HistogramData<Time> }[] = [];

    for (const c of candles) {
      if (typeof c.open !== 'number' || typeof c.close !== 'number' || isNaN(c.open) || isNaN(c.close)) continue;
      const dt = new Date(c.timestamp_utc);
      const sec = Math.floor(dt.getTime() / 1000);
      if (isNaN(sec) || seenSecs.has(sec)) continue;
      seenSecs.add(sec);

      const isUp = c.close >= c.open;
      validPairs.push({
        bar: {
          time: sec as unknown as Time,
          open: c.open,
          high: c.high,
          low: c.low,
          close: c.close,
        },
        vol: {
          time: sec as unknown as Time,
          value: c.volume_base || 0,
          color: isUp ? 'rgba(8, 153, 129, 0.4)' : 'rgba(242, 54, 69, 0.4)',
        },
      });
    }

    // Lightweight charts requires strictly ascending order by time
    validPairs.sort((a, b) => (a.bar.time as number) - (b.bar.time as number));
    const chartBars = validPairs.map((p) => p.bar);
    const volumeBars = validPairs.map((p) => p.vol);

    candleSeries.setData(chartBars);
    volumeSeries.setData(volumeBars);
    chart?.timeScale().fitContent();
  }
</script>

<div class="chart-wrapper">
  <div bind:this={chartContainer} class="chart-canvas"></div>
</div>

<style>
  .chart-wrapper {
    position: relative;
    width: 100%;
    height: 100%;
    background-color: var(--bg-main);
    overflow: hidden;
  }

  .chart-canvas {
    width: 100%;
    height: 100%;
  }
</style>
