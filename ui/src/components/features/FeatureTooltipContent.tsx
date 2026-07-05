/**
 * FeatureTooltipContent - Tooltip body for a hovered feature.
 *
 * Pure presentational component rendering feature type, description,
 * position range, and category.
 */

import React from 'react';
import type { MappedFeature } from '@/lib/types';
import { FEATURE_CATEGORIES } from '@/lib/features';

interface FeatureTooltipContentProps {
  feature: MappedFeature;
}

export const FeatureTooltipContent = React.memo(function FeatureTooltipContent({
  feature,
}: FeatureTooltipContentProps) {
  const config = FEATURE_CATEGORIES[feature.categoryKey];

  return (
    <div className="space-y-1 text-xs max-w-[240px]">
      <div className="flex items-center gap-1.5">
        <span
          className="inline-block h-2 w-2 rounded-full shrink-0"
          style={{ backgroundColor: config?.color ?? '#888' }}
        />
        <span className="font-semibold">{feature.feature_type}</span>
      </div>
      {feature.description && (
        <p className="text-muted-foreground">{feature.description}</p>
      )}
      <p>
        Positions {feature.begin}–{feature.end} (UniProt)
        {feature.msaBegin !== null && feature.msaEnd !== null && (
          <span className="text-muted-foreground">
            {' '}→ {feature.msaBegin}–{feature.msaEnd} (MSA)
          </span>
        )}
      </p>
      {config && (
        <p className="text-muted-foreground">{config.label}</p>
      )}
    </div>
  );
});
