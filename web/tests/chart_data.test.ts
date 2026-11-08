import { describe, expect, it } from 'bun:test';
import { prepareChartSeriesData } from '../src/lib/chart_utils';
import type { AggregatorCandle } from '../src/lib/types';
import { CandlestickSeries, HistogramSeries, createChart } from 'lightweight-charts';

describe('Lightweight Charts v5 Contract & Module Verification', () => {
  it('should export CandlestickSeries and HistogramSeries definitions', () => {
    expect(CandlestickSeries).toBeDefined();
    expect(HistogramSeries).toBeDefined();
    expect(typeof createChart).toBe('function');
  });
});

describe('prepareChartSeriesData (Deterministic Sanity & Sorting)', () => {
  it('returns empty series when candle array is empty or undefined', () => {
    const res = prepareChartSeriesData([]);
    expect(res.candles).toEqual([]);
    expect(res.volumes).toEqual([]);
  });

  it('correctly parses integer Unix seconds and sorts in ascending order', () => {
    const rawCandles: AggregatorCandle[] = [
      {
        time: 1700000200,
        timestamp_utc: '2023-11-14T20:30:00Z',
        canonical_symbol: 'BBCA',
        timeframe: '1d',
        open: 9000,
        high: 9100,
        low: 8950,
        close: 9050,
        volume_base: 50000,
        volume_quote: 450000000,
        source: 'yahoo',
        raw_symbol: 'BBCA.JK',
        ingestion_mode: 'lakehouse',
        is_settled: true,
        price_type: 'last',
      },
      {
        time: 1700000100, // Earlier timestamp listed second
        timestamp_utc: '2023-11-14T20:28:20Z',
        canonical_symbol: 'BBCA',
        timeframe: '1d',
        open: 8900,
        high: 8950,
        low: 8850,
        close: 8875,
        volume_base: 25000,
        volume_quote: 221875000,
        source: 'yahoo',
        raw_symbol: 'BBCA.JK',
        ingestion_mode: 'lakehouse',
        is_settled: true,
        price_type: 'last',
      },
    ];

    const { candles, volumes } = prepareChartSeriesData(rawCandles);

    expect(candles.length).toBe(2);
    expect(volumes.length).toBe(2);

    // Verify ascending order
    expect(Number(candles[0].time)).toBe(1700000100);
    expect(Number(candles[1].time)).toBe(1700000200);

    // Verify OHLC values
    expect(candles[0].open).toBe(8900);
    expect(candles[0].close).toBe(8875);
    expect(candles[1].open).toBe(9000);
    expect(candles[1].close).toBe(9050);

    // Verify volume colors (red for down, green for up)
    expect(volumes[0].color).toBe('rgba(242, 54, 69, 0.4)'); // 8875 < 8900
    expect(volumes[1].color).toBe('rgba(8, 153, 129, 0.4)'); // 9050 >= 9000
  });

  it('deduplicates identical timestamps to prevent lightweight-charts crash', () => {
    const rawCandles: AggregatorCandle[] = [
      {
        time: 1700000100,
        timestamp_utc: '2023-11-14T20:28:20Z',
        canonical_symbol: 'BBCA',
        timeframe: '1d',
        open: 8900,
        high: 8950,
        low: 8850,
        close: 8950,
        volume_base: 25000,
        volume_quote: 221875000,
        source: 'yahoo',
        raw_symbol: 'BBCA.JK',
        ingestion_mode: 'lakehouse',
        is_settled: true,
        price_type: 'last',
      },
      {
        time: 1700000100, // Exact duplicate timestamp
        timestamp_utc: '2023-11-14T20:28:20Z',
        canonical_symbol: 'BBCA',
        timeframe: '1d',
        open: 8900,
        high: 8950,
        low: 8850,
        close: 8950,
        volume_base: 25000,
        volume_quote: 221875000,
        source: 'yahoo',
        raw_symbol: 'BBCA.JK',
        ingestion_mode: 'lakehouse',
        is_settled: true,
        price_type: 'last',
      },
    ];

    const { candles, volumes } = prepareChartSeriesData(rawCandles);
    expect(candles.length).toBe(1);
    expect(volumes.length).toBe(1);
  });

  it('filters out corrupt records with NaN open/close or invalid timestamps', () => {
    const rawCandles: any[] = [
      { time: 1700000100, open: NaN, close: 100, high: 100, low: 90 },
      { time: 1700000200, open: 100, close: NaN, high: 100, low: 90 },
      { time: -1, open: 100, close: 105, high: 110, low: 95 },
      { time: 1700000300, open: 100, close: 105, high: 110, low: 95, volume_base: 1000 },
    ];

    const { candles } = prepareChartSeriesData(rawCandles);
    expect(candles.length).toBe(1);
    expect(Number(candles[0].time)).toBe(1700000300);
  });

  it('falls back to timestamp_utc if time field is 0 or undefined', () => {
    const rawCandles: AggregatorCandle[] = [
      {
        time: 0,
        timestamp_utc: '2023-11-14T00:00:00Z',
        canonical_symbol: 'BBCA',
        timeframe: '1d',
        open: 9000,
        high: 9100,
        low: 8950,
        close: 9050,
        volume_base: 50000,
        volume_quote: 450000000,
        source: 'yahoo',
        raw_symbol: 'BBCA.JK',
        ingestion_mode: 'lakehouse',
        is_settled: true,
        price_type: 'last',
      },
    ];

    const { candles } = prepareChartSeriesData(rawCandles);
    expect(candles.length).toBe(1);
    const expectedSec = Math.floor(new Date('2023-11-14T00:00:00Z').getTime() / 1000);
    expect(Number(candles[0].time)).toBe(expectedSec);
  });
});
