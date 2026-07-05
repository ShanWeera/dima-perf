/**
 * DiMA Desktop - Dashboard Layout Configuration
 *
 * Default panel layout and panel metadata for the dashboard grid.
 * Extracted from DashboardGrid to allow imports from SettingsView
 * without creating a circular component dependency.
 */

import type { Layout } from 'react-grid-layout';

// Default layout optimized for common viewport sizes (~1080-1440px height).
// Total grid height is ~18 rows × 60px = 1080px — fits a 1080p display without
// excessive scrolling while keeping charts readable.
export const DEFAULT_LAYOUT: Layout[] = [
  { i: 'entropy-line', x: 0, y: 0, w: 12, h: 6, minW: 4, minH: 4 },
  { i: 'variant-distribution', x: 0, y: 6, w: 4, h: 5, minW: 3, minH: 4 },
  { i: 'position-explorer', x: 4, y: 6, w: 4, h: 5, minW: 3, minH: 4 },
  { i: 'metadata-chart', x: 8, y: 6, w: 4, h: 5, minW: 3, minH: 4 },
  { i: 'hcs-map', x: 0, y: 11, w: 6, h: 4, minW: 4, minH: 3 },
  { i: 'pdb-viewer', x: 6, y: 11, w: 6, h: 6, minW: 4, minH: 5 },
  { i: 'feature-tracks', x: 0, y: 17, w: 12, h: 4, minW: 6, minH: 3 },
];

/** Panel metadata for toggle controls and display labels */
export const PANEL_INFO: { id: string; label: string }[] = [
  { id: 'entropy-line', label: 'Entropy Chart' },
  { id: 'variant-distribution', label: 'Variant Distribution' },
  { id: 'position-explorer', label: 'Position Explorer' },
  { id: 'metadata-chart', label: 'Metadata Chart' },
  { id: 'hcs-map', label: 'HCS Map' },
  { id: 'pdb-viewer', label: 'PDB 3D Viewer' },
  { id: 'feature-tracks', label: 'Feature Tracks' },
];
