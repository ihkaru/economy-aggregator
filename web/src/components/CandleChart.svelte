<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    createChart,
    CandlestickSeries,
    HistogramSeries,
    ColorType,
    CrosshairMode,
    type IChartApi,
    type ISeriesApi,
    type CandlestickData,
    type HistogramData,
    type Time,
  } from 'lightweight-charts';
  import type { AggregatorCandle, ChartLegendData } from '../lib/types';
  import { prepareChartSeriesData } from '../lib/chart_utils';

  export let candles: AggregatorCandle[] = [];
  export let onCrosshairData: (data: ChartLegendData | null) => void;
  export let pricePrecision: number = 2;
  export let minMove: number = 0.01;
  export let timeframe: string = '1d';

  let chartContainer: HTMLDivElement;
  let chart: IChartApi | null = null;
  let candleSeries: ISeriesApi<'Candlestick'> | null = null;
  let volumeSeries: ISeriesApi<'Histogram'> | null = null;

  $: isIntraday = timeframe === '1m' || timeframe === '5m' || timeframe === '15m' || timeframe === '1h';

  $: if (chart) {
    chart.applyOptions({
      timeScale: {
        timeVisible: isIntraday,
      },
    });
  }

  $: if (candleSeries) {
    candleSeries.applyOptions({
      priceFormat: {
        type: 'price',
        precision: pricePrecision,
        minMove: minMove,
      },
    });
  }

  onMount(() => {
    initChart();
    if (candles.length > 0) {
      updateSeriesData();
    }
  });

  onDestroy(() => {
    if (chart) {
      chart.remove();
      chart = null;
    }
  });

  $: if (candles && candleSeries) {
    updateSeriesData();
  }

  function initChart() {
    if (!chartContainer) return;

    chart = createChart(chartContainer, {
      autoSize: true,
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
        timeVisible: isIntraday,
        secondsVisible: false,
        borderVisible: true,
        barSpacing: 9,
        minBarSpacing: 3,
        rightOffset: 3,
      },
      rightPriceScale: {
        borderColor: '#1c202d',
        autoScale: true,
        borderVisible: true,
        scaleMargins: {
          top: 0.1,
          bottom: 0.22,
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

    candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#089981',
      downColor: '#f23645',
      borderVisible: false,
      wickUpColor: '#089981',
      wickDownColor: '#f23645',
      priceFormat: {
        type: 'price',
        precision: pricePrecision,
        minMove: minMove,
      },
    });

    volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    });

    chart.priceScale('volume').applyOptions({
      scaleMargins: {
        top: 0.82,
        bottom: 0,
      },
    });

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
        const timeSec = typeof param.time === 'number'
          ? param.time
          : Math.floor(new Date(param.time as string).getTime() / 1000);
        const dateStr = new Date(timeSec * 1000).toISOString().split('T')[0];

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
  }

  function updateSeriesData() {
    if (!candleSeries || !volumeSeries) return;

    if (candles.length === 0) {
      candleSeries.setData([]);
      volumeSeries.setData([]);
      return;
    }

    const { candles: candleData, volumes: volumeData } = prepareChartSeriesData(candles);
    if (candleData.length === 0) {
      candleSeries.setData([]);
      volumeSeries.setData([]);
      return;
    }

    candleSeries.setData(candleData);
    volumeSeries.setData(volumeData);

    // TradingView Best Practice: Display the latest ~90 to 120 bars so candles have natural size
    const totalBars = candleData.length;
    const visibleCount = Math.min(100, totalBars);
    chart?.timeScale().setVisibleLogicalRange({
      from: totalBars - visibleCount,
      to: totalBars + 2,
    });
  }
</script>

<div class="chart-wrapper">
  <div bind:this={chartContainer} class="chart-canvas"></div>
</div>

<style>
  .chart-wrapper {
    position: relative;
    flex: 1;
    width: 100%;
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background-color: var(--bg-main);
    overflow: hidden;
  }

  .chart-canvas {
    flex: 1;
    width: 100%;
    height: 100%;
    min-height: 0;
  }
</style>
