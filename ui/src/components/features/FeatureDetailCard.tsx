/**
 * FeatureDetailCard - Detail popover for a selected feature.
 *
 * Shows full information about the selected feature with action buttons.
 */

import React from 'react';
import { Eye } from 'lucide-react';
import type { MappedFeature } from '@/lib/types';
import { FEATURE_CATEGORIES } from '@/lib/features';
import { Button } from '@/components/ui/button';

interface FeatureDetailCardProps {
  feature: MappedFeature;
  onDeselect: () => void;
  /** Callback to zoom the 3D viewer to this feature */
  onViewIn3D?: () => void;
}

export const FeatureDetailCard = React.memo(function FeatureDetailCard({
  feature,
  onDeselect,
  onViewIn3D,
}: FeatureDetailCardProps) {
  const config = FEATURE_CATEGORIES[feature.categoryKey];

  const handleCopy = async () => {
    const text = [
      `Type: ${feature.feature_type}`,
      `Description: ${feature.description || '(none)'}`,
      `UniProt positions: ${feature.begin}–${feature.end}`,
      feature.msaBegin !== null ? `MSA positions: ${feature.msaBegin}–${feature.msaEnd}` : '',
      `Category: ${config?.label ?? feature.categoryKey}`,
      feature.evidences.length > 0 ? `Evidence: ${feature.evidences.join(', ')}` : '',
    ]
      .filter(Boolean)
      .join('\n');
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const { useToastStore } = await import('@/stores/toastStore');
      useToastStore.getState().addToast('Could not copy to clipboard.', 'warning');
    }
  };

  return (
    <div
      className="border rounded-lg bg-card shadow-sm p-3 text-xs space-y-2"
      style={{ borderLeftColor: config?.color, borderLeftWidth: 3 }}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-1.5">
          <span
            className="inline-block h-2.5 w-2.5 rounded-full shrink-0"
            style={{ backgroundColor: config?.color ?? '#888' }}
          />
          <span className="font-semibold text-sm">{feature.feature_type}</span>
        </div>
        <button
          onClick={onDeselect}
          className="text-muted-foreground hover:text-foreground text-xs shrink-0"
          aria-label="Close feature detail"
        >
          ✕
        </button>
      </div>

      {feature.description && (
        <p>{feature.description}</p>
      )}

      <div className="text-muted-foreground space-y-0.5">
        <p>UniProt: {feature.begin}–{feature.end}</p>
        {feature.msaBegin !== null && feature.msaEnd !== null && (
          <p>MSA: {feature.msaBegin}–{feature.msaEnd}</p>
        )}
        {feature.evidences.length > 0 && (
          <p>Evidence: {feature.evidences.join(', ')}</p>
        )}
      </div>

      <div className="flex gap-2 pt-1">
        {onViewIn3D && (
          <Button variant="outline" size="sm" className="h-6 text-xs" onClick={onViewIn3D}>
            <Eye className="h-3 w-3 mr-1" />
            View in 3D
          </Button>
        )}
        <Button variant="outline" size="sm" className="h-6 text-xs" onClick={handleCopy}>
          Copy
        </Button>
      </div>
    </div>
  );
});
