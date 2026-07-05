/**
 * DiMA Desktop - Chart Theme Hook
 * 
 * Provides theme-aware colors for ECharts and other chart components.
 * Uses static light/dark theme objects selected by the user's theme preference.
 */

import { useMemo } from 'react';
import { useSettingsStore } from '@/stores/settingsStore';

export interface ChartTheme {
  textColor: string;
  textMutedColor: string;
  backgroundColor: string;
  gridLineColor: string;
  axisLineColor: string;
  primaryColor: string;
  tooltipBg: string;
  tooltipBorder: string;
  tooltipText: string;
  isDark: boolean;
}

const LIGHT_THEME: ChartTheme = {
  textColor: '#1f2937',
  textMutedColor: '#6b7280',
  backgroundColor: 'transparent',
  gridLineColor: '#e5e7eb',
  axisLineColor: '#d1d5db',
  primaryColor: '#3b82f6',
  tooltipBg: '#ffffff',
  tooltipBorder: '#e5e7eb',
  tooltipText: '#1f2937',
  isDark: false,
};

const DARK_THEME: ChartTheme = {
  textColor: '#e5e7eb',
  textMutedColor: '#9ca3af',
  backgroundColor: 'transparent',
  gridLineColor: '#374151',
  axisLineColor: '#4b5563',
  primaryColor: '#60a5fa',
  tooltipBg: '#1f2937',
  tooltipBorder: '#374151',
  tooltipText: '#e5e7eb',
  isDark: true,
};

/**
 * Returns a stable ChartTheme object that updates when the effective theme changes.
 */
export function useChartTheme(): ChartTheme {
  const effectiveTheme = useSettingsStore((s) => s.effectiveTheme);

  return useMemo(
    () => (effectiveTheme === 'dark' ? DARK_THEME : LIGHT_THEME),
    [effectiveTheme]
  );
}
