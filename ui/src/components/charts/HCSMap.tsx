/**
 * DiMA Desktop - HCS (Highly Conserved Sequences) Visual Map
 * 
 * Shows conserved regions as colored blocks with sequence display.
 */

import { useMemo, useState } from 'react';
import type { Position } from '@/lib/types';
import { cn } from '@/lib/utils';

interface HCSMapProps {
  positions: Position[];
  threshold: number;
  onThresholdChange: (threshold: number) => void;
}

interface HCSRegion {
  startPosition: number;
  endPosition: number;
  sequence: string;
  indices: number[];
}

export function HCSMap({
  positions,
  threshold,
  onThresholdChange,
}: HCSMapProps) {
  const [hoveredRegion, setHoveredRegion] = useState<number | null>(null);
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  // Find HCS regions (consecutive Index motifs above threshold)
  const hcsRegions: HCSRegion[] = useMemo(() => {
    const regions: HCSRegion[] = [];
    let currentRegion: HCSRegion | null = null;

    positions.forEach((pos) => {
      const indexVariant = pos.diversity_motifs?.find(
        (v) => v.motif_short === 'I' && v.incidence >= threshold
      );

      if (indexVariant) {
        if (currentRegion) {
          // Extend current region
          currentRegion.endPosition = pos.position;
          currentRegion.sequence += indexVariant.sequence.slice(-1); // Add last char
          currentRegion.indices.push(pos.position);
        } else {
          // Start new region
          currentRegion = {
            startPosition: pos.position,
            endPosition: pos.position,
            sequence: indexVariant.sequence,
            indices: [pos.position],
          };
        }
      } else {
        // End current region if any
        if (currentRegion && currentRegion.indices.length > 1) {
          regions.push(currentRegion);
        }
        currentRegion = null;
      }
    });

    // Don't forget last region
    if (currentRegion !== null && (currentRegion as HCSRegion).indices.length > 1) {
      regions.push(currentRegion);
    }

    return regions;
  }, [positions, threshold]);

  const handleCopy = async (region: HCSRegion, index: number) => {
    const fastaContent = `>HCS_${index + 1} (positions ${region.startPosition}-${region.endPosition})\n${region.sequence}`;
    await navigator.clipboard.writeText(fastaContent);
    setCopiedIndex(index);
    setTimeout(() => setCopiedIndex(null), 2000);
  };

  return (
    <div className="flex h-full flex-col gap-4 p-4">
      {/* Threshold Slider */}
      <div className="flex items-center gap-4">
        <label className="text-sm font-medium">
          Threshold: {(threshold * 100).toFixed(0)}%
        </label>
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={threshold * 100}
          onChange={(e) => onThresholdChange(Number(e.target.value) / 100)}
          className="flex-1"
        />
      </div>

      {/* Visual Map */}
      <div className="relative flex-1 overflow-hidden">
        {positions.length > 0 && (
          <div className="flex h-8 w-full rounded bg-muted">
            {hcsRegions.map((region, i) => {
              const startPct = ((region.startPosition - positions[0].position) / positions.length) * 100;
              const widthPct = ((region.endPosition - region.startPosition + 1) / positions.length) * 100;
              
              return (
                <div
                  key={i}
                  className={cn(
                    "absolute h-full cursor-pointer rounded transition-colors",
                    hoveredRegion === i ? "bg-green-400" : "bg-green-500"
                  )}
                  style={{
                    left: `${startPct}%`,
                    width: `${Math.max(widthPct, 0.5)}%`,
                  }}
                  onMouseEnter={() => setHoveredRegion(i)}
                  onMouseLeave={() => setHoveredRegion(null)}
                  onClick={() => handleCopy(region, i)}
                  title={`HCS ${i + 1}: Positions ${region.startPosition}-${region.endPosition}`}
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
            No HCS regions found at {(threshold * 100).toFixed(0)}% threshold
          </p>
        ) : (
          <div className="space-y-2">
            {hcsRegions.map((region, i) => (
              <div
                key={i}
                className={cn(
                  "rounded-lg border p-3 transition-colors",
                  hoveredRegion === i && "border-green-500 bg-green-500/5"
                )}
                onMouseEnter={() => setHoveredRegion(i)}
                onMouseLeave={() => setHoveredRegion(null)}
              >
                <div className="flex items-center justify-between">
                  <span className="font-medium">
                    HCS {i + 1}: Positions {region.startPosition}-{region.endPosition}
                  </span>
                  <button
                    onClick={() => handleCopy(region, i)}
                    className="rounded px-2 py-1 text-xs hover:bg-muted"
                  >
                    {copiedIndex === i ? 'Copied!' : 'Copy FASTA'}
                  </button>
                </div>
                <p className="mt-1 font-mono text-sm text-muted-foreground">
                  {region.sequence.length > 50
                    ? region.sequence.slice(0, 50) + '...'
                    : region.sequence}
                </p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
