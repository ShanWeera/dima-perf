/**
 * DiMA Desktop - HCS (Highly Conserved Sequences) Visual Map
 * 
 * Shows conserved regions as colored blocks with sequence display.
 */

import { useMemo, useState, useCallback, useRef, useEffect, memo } from 'react';
import type { Position, MappedFeature } from '@/lib/types';
import { cn } from '@/lib/utils';
import { computeHCSRegions, type HCSRegion } from '@/lib/hcs';
import { useFeatureStore } from '@/stores/featureStore';
import { useFeatureOverlaps } from '@/hooks/useFeatureOverlaps';
import { FeatureOverlapBadges } from '@/components/features/FeatureOverlapBadges';

interface HCSMapProps {
  positions: Position[];
  threshold: number;
  onThresholdChange: (threshold: number) => void;
  /** K-mer length used in analysis — needed for correct feature overlap detection */
  kmerLength?: number;
  /** Index of the currently hovered HCS region (lifted to parent for cross-component sync) */
  hoveredRegion?: number | null;
  /** Index of the persistently selected HCS region */
  selectedRegion?: number | null;
  /** Callback when the user hovers/unhovers a region */
  onHoverRegion?: (index: number | null) => void;
  /** Callback when the user clicks to select/deselect a region */
  onSelectRegion?: (index: number | null) => void;
}

export const HCSMap = memo(function HCSMap({
  positions,
  threshold,
  onThresholdChange,
  kmerLength = 9,
  hoveredRegion = null,
  selectedRegion = null,
  onHoverRegion,
  onSelectRegion,
}: HCSMapProps) {
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    return () => { clearTimeout(copyTimerRef.current); };
  }, []);

  // Feature store: individual selectors to prevent cascade re-renders (Fix 5.82)
  const mappedFeatures = useFeatureStore((s) => s.mappedFeatures);
  const setHoveredFeature = useFeatureStore((s) => s.setHoveredFeature);
  const setSelectedFeature = useFeatureStore((s) => s.setSelectedFeature);

  const hcsRegions = useMemo(
    () => computeHCSRegions(positions, threshold),
    [positions, threshold]
  );

  // Compute which features overlap each HCS region (for overlap badges).
  // kmerLength corrects the overlap range to include trailing k-1 residues (Fix 5.65).
  const featureOverlaps = useFeatureOverlaps(hcsRegions, mappedFeatures, kmerLength);

  // Stable callbacks for feature overlap badge interactions
  const handleFeatureHover = useCallback(
    (f: MappedFeature | null) => setHoveredFeature(f),
    [setHoveredFeature]
  );
  const handleFeatureSelect = useCallback(
    (f: MappedFeature) => setSelectedFeature(f),
    [setSelectedFeature]
  );

  const handleCopy = async (region: HCSRegion, index: number) => {
    const fastaContent = `>HCS_${index + 1} (positions ${region.startPosition}-${region.endPosition})\n${region.sequence}`;
    try {
      await navigator.clipboard.writeText(fastaContent);
      setCopiedIndex(index);
      clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => setCopiedIndex(null), 2000);
    } catch {
      // Clipboard API may be unavailable in some webview contexts
      const { useToastStore } = await import('@/stores/toastStore');
      useToastStore.getState().addToast('Could not copy to clipboard.', 'warning');
    }
  };

  return (
    <div className="flex h-full flex-col gap-4 p-4">
      {/* Threshold Slider */}
      <div className="flex items-center gap-4">
        <label className="text-sm font-medium">
          Threshold: {threshold.toFixed(0)}%
        </label>
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={threshold}
          onChange={(e) => onThresholdChange(Number(e.target.value))}
          className="flex-1"
          aria-label="HCS threshold percentage"
        />
      </div>

      {/* Visual Map */}
      <div className="relative flex-1 overflow-hidden">
        {positions.length > 0 && (
          <div className="flex h-8 w-full rounded bg-muted">
            {hcsRegions.map((region, i) => {
              // Use the actual position span (first..last position) as the denominator,
              // not positions.length (count). This is correct for non-contiguous
              // or non-zero-origin positions. (Fix 6.5)
              const firstPos = positions[0].position;
              const lastPos = positions[positions.length - 1].position;
              const positionSpan = Math.max(lastPos - firstPos + 1, 1);
              const startPct = ((region.startPosition - firstPos) / positionSpan) * 100;
              const widthPct = ((region.endPosition - region.startPosition + 1) / positionSpan) * 100;
              const isHovered = hoveredRegion === i;
              const isSelected = selectedRegion === i;
              const hasLowSupport = region.lowSupportPositions.length > 0;
              
              return (
                <div
                  key={`${region.startPosition}-${region.endPosition}`}
                  role="button"
                  tabIndex={0}
                  aria-label={`HCS region ${i + 1}: positions ${region.startPosition} to ${region.endPosition}${hasLowSupport ? ` (${region.lowSupportPositions.length} low-support)` : ''}`}
                  aria-pressed={isSelected}
                  className={cn(
                    "absolute h-full cursor-pointer rounded transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-1",
                    isSelected ? "bg-yellow-400" : isHovered ? "bg-green-400" : hasLowSupport ? "bg-green-500/70" : "bg-green-500",
                    hasLowSupport && "border border-dashed border-orange-400"
                  )}
                  style={{
                    left: `${startPct}%`,
                    width: `${Math.max(widthPct, 0.5)}%`,
                  }}
                  onMouseEnter={() => onHoverRegion?.(i)}
                  onMouseLeave={() => onHoverRegion?.(null)}
                  onClick={() => onSelectRegion?.(i === selectedRegion ? null : i)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onSelectRegion?.(i === selectedRegion ? null : i);
                    }
                  }}
                  title={`HCS ${i + 1}: Positions ${region.startPosition}-${region.endPosition} (click to ${isSelected ? 'deselect' : 'select'})`}
                />
              );
            })}
          </div>
        )}
      </div>

      {/* Region List */}
      <div className="flex-1 overflow-auto">
        {hcsRegions.length === 0 ? (
          <p className="text-center text-sm text-muted-foreground">
            No HCS regions found at {threshold.toFixed(0)}% threshold
          </p>
        ) : (
          <div className="space-y-2">
            {hcsRegions.map((region, i) => {
              const isHovered = hoveredRegion === i;
              const isSelected = selectedRegion === i;

              const regionHasLowSupport = region.lowSupportPositions.length > 0;

              return (
                <div
                  key={`${region.startPosition}-${region.endPosition}`}
                  className={cn(
                    "relative rounded-lg border p-3 transition-colors focus-within:ring-2 focus-within:ring-ring",
                    isSelected && "border-yellow-500 bg-yellow-500/10 ring-2 ring-yellow-400/50",
                    isHovered && !isSelected && "border-green-500 bg-green-500/5",
                    isHovered && isSelected && "border-yellow-500 bg-yellow-500/15 ring-2 ring-yellow-400/50",
                    regionHasLowSupport && !isSelected && !isHovered && "border-l-2 border-l-orange-400 border-dashed"
                  )}
                  onMouseEnter={() => onHoverRegion?.(i)}
                  onMouseLeave={() => onHoverRegion?.(null)}
                >
                  {/* Stretched-link select button — covers the full card area (WCAG 4.1.2 fix: 
                      no nested interactive elements). The Copy button sits above via z-index. */}
                  <button
                    className="absolute inset-0 z-0 cursor-pointer rounded-lg focus:outline-none"
                    aria-label={`HCS region ${i + 1}: positions ${region.startPosition} to ${region.endPosition}, ${region.sequence.length} residues${regionHasLowSupport ? `, ${region.lowSupportPositions.length} low-support positions` : ''}`}
                    aria-pressed={isSelected}
                    onClick={() => onSelectRegion?.(i === selectedRegion ? null : i)}
                  />
                  <div className="relative z-[1] flex items-center justify-between">
                    <span className="font-medium flex items-center gap-2">
                      HCS {i + 1}: Positions {region.startPosition}-{region.endPosition}
                      {isSelected && (
                        <span className="text-xs bg-yellow-500/20 text-yellow-700 dark:text-yellow-300 px-1.5 py-0.5 rounded-full font-normal">
                          Selected
                        </span>
                      )}
                      {region.lowSupportPositions.length > 0 && (
                        <span
                          className="text-xs bg-orange-500/20 text-orange-700 dark:text-orange-300 px-1.5 py-0.5 rounded-full font-normal inline-flex items-center gap-1"
                          title={`${region.lowSupportPositions.length} position(s) with low support (NS/LS) — results at these positions may not be statistically reliable`}
                        >
                          ⚠ {region.lowSupportPositions.length} low-support
                        </span>
                      )}
                    </span>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleCopy(region, i);
                      }}
                      aria-label={`Copy FASTA sequence for HCS region ${i + 1}`}
                      className="rounded px-2 py-1 text-xs hover:bg-muted focus:outline-none focus:ring-2 focus:ring-ring"
                    >
                      {copiedIndex === i ? 'Copied!' : 'Copy FASTA'}
                    </button>
                  </div>
                  <p className="relative z-[1] mt-1 font-mono text-sm text-muted-foreground">
                    {region.sequence.length > 50
                      ? region.sequence.slice(0, 50) + '...'
                      : region.sequence}
                  </p>
                  {/* Feature overlap badges (only rendered when overlaps exist) */}
                  {featureOverlaps.get(i) && (
                    <div className="relative z-[1]">
                      <FeatureOverlapBadges
                        overlappingFeatures={featureOverlaps.get(i)!}
                        onHoverFeature={handleFeatureHover}
                        onSelectFeature={handleFeatureSelect}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
});
