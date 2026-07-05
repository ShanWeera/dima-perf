/**
 * DiMA Desktop - Entropy Line Chart
 * 
 * Interactive line chart showing entropy values across positions.
 * Features: zoom/pan, click to select position, average line, markers.
 * Includes integrated heatmap as zoom/overview control.
 */

import { useMemo, useRef, useCallback, memo } from 'react';
import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { Position, Annotation } from '@/lib/types';
import { ANNOTATION_COLORS } from '@/lib/types';
import { useSettingsStore } from '@/stores/settingsStore';
import { ENTROPY_COLORS } from '@/lib/colors';
import { useChartTheme } from '@/hooks/useChartTheme';
import { lttbDownsampleByRange } from '@/lib/lttb';

interface EntropyLineChartProps {
  positions: Position[];
  onSelectPosition: (position: number) => void;
  averageEntropy: number;
  annotations?: Annotation[];
  selectedPosition?: number | null;
  /** Optional external ref to access the underlying ReactECharts instance (for chart export) */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  chartInstanceRef?: React.MutableRefObject<any>;
}

export const EntropyLineChart = memo(function EntropyLineChart({
  positions,
  onSelectPosition,
  averageEntropy,
  annotations = [],
  selectedPosition = null,
  chartInstanceRef,
}: EntropyLineChartProps) {
  const chartRef = useRef<ReactECharts>(null);

  // Sync internal ref to external ref for chart export
  if (chartInstanceRef) {
    chartInstanceRef.current = chartRef.current;
  }
  const decimalPrecision = useSettingsStore((s) => s.settings.decimalPrecision);
  const chartTheme = useChartTheme();

  // Build annotation markers for the chart
  // NOTE: All hooks must be called before any early return to satisfy Rules of Hooks.
  const annotationMarkers = useMemo(() => {
    if (positions.length === 0) return [];
    return annotations.map((ann) => {
      const pos = positions.find((p) => p.position === ann.positionNumber);
      if (!pos) return null;
      return {
        coord: [ann.positionNumber, pos.entropy],
        symbol: 'circle',
        symbolSize: 12,
        itemStyle: {
          color: ANNOTATION_COLORS[ann.color],
          borderColor: chartTheme.isDark ? '#1f2937' : '#fff',
          borderWidth: 2,
        },
        label: {
          show: false,
        },
        name: ann.label || `Position ${ann.positionNumber}`,
      };
    }).filter(Boolean);
  }, [annotations, positions, chartTheme.isDark]);

  const option: EChartsOption = useMemo(() => {
    const lineData: [number, number][] = positions.map((p) => [p.position, p.entropy]);
    // Use reduce instead of Math.max(...) to avoid engine argument limits on large arrays
    const maxEntropy = positions.reduce((max, p) => (p.entropy > max ? p.entropy : max), 0);

    // Downsampling strategy: for large datasets, use custom LTTB with x-range
    // bucketization that respects actual genomic positions. This preserves peaks
    // even on filtered/sparse data where positions are non-contiguous.
    // Threshold adapts to display density (1 point per ~2 pixels).
    const pixelThreshold = typeof window !== 'undefined'
      ? Math.max(800, Math.floor(window.innerWidth * (window.devicePixelRatio ?? 1) * 0.5))
      : 2000;
    const downsampledLineData = lineData.length > pixelThreshold
      ? lttbDownsampleByRange(lineData, pixelThreshold)
      : lineData;
    
    // Heatmap data: [xIndex, yIndex (always 0), entropy value]
    const heatmapData = positions.map((p, i) => [i, 0, p.entropy]);

    return {
      aria: { enabled: true, label: { description: `Shannon entropy for ${positions.length} positions` } },
      tooltip: {
        trigger: 'axis',
        appendToBody: true,
        backgroundColor: chartTheme.tooltipBg,
        borderColor: chartTheme.tooltipBorder,
        textStyle: { color: chartTheme.tooltipText },
        formatter: (params: unknown) => {
          const paramArray = params as { data: [number, number] | number[]; seriesType: string }[];
          const lineParam = paramArray.find(p => p.seriesType === 'line');
          if (!lineParam) {
            const heatParam = paramArray.find(p => p.seriesType === 'heatmap');
            if (heatParam) {
              const [idx, , entropy] = heatParam.data as number[];
              const pos = positions[idx];
              if (pos) {
                return `Position: ${pos.position}<br/>Entropy: ${(entropy as number).toFixed(decimalPrecision)} bits`;
              }
            }
            return '';
          }
          if (!lineParam.data) return '';
          const [pos, entropy] = lineParam.data as [number, number];
          if (!Number.isFinite(entropy)) return `Position: ${pos}`;
          return `Position: ${pos}<br/>Entropy: ${entropy.toFixed(decimalPrecision)} bits`;
        },
      },
      grid: [
        { left: 60, right: 40, top: 40, bottom: '22%' },
        { left: 60, right: 40, top: '82%', height: 30 },
      ],
      xAxis: [
        {
          type: 'value',
          gridIndex: 0,
          name: 'Position',
          nameLocation: 'middle',
          nameGap: 30,
          nameTextStyle: { color: chartTheme.textColor },
          axisLabel: { color: chartTheme.textMutedColor },
          axisLine: { lineStyle: { color: chartTheme.axisLineColor } },
          splitLine: { lineStyle: { color: chartTheme.gridLineColor } },
          min: positions[0]?.position || 1,
          max: positions[positions.length - 1]?.position || 100,
        },
        {
          type: 'category',
          gridIndex: 1,
          data: positions.map((p) => p.position),
          axisLabel: { show: false },
          axisTick: { show: false },
          axisLine: { show: false },
        },
      ],
      yAxis: [
        {
          type: 'value',
          gridIndex: 0,
          name: 'Entropy (bits)',
          nameLocation: 'middle',
          nameGap: 45,
          nameTextStyle: { color: chartTheme.textColor },
          axisLabel: { color: chartTheme.textMutedColor },
          axisLine: { lineStyle: { color: chartTheme.axisLineColor } },
          splitLine: { lineStyle: { color: chartTheme.gridLineColor } },
          min: 0,
          max: Math.ceil(maxEntropy * 10) / 10 + 0.1,
        },
        {
          type: 'category',
          gridIndex: 1,
          data: [''],
          axisLabel: { show: false },
          axisTick: { show: false },
          axisLine: { show: false },
        },
      ],
      visualMap: {
        min: 0,
        max: maxEntropy,
        calculable: false,
        orient: 'horizontal',
        left: 'center',
        bottom: 0,
        show: true,
        itemWidth: 12,
        itemHeight: 80,
        text: ['High', 'Low'],
        textStyle: { color: chartTheme.textMutedColor, fontSize: 10 },
        seriesIndex: 1,
        inRange: {
          color: ENTROPY_COLORS,
        },
      },
      dataZoom: [
        {
          type: 'inside',
          xAxisIndex: [0, 1],
          filterMode: 'none',
        },
        {
          type: 'slider',
          xAxisIndex: [0, 1],
          filterMode: 'none',
          height: 30,
          bottom: '8%',
          showDataShadow: false,
          showDetail: false,
          brushSelect: true,
          handleSize: '60%',
          handleStyle: {
            color: chartTheme.isDark ? '#374151' : '#fff',
            borderColor: chartTheme.primaryColor,
          },
          fillerColor: chartTheme.isDark
            ? 'rgba(96, 165, 250, 0.25)'
            : 'rgba(59, 130, 246, 0.2)',
          borderColor: 'transparent',
          backgroundColor: 'transparent',
          textStyle: { color: chartTheme.textMutedColor },
        },
      ],
      series: [
        {
          name: 'Entropy',
          type: 'line',
          xAxisIndex: 0,
          yAxisIndex: 0,
          data: downsampledLineData,
          smooth: false,
          symbol: 'circle',
          // Always show symbols to ensure every point is clickable across all
          // dataset sizes (501-999 range was previously unclickable). Smaller
          // symbols prevent visual clutter on dense charts.
          symbolSize: lineData.length > 2000 ? 1 : lineData.length > 1000 ? 2 : lineData.length > 500 ? 4 : 6,
          showSymbol: true,
          large: downsampledLineData.length > 2000,
          largeThreshold: 2000,
          triggerLineEvent: true,
          lineStyle: {
            width: 2,
            color: chartTheme.primaryColor,
          },
          itemStyle: {
            color: chartTheme.primaryColor,
          },
          emphasis: {
            focus: 'series',
            itemStyle: {
              borderColor: chartTheme.isDark ? '#1f2937' : '#fff',
              borderWidth: 2,
            },
          },
          markLine: {
            silent: true,
            symbol: 'none',
            lineStyle: {
              type: 'dashed',
              color: chartTheme.textMutedColor,
            },
            data: Number.isFinite(averageEntropy) ? [
              {
                yAxis: averageEntropy,
                label: {
                  formatter: `Avg: ${averageEntropy.toFixed(decimalPrecision)}`,
                  position: 'end',
                  color: chartTheme.textMutedColor,
                },
              },
            ] : [],
          },
          // Visual indicator for the currently selected position
          markPoint: (() => {
            if (selectedPosition == null) return { data: [] };
            const idx = positions.findIndex(p => p.position === selectedPosition);
            if (idx === -1) return { data: [] };
            return {
              symbol: 'pin',
              symbolSize: 40,
              itemStyle: {
                color: chartTheme.isDark ? '#f59e0b' : '#d97706',
                borderColor: chartTheme.isDark ? '#fbbf24' : '#f59e0b',
                borderWidth: 1,
              },
              label: {
                show: true,
                formatter: `${positions[idx].entropy.toFixed(2)}`,
                fontSize: 10,
                color: '#fff',
              },
              data: [{ name: 'Selected', coord: [positions[idx].position, positions[idx].entropy] }],
            };
          })(),
        },
        {
          name: 'Entropy Heatmap',
          type: 'heatmap',
          xAxisIndex: 1,
          yAxisIndex: 1,
          data: heatmapData,
          progressive: heatmapData.length > 500 ? 200 : 0,
          progressiveThreshold: 500,
          emphasis: {
            itemStyle: {
              borderColor: chartTheme.isDark ? '#1f2937' : '#fff',
              borderWidth: 1,
            },
          },
        },
        ...(annotationMarkers.length > 0
          ? [
              {
                name: 'Annotations',
                type: 'scatter' as const,
                xAxisIndex: 0,
                yAxisIndex: 0,
                data: annotationMarkers.map((m) => ({
                  value: m!.coord,
                  itemStyle: m!.itemStyle,
                  name: m!.name,
                })),
                symbolSize: 12,
                z: 10,
              },
            ]
          : []),
      ],
    };
  }, [positions, averageEntropy, decimalPrecision, annotationMarkers, chartTheme, selectedPosition]);

  const handleClick = useCallback(
    (params: { data?: [number, number] | number[]; seriesType?: string; seriesName?: string; dataIndex?: number; event?: { offsetX?: number } }) => {
      // Heatmap clicks: dataIndex maps directly to positions array (heatmap uses full data)
      if (params.seriesType === 'heatmap' && params.dataIndex !== undefined) {
        const pos = positions[params.dataIndex];
        if (pos) {
          onSelectPosition(pos.position);
          return;
        }
      }

      // Line clicks: use data[0] (the actual position number) rather than dataIndex,
      // because the line series may be downsampled — dataIndex would map to the
      // downsampled array, not the original positions array. (Fix 5.3)
      if (params.seriesType === 'line' && params.data && Array.isArray(params.data) && params.data.length >= 1) {
        const posNum = typeof params.data[0] === 'number' ? params.data[0] : parseInt(String(params.data[0]), 10);
        if (!isNaN(posNum)) {
          onSelectPosition(posNum);
          return;
        }
      }

      // Generic data array fallback
      if (params.data && Array.isArray(params.data) && params.data.length >= 1) {
        const posNum = typeof params.data[0] === 'number' ? params.data[0] : parseInt(String(params.data[0]), 10);
        if (!isNaN(posNum)) {
          onSelectPosition(posNum);
          return;
        }
      }

      // Ultimate fallback for line body clicks (e.g., 501-999 points where
      // triggerLineEvent fires but neither dataIndex nor data are populated).
      // Use ECharts' coordinate conversion to find the nearest position.
      if (params.event?.offsetX != null && chartRef.current) {
        const instance = chartRef.current.getEchartsInstance();
        const pixelCoord = [params.event.offsetX, 0];
        const dataCoord = instance.convertFromPixel({ xAxisIndex: 0 }, pixelCoord);
        if (dataCoord) {
          const clickedX = dataCoord[0];
          // Binary-search-like nearest position lookup
          let nearest = positions[0];
          let minDist = Math.abs(nearest.position - clickedX);
          for (let i = 1; i < positions.length; i++) {
            const dist = Math.abs(positions[i].position - clickedX);
            if (dist < minDist) {
              minDist = dist;
              nearest = positions[i];
            } else {
              break; // positions are sorted, so distance only increases past the nearest
            }
          }
          if (nearest) {
            onSelectPosition(nearest.position);
          }
        }
      }
    },
    [onSelectPosition, positions]
  );

  // Stable reference for onEvents to prevent ECharts handler re-registration (Fix 5.86)
  const chartEvents = useMemo(() => ({ click: handleClick }), [handleClick]);

  // Early return AFTER all hooks to satisfy Rules of Hooks
  if (positions.length === 0) {
    return (
      <div className="flex h-full w-full items-center justify-center text-muted-foreground text-sm">
        No position data available
      </div>
    );
  }

  const ariaLabel = `Entropy line chart showing ${positions.length} positions. Average entropy: ${averageEntropy.toFixed(2)}. Click a point to select a position for detailed view.`;

  return (
    <div className="h-full w-full min-h-0 min-w-0" role="img" aria-label={ariaLabel}>
      <ReactECharts
        ref={chartRef}
        option={option}
        style={{ height: '100%', width: '100%' }}
        onEvents={chartEvents}
        opts={{ renderer: 'canvas' }}
        notMerge={true}
        lazyUpdate={true}
      />
    </div>
  );
});
