/**
 * FeatureOverlapBadges - Compact colored badges showing which features
 * overlap with a given HCS region.
 *
 * Displays up to 4 badges inline, with a "+N more" overflow popover.
 * Hovering or clicking a badge triggers the same feature hover/select
 * as interacting with the Feature Track panel.
 */

import React from 'react';
import type { MappedFeature } from '@/lib/types';
import { FEATURE_CATEGORIES, isPointFeature } from '@/lib/features';
import { hexToRgba } from '@/lib/colors';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';

interface FeatureOverlapBadgesProps {
  /** Features that overlap with this HCS region */
  overlappingFeatures: MappedFeature[];
  /** Callback when a badge is hovered */
  onHoverFeature: (feature: MappedFeature | null) => void;
  /** Callback when a badge is clicked */
  onSelectFeature: (feature: MappedFeature) => void;
}

const MAX_VISIBLE = 4;

function badgeLabel(f: MappedFeature): string {
  if (isPointFeature(f)) {
    // Point features: show type + position
    const shortType = f.feature_type.length > 6
      ? f.feature_type.slice(0, 5) + '.'
      : f.feature_type;
    return `${shortType} ${f.begin}`;
  }
  // Range features: abbreviated description
  const desc = f.description || f.feature_type;
  return desc.length > 12 ? desc.slice(0, 11) + '…' : desc;
}

export const FeatureOverlapBadges = React.memo(function FeatureOverlapBadges({
  overlappingFeatures,
  onHoverFeature,
  onSelectFeature,
}: FeatureOverlapBadgesProps) {
  if (overlappingFeatures.length === 0) return null;

  const visible = overlappingFeatures.slice(0, MAX_VISIBLE);
  const overflow = overlappingFeatures.slice(MAX_VISIBLE);

  return (
    <div className="flex flex-wrap items-center gap-1 mt-1.5">
      <span className="text-xs text-muted-foreground mr-0.5">Overlaps:</span>
      {visible.map((f, i) => {
        const config = FEATURE_CATEGORIES[f.categoryKey];
        return (
          <button
            key={`${f.feature_type}-${f.begin}-${i}`}
            className="rounded-full px-1.5 py-0.5 text-[10px] font-medium cursor-pointer transition-opacity hover:opacity-100 opacity-90"
            style={{
              backgroundColor: hexToRgba(config?.color ?? '#888', 0.12),
              color: config?.color ?? '#888',
            }}
            onMouseEnter={() => onHoverFeature(f)}
            onMouseLeave={() => onHoverFeature(null)}
            onClick={(e) => {
              e.stopPropagation();
              onSelectFeature(f);
            }}
          >
            {badgeLabel(f)}
          </button>
        );
      })}

      {overflow.length > 0 && (
        <Popover>
          <PopoverTrigger asChild>
            <button className="rounded-full bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground hover:bg-muted/80">
              +{overflow.length} more
            </button>
          </PopoverTrigger>
          <PopoverContent className="w-auto max-w-[220px] p-2" align="start">
            <div className="flex flex-wrap gap-1">
              {overflow.map((f, i) => {
                const config = FEATURE_CATEGORIES[f.categoryKey];
                return (
                  <button
                    key={`overflow-${f.feature_type}-${f.begin}-${i}`}
                    className="rounded-full px-1.5 py-0.5 text-[10px] font-medium cursor-pointer"
                    style={{
                      backgroundColor: hexToRgba(config?.color ?? '#888', 0.12),
                      color: config?.color ?? '#888',
                    }}
                    onMouseEnter={() => onHoverFeature(f)}
                    onMouseLeave={() => onHoverFeature(null)}
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelectFeature(f);
                    }}
                  >
                    {badgeLabel(f)}
                  </button>
                );
              })}
            </div>
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
});
