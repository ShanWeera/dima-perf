/**
 * DiMA Desktop - Entropy Heatmap
 * 
 * Single-row heatmap showing entropy gradient across positions.
 */

import { useMemo, useCallback } from 'react';
import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { Position, Annotation } from '@/lib/types';
import { ANNOTATION_COLORS } from '@/lib/types';
import { ENTROPY_COLORS } from '@/lib/colors';

interface EntropyHeatmapProps {
  positions: Position[];
  selectedPosition: number | null;
  onSelectPosition: (position: number) => void;
  annotations?: Annotation[];
}

export function EntropyHeatmap({
  positions,
  selectedPosition: _selectedPosition,
  onSelectPosition,
  annotations = [],
}: EntropyHeatmapProps) {
  // Build annotation data for display
  const annotationData = useMemo(() => {
    return annotations.map((ann) => {
      const posIdx = positions.findIndex((p) => p.position === ann.positionNumber);
      if (posIdx === -1) return null;
      return {
        posIdx,
        color: ANNOTATION_COLORS[ann.color],
        label: ann.label,
      };
    }).filter(Boolean);
  }, [annotations, positions]);
  const option: EChartsOption = useMemo(() => {
    const maxEntropy = Math.max(...positions.map((p) => p.entropy));
    const data = positions.map((p, i) => [i, 0, p.entropy]);

    return {
      tooltip: {
        appendToBody: true,
        formatter: (params: unknown) => {
          const param = params as { data: [number, number, number] };
          const position = positions[param.data[0]];
          return `Position: ${position.position}<br/>Entropy: ${param.data[2].toFixed(4)}`;
        },
      },
      grid: {
        left: 60,
        right: 20,
        top: 10,
        bottom: 40,
      },
      xAxis: {
        type: 'category',
        data: positions.map((p) => p.position),
        name: 'Position',
        nameLocation: 'middle',
        nameGap: 25,
        axisLabel: {
          interval: Math.floor(positions.length / 10),
        },
      },
      yAxis: {
        type: 'category',
        data: ['Entropy'],
        axisLabel: {
          show: false,
        },
        axisTick: {
          show: false,
        },
      },
      visualMap: {
        min: 0,
        max: maxEntropy,
        calculable: true,
        orient: 'horizontal',
        left: 'center',
        bottom: 5,
        itemWidth: 15,
        itemHeight: 100,
        inRange: {
          color: ENTROPY_COLORS,
        },
        show: false,
      },
      series: [
        {
          type: 'heatmap',
          data: data,
          // Performance optimizations for large datasets
          progressive: data.length > 500 ? 200 : 0,
          progressiveThreshold: 500,
          emphasis: {
            itemStyle: {
              borderColor: '#fff',
              borderWidth: 2,
            },
          },
        },
        // Annotation markers - show colored dots at annotated positions
        ...(annotationData.length > 0
          ? [
              {
                type: 'scatter' as const,
                data: annotationData.map((ann) => ({
                  value: [ann!.posIdx, 0],
                  itemStyle: {
                    color: ann!.color,
                    borderColor: '#fff',
                    borderWidth: 1,
                  },
                })),
                symbolSize: 10,
                symbolOffset: [0, -15],
                z: 10,
              },
            ]
          : []),
      ],
    };
  }, [positions, annotationData]);

  const handleClick = useCallback(
    (params: { dataIndex?: number }) => {
      if (params.dataIndex !== undefined) {
        onSelectPosition(positions[params.dataIndex].position);
      }
    },
    [onSelectPosition, positions]
  );

  return (
    <div className="h-full w-full min-h-0 min-w-0">
      <ReactECharts
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
