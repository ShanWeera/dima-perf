/**
 * DiMA Desktop - Motif Distribution Bar Chart
 * 
 * Shows distribution of motif types (Index, Major, Minor, Unique)
 * and variant incidence percentages for the selected position.
 */

import { useMemo } from 'react';
import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { Variant } from '@/lib/types';

interface VariantPieChartProps {
  variants: Variant[] | null;
  totalVariantsIncidence?: number;
  distinctVariantsIncidence?: number;
}

// Colors for each bar category
const BAR_COLORS = {
  Index: '#3b82f6',      // blue
  Major: '#22c55e',      // green
  Minor: '#f59e0b',      // amber
  Unique: '#f97316',     // orange
  'Total Variants': '#8b5cf6',    // purple
  'Distinct Variants': '#a855f7', // violet
};

export function VariantPieChart({ 
  variants, 
  totalVariantsIncidence = 0,
  distinctVariantsIncidence = 0,
}: VariantPieChartProps) {
  const option: EChartsOption = useMemo(() => {
    if (!variants || variants.length === 0) {
      return {
        title: {
          text: 'No variants available',
          left: 'center',
          top: 'center',
          textStyle: {
            color: '#9ca3af',
            fontSize: 14,
          },
        },
      };
    }

    // Calculate total incidence for each motif type
    const motifIncidences: Record<string, number> = {
      Index: 0,
      Major: 0,
      Minor: 0,
      Unique: 0,
    };

    variants.forEach((v) => {
      const motif = v.motif_short;
      if (motif === 'I') {
        motifIncidences.Index += v.incidence;
      } else if (motif === 'Ma') {
        motifIncidences.Major += v.incidence;
      } else if (motif === 'Mi') {
        motifIncidences.Minor += v.incidence;
      } else if (motif === 'U') {
        motifIncidences.Unique += v.incidence;
      }
    });

    const categories = ['Index', 'Major', 'Minor', 'Unique', 'Total Variants', 'Distinct Variants'];
    const rawData = [
      motifIncidences.Index,
      motifIncidences.Major,
      motifIncidences.Minor,
      motifIncidences.Unique,
      totalVariantsIncidence,
      distinctVariantsIncidence,
    ];
    // Round to 1 decimal place for display
    const data = rawData.map(v => Math.round(v * 10) / 10);

    return {
      tooltip: {
        trigger: 'axis',
        appendToBody: true,
        axisPointer: {
          type: 'shadow',
        },
        formatter: (params: unknown) => {
          const param = (params as { name: string; value: number }[])[0];
          if (!param) return '';
          return `${param.name}: ${param.value.toFixed(1)}%`;
        },
      },
      grid: {
        left: 60,
        right: 20,
        top: 20,
        bottom: 60,
      },
      xAxis: {
        type: 'category',
        data: categories,
        axisLabel: {
          interval: 0,
          rotate: 0,
          fontSize: 11,
        },
      },
      yAxis: {
        type: 'value',
        name: 'Incidence (%)',
        nameLocation: 'middle',
        nameGap: 40,
        min: 0,
        max: 100,
        axisLabel: {
          formatter: '{value}',
        },
      },
      series: [
        {
          type: 'bar',
          data: data.map((value, i) => ({
            value,
            itemStyle: {
              color: BAR_COLORS[categories[i] as keyof typeof BAR_COLORS],
            },
          })),
          barWidth: '50%',
          label: {
            show: true,
            position: 'top',
            formatter: '{c}%',
            fontSize: 11,
          },
        },
      ],
    };
  }, [variants, totalVariantsIncidence, distinctVariantsIncidence]);

  return (
    <div className="h-full w-full min-h-0 min-w-0">
      <ReactECharts
        option={option}
        style={{ height: '100%', width: '100%' }}
        opts={{ renderer: 'canvas' }}
        notMerge={true}
      />
    </div>
  );
}
