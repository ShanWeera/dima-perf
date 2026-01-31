/**
 * DiMA Desktop - Entropy Line Chart
 * 
 * Interactive line chart showing entropy values across positions.
 * Features: zoom/pan, click to select position, average line, markers.
 * Includes integrated heatmap as zoom/overview control.
 */

import { useMemo, useRef, useCallback } from 'react';
import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { Position, Annotation } from '@/lib/types';
import { ANNOTATION_COLORS } from '@/lib/types';
import { useSettingsStore } from '@/stores/settingsStore';
import { ENTROPY_COLORS } from '@/lib/colors';

interface EntropyLineChartProps {
  positions: Position[];
  selectedPosition: number | null;
  onSelectPosition: (position: number) => void;
  averageEntropy: number;
  highestEntropyPosition: number;
  annotations?: Annotation[];
}

export function EntropyLineChart({
  positions,
  selectedPosition: _selectedPosition,
  onSelectPosition,
  averageEntropy,
  highestEntropyPosition: _highestEntropyPosition,
  annotations = [],
}: EntropyLineChartProps) {
  const chartRef = useRef<ReactECharts>(null);
  const { settings } = useSettingsStore();

  // Build annotation markers for the chart
  const annotationMarkers = useMemo(() => {
    return annotations.map((ann) => {
      const pos = positions.find((p) => p.position === ann.positionNumber);
      if (!pos) return null;
      return {
        coord: [ann.positionNumber, pos.entropy],
        symbol: 'circle',
        symbolSize: 12,
        itemStyle: {
          color: ANNOTATION_COLORS[ann.color],
          borderColor: '#fff',
          borderWidth: 2,
        },
        label: {
          show: false,
        },
        name: ann.label || `Position ${ann.positionNumber}`,
      };
    }).filter(Boolean);
  }, [annotations, positions]);

  const option: EChartsOption = useMemo(() => {
    const lineData = positions.map((p) => [p.position, p.entropy]);
    const maxEntropy = Math.max(...positions.map((p) => p.entropy));
    
    // Heatmap data: [xIndex, yIndex (always 0), entropy value]
    const heatmapData = positions.map((p, i) => [i, 0, p.entropy]);

    return {
      tooltip: {
        trigger: 'axis',
        appendToBody: true,
        formatter: (params: unknown) => {
          const paramArray = params as { data: [number, number] | number[]; seriesType: string }[];
          const lineParam = paramArray.find(p => p.seriesType === 'line');
          if (!lineParam) {
            // Handle heatmap tooltip
            const heatParam = paramArray.find(p => p.seriesType === 'heatmap');
            if (heatParam) {
              const [idx, , entropy] = heatParam.data as number[];
              const pos = positions[idx];
              if (pos) {
                return `Position: ${pos.position}<br/>Entropy: ${(entropy as number).toFixed(settings.decimalPrecision)}`;
              }
            }
            return '';
          }
          const [pos, entropy] = lineParam.data as [number, number];
          return `Position: ${pos}<br/>Entropy: ${entropy.toFixed(settings.decimalPrecision)}`;
        },
      },
      // Two grids: line chart (top) and heatmap (bottom)
      grid: [
        { left: 60, right: 40, top: 40, bottom: '22%' },  // Line chart grid
        { left: 60, right: 40, top: '82%', height: 30 },  // Heatmap grid
      ],
      // Two x-axes linked together
      xAxis: [
        {
          type: 'value',
          gridIndex: 0,
          name: 'Position',
          nameLocation: 'middle',
          nameGap: 30,
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
      // Two y-axes
      yAxis: [
        {
          type: 'value',
          gridIndex: 0,
          name: 'Entropy',
          nameLocation: 'middle',
          nameGap: 45,
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
      // Visual map for heatmap colors (hidden, just for coloring)
      visualMap: {
        min: 0,
        max: maxEntropy,
        calculable: false,
        orient: 'horizontal',
        left: 'center',
        bottom: 0,
        show: false,
        seriesIndex: 1, // Apply to heatmap series
        inRange: {
          color: ENTROPY_COLORS,
        },
      },
      // DataZoom linked to both x-axes - heatmap acts as the brush selector
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
            color: '#fff',
            borderColor: '#3b82f6',
          },
          fillerColor: 'rgba(59, 130, 246, 0.2)',
          borderColor: 'transparent',
          backgroundColor: 'transparent',
        },
      ],
      series: [
        // Line chart series
        {
          name: 'Entropy',
          type: 'line',
          xAxisIndex: 0,
          yAxisIndex: 0,
          data: lineData,
          smooth: false,
          symbol: 'circle',
          symbolSize: lineData.length > 500 ? 4 : 6,
          showSymbol: lineData.length <= 500,
          large: lineData.length > 1000,
          largeThreshold: 1000,
          sampling: lineData.length > 500 ? 'lttb' : undefined,
          triggerLineEvent: true, // Enable click events on the line itself
          lineStyle: {
            width: 2,
            color: '#3b82f6',
          },
          itemStyle: {
            color: '#3b82f6',
          },
          emphasis: {
            focus: 'series',
            itemStyle: {
              borderColor: '#fff',
              borderWidth: 2,
            },
          },
          markLine: {
            silent: true,
            symbol: 'none',
            lineStyle: {
              type: 'dashed',
              color: '#9ca3af',
            },
            data: [
              {
                yAxis: averageEntropy,
                label: {
                  formatter: `Avg: ${averageEntropy.toFixed(2)}`,
                  position: 'end',
                },
              },
            ],
          },
        },
        // Heatmap series (overview/zoom control)
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
              borderColor: '#fff',
              borderWidth: 1,
            },
          },
        },
        // Annotation markers as a scatter series on line chart
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
  }, [positions, averageEntropy, settings.decimalPrecision, annotationMarkers]);

  const handleClick = useCallback(
    (params: { data?: [number, number] | number[]; seriesType?: string; seriesName?: string; dataIndex?: number }) => {
      if (params.seriesType === 'heatmap' && params.dataIndex !== undefined) {
        // Heatmap click - get position from index
        const pos = positions[params.dataIndex];
        if (pos) {
          onSelectPosition(pos.position);
        }
      } else if (params.seriesType === 'line' && params.dataIndex !== undefined) {
        // Line chart click via line or point - use dataIndex
        const pos = positions[params.dataIndex];
        if (pos) {
          onSelectPosition(pos.position);
        }
      } else if (params.data && Array.isArray(params.data) && params.data.length >= 1) {
        // Fallback: Line chart click with data array
        const posNum = typeof params.data[0] === 'number' ? params.data[0] : parseInt(String(params.data[0]), 10);
        if (!isNaN(posNum)) {
          onSelectPosition(posNum);
        }
      }
    },
    [onSelectPosition, positions]
  );

  return (
    <div className="h-full w-full min-h-0 min-w-0">
      <ReactECharts
        ref={chartRef}
        option={option}
        style={{ height: '100%', width: '100%' }}
        onEvents={{
          click: handleClick,
        }}
        opts={{ renderer: 'canvas' }}
        notMerge={true}
      />
    </div>
  );
}
