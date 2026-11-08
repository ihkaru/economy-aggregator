import type { CandlestickData, HistogramData, Time } from 'lightweight-charts';
import type { AggregatorCandle } from './types';

export interface PreparedChartData {
  candles: CandlestickData<Time>[];
  volumes: HistogramData<Time>[];
}

/**
 * Sanitizes, deduplicates, and sorts raw AggregatorCandle records for Lightweight Charts.
 * Invariant: Lightweight Charts strictly requires ascending timestamps with zero duplicates.
 */
export function prepareChartSeriesData(candles: AggregatorCandle[]): PreparedChartData {
  if (!candles || candles.length === 0) {
    return { candles: [], volumes: [] };
  }

  const seenSecs = new Set<number>();
  const validPairs: { bar: CandlestickData<Time>; vol: HistogramData<Time>; sec: number }[] = [];

  for (const c of candles) {
    if (
      typeof c.open !== 'number' ||
      typeof c.high !== 'number' ||
      typeof c.low !== 'number' ||
      typeof c.close !== 'number' ||
      isNaN(c.open) ||
      isNaN(c.high) ||
      isNaN(c.low) ||
      isNaN(c.close)
    ) {
      continue;
    }

    const sec =
      typeof c.time === 'number' && c.time > 0
        ? c.time
        : Math.floor(new Date(c.timestamp_utc).getTime() / 1000);

    if (isNaN(sec) || sec <= 0 || seenSecs.has(sec)) {
      continue;
    }
    seenSecs.add(sec);

    const isUp = c.close >= c.open;
    validPairs.push({
      sec,
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

  // Strict ascending order by Unix timestamp seconds
  validPairs.sort((a, b) => a.sec - b.sec);

  return {
    candles: validPairs.map((p) => p.bar),
    volumes: validPairs.map((p) => p.vol),
  };
}
