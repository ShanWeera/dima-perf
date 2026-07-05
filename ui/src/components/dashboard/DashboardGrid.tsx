/**
 * DiMA Desktop - Dashboard Grid
 * 
 * Customizable dashboard using react-grid-layout for panel arrangement.
 */

import { useState, useMemo, useCallback, useEffect, useRef, lazy, Suspense } from 'react';
import GridLayout, { Layout } from 'react-grid-layout';
import 'react-grid-layout/css/styles.css';
import type { AnalysisResult, Position, Variant, Annotation } from '@/lib/types';
import { computeHCSRegions } from '@/lib/hcs';
import { EntropyLineChart } from '@/components/charts/EntropyLineChart';
import { VariantBarChart } from '@/components/charts/VariantBarChart';
import { MetadataPieChart } from '@/components/charts/MetadataPieChart';
import { PositionExplorer } from '@/components/charts/PositionExplorer';
import { HCSMap } from '@/components/charts/HCSMap';
import { FeatureTrackViewer } from '@/components/features/FeatureTrackViewer';
import { DashboardPanel } from './DashboardPanel';
import { VariantModal } from './VariantModal';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { Loader2 } from 'lucide-react';

// Lazy-load the PDB viewer since it imports 3Dmol.js (~2MB)
const PDBViewer = lazy(() => import('@/components/charts/PDBViewer').then(m => ({ default: m.PDBViewer })));

function PanelLoadingFallback() {
  return (
    <div className="flex h-full items-center justify-center">
      <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
    </div>
  );
}

// Hook to track container width via ResizeObserver.
// Updates the width prop for GridLayout so it can recalculate item positions
// without remounting the component tree (which would destroy child state).
function useContainerWidth() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const lastWidth = useRef(0);
  const rafId = useRef<number | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    // Debounce via requestAnimationFrame to avoid excessive re-renders
    // during continuous resize events (e.g. sidebar animation)
    const updateWidth = () => {
      if (rafId.current) cancelAnimationFrame(rafId.current);
      rafId.current = requestAnimationFrame(() => {
        const newWidth = container.clientWidth;
        if (newWidth > 0 && newWidth !== lastWidth.current) {
          lastWidth.current = newWidth;
          setWidth(newWidth);
        }
      });
    };

    const resizeObserver = new ResizeObserver(updateWidth);
    resizeObserver.observe(container);
    window.addEventListener('resize', updateWidth);

    // Initial measurement (synchronous for first paint)
    const initialWidth = container.clientWidth;
    if (initialWidth > 0) {
      lastWidth.current = initialWidth;
      setWidth(initialWidth);
    } else {
      // Container may not be laid out yet (e.g. fast tab switch, display:none parent).
      // Retry on next frames until we get a real width. (Fix 5.76)
      let retryFrame: number;
      const retryMeasure = () => {
        const w = container.clientWidth;
        if (w > 0) {
          lastWidth.current = w;
          setWidth(w);
        } else {
          retryFrame = requestAnimationFrame(retryMeasure);
        }
      };
      retryFrame = requestAnimationFrame(retryMeasure);
      // Capture cleanup for the retry loop
      const originalCleanup = () => cancelAnimationFrame(retryFrame);
      return () => {
        originalCleanup();
        if (rafId.current) cancelAnimationFrame(rafId.current);
        resizeObserver.disconnect();
        window.removeEventListener('resize', updateWidth);
      };
    }

    return () => {
      if (rafId.current) cancelAnimationFrame(rafId.current);
      resizeObserver.disconnect();
      window.removeEventListener('resize', updateWidth);
    };
  }, []);

  return { containerRef, width };
}

interface DashboardGridProps {
  results: AnalysisResult;
  filteredPositions: Position[];
  filterActive: boolean;
  selectedPosition: number | null;
  onSelectPosition: (position: number | null) => void;
  layout: Layout[];
  onLayoutChange: (layout: Layout[]) => void;
  hiddenPanels: string[];
  alphabet?: 'protein' | 'nucleotide';
  annotations?: Annotation[];
  onExportChart?: (dataUrl: string, chartType: string) => void;
}


export function DashboardGrid({
  results,
  filteredPositions,
  filterActive,
  selectedPosition,
  onSelectPosition,
  layout,
  onLayoutChange,
  hiddenPanels,
  alphabet = 'protein',
  annotations = [],
  onExportChart,
}: DashboardGridProps) {
  const [selectedVariant, setSelectedVariant] = useState<Variant | null>(null);
  const [hcsThreshold, setHcsThreshold] = useState(0);
  const [hoveredHcsRegion, setHoveredHcsRegion] = useState<number | null>(null);
  const [selectedHcsRegion, setSelectedHcsRegion] = useState<number | null>(null);
  const { containerRef, width } = useContainerWidth();
  // Ref to the EntropyLineChart's ReactECharts instance for chart image export.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const entropyChartRef = useRef<any>(null);

  // Clear stale region references whenever the threshold changes,
  // since region indices become invalid after the array is recomputed.
  useEffect(() => {
    setHoveredHcsRegion(null);
    setSelectedHcsRegion(null);
  }, [hcsThreshold]);

  // The "active" HCS region for the PDB viewer: hover previews take priority,
  // falling back to the persistently selected region.
  const activeHcsRegion = hoveredHcsRegion ?? selectedHcsRegion ?? null;

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
    return layout.filter((item) => {
      if (hiddenPanels.includes(item.i)) return false;
      // Exclude metadata-chart from layout when there are no metadata fields
      // to prevent an empty gap in the grid
      if (item.i === 'metadata-chart' && metadataFields.length === 0) return false;
      return true;
    });
  }, [layout, hiddenPanels, metadataFields.length]);

  // Persist layout only on drag/resize completion (not every pixel during drag).
  // Merges updated visible layout with hidden panel entries to prevent drops.
  const handleLayoutCommit = useCallback((updatedVisibleLayout: Layout[]) => {
    const hiddenLayout = layout.filter((item) => hiddenPanels.includes(item.i));
    onLayoutChange([...updatedVisibleLayout, ...hiddenLayout]);
  }, [layout, hiddenPanels, onLayoutChange]);

  // Capture auto-compacted positions from react-grid-layout (Fix 5.100).
  // This fires when the library internally recompacts due to panel hide/show.
  // We persist these to prevent layout jumps when panels reappear.
  const handleGridLayoutChange = useCallback((newLayout: Layout[]) => {
    const hiddenLayout = layout.filter((item) => hiddenPanels.includes(item.i));
    onLayoutChange([...newLayout, ...hiddenLayout]);
  }, [layout, hiddenPanels, onLayoutChange]);

  const handleVariantClick = useCallback((variant: Variant) => {
    setSelectedVariant(variant);
  }, []);

  // Compute HCS regions using shared utility (used by FeatureTrackViewer overlay and PDBViewer)
  const hcsRegions = useMemo(
    () => computeHCSRegions(results.results, hcsThreshold),
    [results.results, hcsThreshold]
  );

  // Compute the full MSA sequence length from the maximum position value.
  // Uses a loop (not Math.max(...array)) to avoid call-stack overflow on large datasets. (Fix 5.35)
  const sequenceLength = useMemo(() => {
    if (results.results.length === 0 || results.kmer_length <= 0) return 0;
    let maxPos = 0;
    for (const p of results.results) {
      if (p.position > maxPos) maxPos = p.position;
    }
    return maxPos + results.kmer_length - 1;
  }, [results.results, results.kmer_length]);

  // Wait for real measurement before rendering grid to avoid layout flash.
  // Container div always renders so ResizeObserver can measure it.
  const gridWidth = width;

  return (
    <div ref={containerRef} className="h-full w-full">
      {gridWidth === 0 ? null : (
      <GridLayout
        className="layout"
        layout={visibleLayout}
        cols={12}
        rowHeight={60}
        width={gridWidth}
        onDragStop={handleLayoutCommit}
        onResizeStop={handleLayoutCommit}
        onLayoutChange={handleGridLayoutChange}
        draggableHandle=".panel-handle"
        margin={[16, 16]}
        containerPadding={[16, 16]}
        compactType="vertical"
      >
        {!hiddenPanels.includes('entropy-line') && (
          <div key="entropy-line" className="h-full">
            <DashboardPanel
              title="Entropy Chart"
              subtitle={filterActive ? `Shannon entropy (${filteredPositions.length} of ${results.results.length} positions)` : "Shannon entropy with heatmap overview"}
              panelId="entropy-line"
              onExportChart={onExportChart ? () => {
                const instance = entropyChartRef.current?.getEchartsInstance();
                if (instance) {
                  const dataUrl = instance.getDataURL({ type: 'png', pixelRatio: 2, backgroundColor: '#fff' });
                  onExportChart(dataUrl, 'entropy');
                }
              } : undefined}
            >
              <ErrorBoundary compact label="Entropy Chart">
                <EntropyLineChart
                  positions={filteredPositions}
                  onSelectPosition={onSelectPosition}
                  averageEntropy={results.average_entropy ?? 0}
                  annotations={annotations}
                  selectedPosition={selectedPosition}
                  chartInstanceRef={entropyChartRef}
                />
              </ErrorBoundary>
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('position-explorer') && (
          <div key="position-explorer" className="h-full">
            <DashboardPanel title="Position Explorer" subtitle={selectedPosition ? `Details for k-mer position ${selectedPosition}` : 'Select a position to view details'} panelId="position-explorer">
              <ErrorBoundary compact label="Position Explorer">
                <PositionExplorer
                  position={selectedPositionData}
                  alphabet={alphabet}
                  onVariantClick={handleVariantClick}
                  annotations={annotations}
                />
              </ErrorBoundary>
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('variant-distribution') && (
          <div key="variant-distribution" className="h-full">
            <DashboardPanel title="Motif Distribution" subtitle={selectedPosition != null ? `Distribution of motifs at k-mer position ${selectedPosition}` : 'Select a position to view motifs'} panelId="variant-distribution">
              <ErrorBoundary compact label="Motif Distribution">
                <VariantBarChart
                  variants={selectedPositionData?.diversity_motifs || null}
                  totalVariantsIncidence={selectedPositionData?.total_variants_incidence || 0}
                  distinctVariantsIncidence={selectedPositionData?.distinct_variants_incidence || 0}
                />
              </ErrorBoundary>
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('metadata-chart') && metadataFields.length > 0 && (
          <div key="metadata-chart" className="h-full">
            <DashboardPanel title="Sequence Metadata" subtitle="Distribution by read count at selected position" panelId="metadata-chart">
              <ErrorBoundary compact label="Metadata Chart">
                <MetadataPieChart
                  variants={selectedPositionData?.diversity_motifs || null}
                  availableFields={metadataFields}
                />
              </ErrorBoundary>
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('hcs-map') && (
          <div key="hcs-map" className="h-full">
            <DashboardPanel title="Highly Conserved Sequences" subtitle="Regions with high index motif conservation" panelId="hcs-map">
              <ErrorBoundary compact label="HCS Map">
                <HCSMap
                  positions={results.results}
                  threshold={hcsThreshold}
                  onThresholdChange={setHcsThreshold}
                  kmerLength={results.kmer_length}
                  hoveredRegion={hoveredHcsRegion}
                  selectedRegion={selectedHcsRegion}
                  onHoverRegion={setHoveredHcsRegion}
                  onSelectRegion={setSelectedHcsRegion}
                />
              </ErrorBoundary>
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('pdb-viewer') && (
          <div key="pdb-viewer" className="h-full">
            <DashboardPanel title="3D Structure" subtitle="PDB structure with HCS highlighting" panelId="pdb-viewer">
              <ErrorBoundary compact label="PDB Viewer">
                <Suspense fallback={<PanelLoadingFallback />}>
                  <PDBViewer
                    positions={results.results}
                    hcsThreshold={hcsThreshold}
                    precomputedHcsRegions={hcsRegions}
                    activeHcsRegionIndex={activeHcsRegion}
                    selectedHcsRegionIndex={selectedHcsRegion}
                  />
                </Suspense>
              </ErrorBoundary>
            </DashboardPanel>
          </div>
        )}

        {!hiddenPanels.includes('feature-tracks') && (
          <div key="feature-tracks" className="h-full">
            <DashboardPanel title="Protein Features" subtitle="UniProt annotations aligned to MSA positions" panelId="feature-tracks">
              <ErrorBoundary compact label="Feature Tracks">
                <FeatureTrackViewer
                  hcsRegions={hcsRegions}
                  sequenceLength={sequenceLength}
                />
              </ErrorBoundary>
            </DashboardPanel>
          </div>
        )}
      </GridLayout>
      )}

      {/* Variant Detail Modal */}
      {selectedVariant && (
        <VariantModal
          variant={selectedVariant}
          alphabet={alphabet}
          onClose={() => setSelectedVariant(null)}
        />
      )}
    </div>
  );
}
