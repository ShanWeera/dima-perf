/**
 * DiMA Desktop - Sequence Metadata Pie Chart
 * 
 * Shows distribution of metadata values (host species, country, etc.)
 * for the selected position's variants with a field selector dropdown.
 */

import { useMemo, useState, useEffect } from 'react';
import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { Variant } from '@/lib/types';

interface MetadataPieChartProps {
  variants: Variant[] | null;
  availableFields: string[];
}

// Color palette for pie slices
const PIE_COLORS = [
  '#8b5cf6', // purple
  '#ec4899', // pink
  '#c4a35a', // tan/gold
  '#22d3ee', // cyan
  '#a3e635', // lime
  '#f97316', // orange
  '#3b82f6', // blue
  '#ef4444', // red
  '#22c55e', // green
  '#f59e0b', // amber
];

export function MetadataPieChart({
  variants,
  availableFields,
}: MetadataPieChartProps) {
  const [selectedField, setSelectedField] = useState<string>(availableFields[0] || '');

  // Update selected field when available fields change
  useEffect(() => {
    if (availableFields.length > 0 && !availableFields.includes(selectedField)) {
      setSelectedField(availableFields[0]);
    }
  }, [availableFields, selectedField]);

  const option: EChartsOption = useMemo(() => {
    if (!variants || variants.length === 0 || !selectedField) {
      return {
        title: {
          text: 'No metadata available',
          left: 'center',
          top: 'center',
          textStyle: {
            color: '#9ca3af',
            fontSize: 14,
          },
        },
      };
    }

    // Aggregate metadata across variants
    const valueCounts: Record<string, number> = {};
    let totalCount = 0;
    
    variants.forEach((v) => {
      if (v.metadata && v.metadata[selectedField]) {
        Object.entries(v.metadata[selectedField]).forEach(([value, count]) => {
          valueCounts[value] = (valueCounts[value] || 0) + count;
          totalCount += count;
        });
      }
    });

    if (Object.keys(valueCounts).length === 0) {
      return {
        title: {
          text: `No ${selectedField} data`,
          left: 'center',
          top: 'center',
          textStyle: {
            color: '#9ca3af',
            fontSize: 14,
          },
        },
      };
    }

    // Sort by count and take top entries
    const sorted = Object.entries(valueCounts)
      .sort((a, b) => b[1] - a[1]);
    
    const topN = sorted.slice(0, 10);
    const otherCount = sorted.slice(10).reduce((sum, [, count]) => sum + count, 0);

    const data = topN.map(([name, value], i) => ({
      value,
      name: name || '(empty)',
      itemStyle: { color: PIE_COLORS[i % PIE_COLORS.length] },
    }));

    if (otherCount > 0) {
      data.push({
        value: otherCount,
        name: 'Others',
        itemStyle: { color: '#9ca3af' },
      });
    }

    return {
      tooltip: {
        trigger: 'item',
        appendToBody: true,
        formatter: (params: unknown) => {
          const p = params as { name: string; value: number; percent: number };
          return `${p.name}: ${p.percent.toFixed(1)}%`;
        },
      },
      legend: {
        type: 'scroll',
        orient: 'vertical',
        right: 10,
        top: 'middle',
        icon: 'rect',
        itemWidth: 14,
        itemHeight: 14,
        textStyle: {
          fontSize: 11,
        },
      },
      series: [
        {
          type: 'pie',
          radius: ['0%', '70%'],
          center: ['40%', '50%'],
          avoidLabelOverlap: true,
          label: {
            show: true,
            position: 'outside',
            formatter: (params: unknown) => {
              const p = params as { name: string; percent: number };
              return `${p.name}: ${p.percent.toFixed(1)}%`;
            },
            fontSize: 11,
          },
          labelLine: {
            show: true,
            length: 10,
            length2: 10,
          },
          emphasis: {
            itemStyle: {
              shadowBlur: 10,
              shadowOffsetX: 0,
              shadowColor: 'rgba(0, 0, 0, 0.5)',
            },
          },
          data: data,
        },
      ],
    };
  }, [variants, selectedField]);

  // Format field name for display (convert snake_case to Title Case)
  const formatFieldName = (field: string) => {
    return field
      .split('_')
      .map(word => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
  };

  return (
    <div className="flex h-full w-full flex-col min-h-0 min-w-0 p-4">
      {/* Field Selector */}
      {availableFields.length > 0 && (
        <div className="mb-3">
          <label className="mb-1 block text-xs text-primary font-medium">Metadata</label>
          <select
            value={selectedField}
            onChange={(e) => setSelectedField(e.target.value)}
            className="w-full rounded-md border bg-background px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          >
            {availableFields.map((field) => (
              <option key={field} value={field}>
                {formatFieldName(field)}
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Pie Chart */}
      <div className="flex-1 min-h-0">
        <ReactECharts
          option={option}
          style={{ height: '100%', width: '100%' }}
          opts={{ renderer: 'canvas' }}
          notMerge={true}
        />
      </div>
    </div>
  );
}
