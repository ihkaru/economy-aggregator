import {
  createChart,
  createSeriesMarkers,
  ColorType,
  LineSeries,
  LineStyle,
  type IChartApi,
  type ISeriesApi,
  type ISeriesMarkersPluginApi,
  type LineData,
  type SeriesMarker,
  type Time,
} from 'lightweight-charts';
import type { SmhPredictiveResponse } from './smh_api';

export interface SmhChartHandle {
  update: (data: SmhPredictiveResponse) => void;
  resize: (width: number, height: number) => void;
  destroy: () => void;
}

export function createSmhPriceChart(container: HTMLElement): SmhChartHandle {
  const chart: IChartApi = createChart(container, {
    layout: {
      background: { type: ColorType.Solid, color: '#090d16' },
      textColor: '#94a3b8',
      fontFamily: 'JetBrains Mono, ui-monospace, monospace',
      fontSize: 11,
    },
    grid: {
      vertLines: { color: 'rgba(148, 163, 184, 0.07)' },
      horzLines: { color: 'rgba(148, 163, 184, 0.07)' },
    },
    rightPriceScale: {
      borderColor: 'rgba(148, 163, 184, 0.16)',
    },
    timeScale: {
      borderColor: 'rgba(148, 163, 184, 0.16)',
      timeVisible: true,
    },
    crosshair: {
      vertLine: { color: 'rgba(56, 189, 248, 0.45)', style: LineStyle.Dashed },
      horzLine: { color: 'rgba(56, 189, 248, 0.45)', style: LineStyle.Dashed },
    },
  });

  const priceSeries: ISeriesApi<'Line'> = chart.addSeries(LineSeries, {
    color: '#38bdf8',
    lineWidth: 2,
    title: 'SMH Close',
  });
  const markersPlugin: ISeriesMarkersPluginApi<Time> = createSeriesMarkers(priceSeries, []);
  const predSeries: ISeriesApi<'Line'> = chart.addSeries(LineSeries, {
    color: '#f59e0b',
    lineWidth: 2,
    lineStyle: LineStyle.Dashed,
    title: '5d Forecast',
  });
  const upperSeries: ISeriesApi<'Line'> = chart.addSeries(LineSeries, {
    color: 'rgba(16, 185, 129, 0.65)',
    lineWidth: 1,
    lineStyle: LineStyle.Dotted,
    title: 'Upper 95%',
  });
  const lowerSeries: ISeriesApi<'Line'> = chart.addSeries(LineSeries, {
    color: 'rgba(244, 63, 94, 0.65)',
    lineWidth: 1,
    lineStyle: LineStyle.Dotted,
    title: 'Lower 95%',
  });

  return {
    update(data: SmhPredictiveResponse) {
      const actualData: LineData[] = data.history.map((p) => ({
        time: p.time as Time,
        value: p.close,
      }));
      const predData: LineData[] = [];
      const upData: LineData[] = [];
      const loData: LineData[] = [];
      const markers: SeriesMarker<Time>[] = [];

      let prevSig: string | null = null;
      for (const p of data.history) {
        if (p.predicted_5d_price !== null) {
          predData.push({ time: p.time as Time, value: p.predicted_5d_price });
        }
        if (p.upper_95_price !== null) {
          upData.push({ time: p.time as Time, value: p.upper_95_price });
        }
        if (p.lower_95_price !== null) {
          loData.push({ time: p.time as Time, value: p.lower_95_price });
        }
        if (p.signal && p.signal !== prevSig && p.signal !== 'HOLD') {
          markers.push({
            time: p.time as Time,
            position: p.signal === 'BUY' ? 'belowBar' : 'aboveBar',
            color: p.signal === 'BUY' ? '#10b981' : '#f43f5e',
            shape: p.signal === 'BUY' ? 'arrowUp' : 'arrowDown',
            text: p.signal,
          });
        }
        if (p.signal) prevSig = p.signal;
      }

      // Append 5-day forward projection cone beyond as_of
      for (let i = 1; i < data.forward_cone.length; i++) {
        const fc = data.forward_cone[i];
        predData.push({ time: fc.time as Time, value: fc.projected_price });
        upData.push({ time: fc.time as Time, value: fc.upper_95 });
        loData.push({ time: fc.time as Time, value: fc.lower_95 });
      }

      priceSeries.setData(actualData);
      markersPlugin.setMarkers(markers);
      predSeries.setData(predData);
      upperSeries.setData(upData);
      lowerSeries.setData(loData);
      chart.timeScale().fitContent();
    },
    resize(width: number, height: number) {
      chart.applyOptions({ width, height });
    },
    destroy() {
      chart.remove();
    },
  };
}
