/**
 * DiMA Desktop - Motif Distribution Bar Chart
 * 
 * Shows distribution of motif types (Index, Major, Minor, Unique)
 * and variant incidence percentages for the selected position.
 */

import { useMemo, memo } from 'react';
import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { Variant } from '@/lib/types';
import { MOTIF_COLORS } from '@/lib/colors';
import { useChartTheme } from '@/hooks/useChartTheme';

interface VariantPieChartProps {
  variants: Variant[] | null;
  totalVariantsIncidence?: number;
  distinctVariantsIncidence?: number;
}

// Colors for bar chart categories — uses centralized MOTIF_COLORS for consistency
const BAR_COLORS: Record<string, string> = {
  Index: MOTIF_COLORS['I'],
  Major: MOTIF_COLORS['Ma'],
  Minor: MOTIF_COLORS['Mi'],
  Unique: MOTIF_COLORS['U'],
  'Total Variants': '#8b5cf6',
  'Variant Richness': '#a855f7',
};

export const VariantBarChart = memo(function VariantBarChart({ 
  variants, 
  totalVariantsIncidence = 0,
  distinctVariantsIncidence = 0,
}: VariantPieChartProps) {
  const chartTheme = useChartTheme();
  const option: EChartsOption = useMemo(() => {
    if (!variants || variants.length === 0) {
      return {
        title: {
          text: 'No variants available',
          left: 'center',
          top: 'center',
          textStyle: {
            color: chartTheme.textMutedColor,
            fontSize: 14,
          },
        },
      };
    }

    const ariaOption = { enabled: true, label: { description: `Variant distribution for ${variants.length} variants` } };

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

    // Bars 1-4 are read fractions (sum to ~100%). Bars 5-6 are summary diversity
    // metrics with different denominators — must be visually distinguished to
    // prevent misleading scientific comparisons (per PMC11596295).
    const categories = ['Index', 'Major', 'Minor', 'Unique', 'Total Variants', 'Variant Richness'];
    const rawData = [
      motifIncidences.Index,
      motifIncidences.Major,
      motifIncidences.Minor,
      motifIncidences.Unique,
      totalVariantsIncidence,
      distinctVariantsIncidence,
    ];
    const data = rawData.map(v => Math.round(v * 10) / 10);

    // Tooltip descriptions for each metric to clarify what's being shown
    const metricDescriptions: Record<string, string> = {
      'Index': 'Most prevalent k-mer (read fraction)',
      'Major': 'Second most prevalent (read fraction)',
      'Minor': 'Low-frequency variants (read fraction)',
      'Unique': 'Seen exactly once (read fraction)',
      'Total Variants': 'Non-index reads / support (subset fraction)',
      'Variant Richness': 'Distinct non-index types / non-index reads (type diversity)',
    };

    return {
      aria: ariaOption,
      tooltip: {
        trigger: 'axis',
        appendToBody: true,
        backgroundColor: chartTheme.tooltipBg,
        borderColor: chartTheme.tooltipBorder,
        textStyle: { color: chartTheme.tooltipText },
        axisPointer: {
          type: 'shadow',
        },
        formatter: (params: unknown) => {
          const param = (params as { name: string; value: number }[])[0];
          if (!param) return '';
          const desc = metricDescriptions[param.name] ?? '';
          return `<strong>${param.name}: ${param.value.toFixed(1)}%</strong><br/><span style="font-size:11px;opacity:0.8">${desc}</span>`;
        },
      },
      grid: {
        left: 60,
        right: 20,
        top: 30,
        bottom: 60,
      },
      xAxis: {
        type: 'category',
        data: categories,
        axisLabel: {
          interval: 0,
          rotate: 0,
          fontSize: 11,
          color: chartTheme.textMutedColor,
        },
        axisLine: { lineStyle: { color: chartTheme.axisLineColor } },
      },
      yAxis: {
        type: 'value',
        name: 'Percentage (%)',
        nameLocation: 'middle',
        nameGap: 40,
        nameTextStyle: { color: chartTheme.textColor },
        min: 0,
        max: 100,
        axisLabel: {
          formatter: '{value}',
          color: chartTheme.textMutedColor,
        },
        axisLine: { lineStyle: { color: chartTheme.axisLineColor } },
        splitLine: { lineStyle: { color: chartTheme.gridLineColor } },
      },
      series: [
        {
          type: 'bar',
          data: data.map((value, i) => ({
            value,
            itemStyle: {
              color: BAR_COLORS[categories[i] as keyof typeof BAR_COLORS],
              // Visually distinguish summary metrics from motif fractions
              // by using a dashed border pattern on the last 2 bars
              borderColor: i >= 4 ? chartTheme.textMutedColor : 'transparent',
              borderWidth: i >= 4 ? 1 : 0,
              borderType: i >= 4 ? 'dashed' as const : 'solid' as const,
            },
          })),
          barWidth: '50%',
          label: {
            show: true,
            position: 'top',
            formatter: '{c}%',
            fontSize: 11,
            color: chartTheme.textColor,
          },
          // Vertical separator between motif fractions and summary metrics
          markLine: {
            silent: true,
            symbol: 'none',
            lineStyle: {
              type: 'dashed',
              color: chartTheme.textMutedColor,
              opacity: 0.5,
            },
            data: [
              { xAxis: 3.5 },
            ],
            label: { show: false },
          },
          markArea: {
            silent: true,
            data: [
              [
                { 
                  xAxis: 'Total Variants', 
                  itemStyle: { 
                    color: chartTheme.isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)',
                  },
                },
                { xAxis: 'Variant Richness' },
              ],
            ],
            label: {
              show: true,
              position: 'insideTop',
              formatter: 'Summary Metrics',
              fontSize: 9,
              color: chartTheme.textMutedColor,
              offset: [0, 5],
            },
          },
        },
      ],
    };
  }, [variants, totalVariantsIncidence, distinctVariantsIncidence, chartTheme]);

  const ariaDescription = variants && variants.length > 0
    ? `Motif distribution: Index ${Math.round(totalVariantsIncidence > 0 ? 100 - totalVariantsIncidence : 100)}%, Total Variants ${Math.round(totalVariantsIncidence)}%, Variant Richness ${Math.round(distinctVariantsIncidence)}%.`
    : 'No variant data available.';

  return (
    <div className="h-full w-full min-h-0 min-w-0" role="img" aria-label={`Variant distribution bar chart. ${ariaDescription}`}>
      <ReactECharts
        option={option}
        style={{ height: '100%', width: '100%' }}
        opts={{ renderer: 'canvas' }}
        notMerge={false}
        lazyUpdate={true}
      />
    </div>
  );
});
