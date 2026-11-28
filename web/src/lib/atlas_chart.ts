import {
  AreaSeries,
  ColorType,
  CrosshairMode,
  HistogramSeries,
  LineSeries,
  createChart,
  type IChartApi,
  type ISeriesApi,
  type Time,
} from 'lightweight-charts';
import type { AtlasPoint, AtlasSeries } from './atlas_types';

export type TankSeriesApi = ISeriesApi<'Histogram' | 'Line' | 'Area'>;

export function normalizeChartTime(t: Time | undefined): string | null {
  if (!t) return null;
  if (typeof t === 'string') return t;
  if (typeof t === 'number') return new Date(t * 1000).toISOString().slice(0, 10);
  if (typeof t === 'object' && 'year' in t) {
    return `${t.year}-${String(t.month).padStart(2, '0')}-${String(t.day).padStart(2, '0')}`;
  }
  return null;
}

export function findPointAtOrBefore(
  points: AtlasPoint[],
  dateStr: string
): { point: AtlasPoint; exact: boolean } | null {
  if (!points || points.length === 0) return null;
  let lo = 0;
  let hi = points.length - 1;
  let ans = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (points[mid].time <= dateStr) {
      ans = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  if (ans < 0) return null;
  const point = points[ans];
  return { point, exact: point.time === dateStr };
}

export function createTankChart(container: HTMLElement): IChartApi {
  return createChart(container, {
    autoSize: true,
    layout: {
      background: { type: ColorType.Solid, color: '#090b10' },
      textColor: '#94a3b8',
      fontSize: 10.5,
      fontFamily: "'Google Sans', 'Google Sans Text', sans-serif",
    },
    grid: {
      vertLines: { color: '#141824', style: 2 },
      horzLines: { color: '#141824', style: 2 },
    },
    crosshair: {
      mode: CrosshairMode.Normal,
      vertLine: { color: '#3b82f6', width: 1, style: 3, labelBackgroundColor: '#1b1f2e' },
      horzLine: { color: '#3d445c', width: 1, style: 3, labelBackgroundColor: '#1b1f2e' },
    },
    rightPriceScale: { borderColor: '#1c202d', autoScale: true, scaleMargins: { top: 0.12, bottom: 0.12 } },
    leftPriceScale: { visible: true, borderColor: '#1c202d', autoScale: true, scaleMargins: { top: 0.15, bottom: 0.15 } },
    timeScale: { borderColor: '#1c202d', timeVisible: false, fixLeftEdge: true, rightOffset: 2 },
    handleScale: { mouseWheel: true, pinch: true, axisPressedMouseMove: true },
    handleScroll: { mouseWheel: true, pressedMouseMove: true, horzTouchDrag: true, vertTouchDrag: false },
  });
}

export function addAtlasSeriesToChart(
  chart: IChartApi,
  s: AtlasSeries,
  priceScaleId: 'right' | 'left',
  isOverlay: boolean
): TankSeriesApi {
  const baseOpts = {
    priceScaleId,
    lastValueVisible: true,
    priceLineVisible: !isOverlay,
  };
  if (s.chart_kind === 'histogram') {
    const api = chart.addSeries(HistogramSeries, { ...baseOpts, color: s.color, base: 0 });
    api.setData(
      s.points.map((p) => ({
        time: p.time as Time,
        value: p.value,
        color: p.value < 0 ? '#f23645' : s.color,
      }))
    );
    if (!isOverlay) {
      api.createPriceLine({
        price: 0,
        color: '#2a3044',
        lineWidth: 1,
        lineStyle: 2,
        axisLabelVisible: false,
      });
    }
    return api;
  }
  if (s.chart_kind === 'area') {
    const api = chart.addSeries(AreaSeries, {
      ...baseOpts,
      lineColor: s.color,
      topColor: `${s.color}40`,
      bottomColor: `${s.color}05`,
      lineWidth: 2,
    });
    api.setData(s.points.map((p) => ({ time: p.time as Time, value: p.value })));
    return api;
  }
  const api = chart.addSeries(LineSeries, { ...baseOpts, color: s.color, lineWidth: 2 });
  api.setData(s.points.map((p) => ({ time: p.time as Time, value: p.value })));
  return api;
}
