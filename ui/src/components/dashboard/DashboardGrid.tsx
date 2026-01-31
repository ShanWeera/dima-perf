/**
 * DiMA Desktop - Dashboard Grid
 * 
 * Customizable dashboard using react-grid-layout for panel arrangement.
 */

import { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import GridLayout, { Layout } from 'react-grid-layout';
import 'react-grid-layout/css/styles.css';
import type { AnalysisResult, Position, Variant, Annotation } from '@/lib/types';
import { EntropyLineChart } from '@/components/charts/EntropyLineChart';
import { VariantPieChart } from '@/components/charts/VariantPieChart';
import { MetadataPieChart } from '@/components/charts/MetadataPieChart';
import { PositionExplorer } from '@/components/charts/PositionExplorer';
import { HCSMap } from '@/components/charts/HCSMap';
import { DashboardPanel } from './DashboardPanel';
import { VariantModal } from './VariantModal';

// Enhanced hook to track container width with debouncing and multiple fallbacks
function useContainerWidth() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [resizeKey, setResizeKey] = useState(0);
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastWidth = useRef(0);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const updateWidth = () => {
      const newWidth = container.clientWidth;
      if (newWidth > 0 && newWidth !== lastWidth.current) {
        lastWidth.current = newWidth;
        setWidth(newWidth);
      }
    };

    const debouncedUpdate = () => {
      // Clear any pending timer
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
      // Immediate update for responsiveness
      updateWidth();
      // Debounced "settled" update to force remount
      debounceTimer.current = setTimeout(() => {
        updateWidth();
        setResizeKey(k => k + 1); // Force grid remount after resize settles
      }, 150);
    };

    // ResizeObserver for container
    const resizeObserver = new ResizeObserver(debouncedUpdate);
    resizeObserver.observe(container);

    // Window resize as fallback
    window.addEventListener('resize', debouncedUpdate);

    // Initial measurement
    updateWidth();
    setResizeKey(k => k + 1);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener('resize', debouncedUpdate);
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
    };
  }, []);

  return { containerRef, width, resizeKey };
}

interface DashboardGridProps {
  results: AnalysisResult;
  selectedPosition: number | null;
  onSelectPosition: (position: number | null) => void;
  layout: Layout[];
  onLayoutChange: (layout: Layout[]) => void;
  hiddenPanels: string[];
  alphabet?: 'protein' | 'nucleotide';
  annotations?: Annotation[];
}

const DEFAULT_LAYOUT: Layout[] = [
  { i: 'entropy-line', x: 0, y: 0, w: 12, h: 7, minW: 4, minH: 5 },
  { i: 'variant-distribution', x: 0, y: 7, w: 4, h: 5, minW: 3, minH: 4 },
  { i: 'position-explorer', x: 4, y: 7, w: 4, h: 5, minW: 3, minH: 4 },
  { i: 'metadata-chart', x: 8, y: 7, w: 4, h: 5, minW: 3, minH: 4 },
  { i: 'hcs-map', x: 0, y: 12, w: 12, h: 3, minW: 4, minH: 2 },
];

export function DashboardGrid({
  results,
  selectedPosition,
  onSelectPosition,
  layout,
  onLayoutChange,
  hiddenPanels,
  alphabet = 'protein',
  annotations = [],
}: DashboardGridProps) {
  const [selectedVariant, setSelectedVariant] = useState<Variant | null>(null);
  const [hcsThreshold, setHcsThreshold] = useState(0);
  const { containerRef, width, resizeKey } = useContainerWidth();

  // Get the selected position data
  const selectedPositionData: Position | null = useMemo(() => {
    if (selectedPosition === null) return null;
    return results.results.find((p) => p.position === selectedPosition) || null;
  }, [results.results, selectedPosition]);

  // Get metadata fields from first variant
  const metadataFields = useMemo(() => {
    for (const pos of results.results) {
      if (pos.diversity_motifs) {
        for (const variant of pos.diversity_motifs) {
          if (variant.metadata) {
            return Object.keys(variant.metadata);
          }
        }
      }
    }
    return [];
  }, [results.results]);

  // Filter layout to only include visible panels
  const visibleLayout = useMemo(() => {
    return layout.filter((item) => !hiddenPanels.includes(item.i));
  }, [layout, hiddenPanels]);

  const handleVariantClick = useCallback((variant: Variant) => {
    setSelectedVariant(variant);
  }, []);

  // Calculate usable width (container width minus padding)
  const gridWidth = width > 0 ? width : 1200;

  return (
    <div ref={containerRef} className="h-full w-full">
      <GridLayout
        key={`grid-${resizeKey}`}
        className="layout"
        layout={visibleLayout}
        cols={12}
        rowHeight={60}
        width={gridWidth}
        onLayoutChange={onLayoutChange}
        draggableHandle=".panel-handle"
        margin={[16, 16]}
        containerPadding={[16, 16]}
      >
        {!hiddenPanels.includes('entropy-line') && (
          <div key="entropy-line" className="h-full">
            <DashboardPanel title="Entropy Chart" subtitle="Shannon entropy with heatmap overview" panelId="entropy-line">
              <EntropyLineChart
                positions={results.results}
                selectedPosition={selectedPosition}
                onSelectPosition={onSelectPosition}
                averageEntropy={results.average_entropy}
                highestEntropyPosition={results.highest_entropy.position}
                annotations={annotations}
              />
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('position-explorer') && (
          <div key="position-explorer" className="h-full">
            <DashboardPanel title="Position Explorer" subtitle={selectedPosition ? `Details for k-mer position ${selectedPosition}` : 'Select a position to view details'} panelId="position-explorer">
              <PositionExplorer
                position={selectedPositionData}
                alphabet={alphabet}
                onVariantClick={handleVariantClick}
                annotations={annotations}
              />
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('variant-distribution') && (
          <div key="variant-distribution" className="h-full">
            <DashboardPanel title="Motif Distribution" subtitle={`Distribution of motifs at k-mer position ${selectedPosition || 1}`} panelId="variant-distribution">
              <VariantPieChart
                variants={selectedPositionData?.diversity_motifs || null}
                totalVariantsIncidence={selectedPositionData?.total_variants_incidence || 0}
                distinctVariantsIncidence={selectedPositionData?.distinct_variants_incidence || 0}
              />
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('metadata-chart') && metadataFields.length > 0 && (
          <div key="metadata-chart" className="h-full">
            <DashboardPanel title="Sequence Metadata" subtitle="Metadata of the selected distinct sequence" panelId="metadata-chart">
              <MetadataPieChart
                variants={selectedPositionData?.diversity_motifs || null}
                availableFields={metadataFields}
              />
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('hcs-map') && (
          <div key="hcs-map" className="h-full">
            <DashboardPanel title="Highly Conserved Sequences" subtitle="Regions with high index motif conservation" panelId="hcs-map">
              <HCSMap
                positions={results.results}
                threshold={hcsThreshold}
                onThresholdChange={setHcsThreshold}
              />
            </DashboardPanel>
          </div>
        )}
      </GridLayout>

      {/* Variant Detail Modal */}
      {selectedVariant && (
        <VariantModal
          variant={selectedVariant}
          alphabet="protein"
          onClose={() => setSelectedVariant(null)}
        />
      )}
    </div>
  );
}

export { DEFAULT_LAYOUT };
