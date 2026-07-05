/**
 * FeatureCategoryChips - Pill-shaped toggles for feature category visibility.
 *
 * Pure presentational component. Renders one chip per category with a color
 * swatch, label, count, and active/inactive styling.
 */

import React from 'react';
import { FEATURE_CATEGORIES, FEATURE_CATEGORY_ORDER } from '@/lib/features';
import { hexToRgba } from '@/lib/colors';

interface FeatureCategoryChipsProps {
  /** Features grouped by category (for counts) */
  featureCounts: Map<string, number>;
  /** Currently visible category keys */
  visibleCategories: Set<string>;
  /** Toggle a category on/off */
  onToggle: (category: string) => void;
  /** Toggle all on/off */
  onToggleAll: (visible: boolean) => void;
}

export const FeatureCategoryChips = React.memo(function FeatureCategoryChips({
  featureCounts,
  visibleCategories,
  onToggle,
  onToggleAll,
}: FeatureCategoryChipsProps) {
  const allVisible = FEATURE_CATEGORY_ORDER.every((k) => visibleCategories.has(k));

  // Only render categories that have at least one feature
  const activeCategories = FEATURE_CATEGORY_ORDER.filter(
    (key) => (featureCounts.get(key) ?? 0) > 0
  );

  if (activeCategories.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-1.5 px-4 pt-3 pb-1">
      {/* All toggle */}
      <button
        onClick={() => onToggleAll(!allVisible)}
        className={`
          rounded-full border px-2.5 py-1 text-xs font-medium transition-all
          ${allVisible
            ? 'border-foreground/20 bg-foreground/10 text-foreground'
            : 'border-transparent bg-muted text-muted-foreground opacity-60 hover:opacity-100'}
        `}
        aria-label={allVisible ? 'Hide all feature categories' : 'Show all feature categories'}
      >
        All
      </button>

      {activeCategories.map((key) => {
        const config = FEATURE_CATEGORIES[key];
        const count = featureCounts.get(key) ?? 0;
        const isActive = visibleCategories.has(key);

        return (
          <button
            key={key}
            onClick={() => onToggle(key)}
            className={`
              flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium transition-all
              ${isActive
                ? 'opacity-100'
                : 'border-transparent bg-muted text-muted-foreground opacity-60 hover:opacity-100'}
            `}
            style={
              isActive
                ? {
                    backgroundColor: hexToRgba(config.color, 0.125),
                    borderColor: hexToRgba(config.color, 0.188),
                    color: config.color,
                  }
                : undefined
            }
            aria-label={`${config.label}, ${count} features, currently ${isActive ? 'visible' : 'hidden'}. Toggle to ${isActive ? 'hide' : 'show'}.`}
          >
            {/* Color swatch */}
            <span
              className="inline-block h-2 w-2 rounded-full shrink-0"
              style={{ backgroundColor: config.color }}
            />
            {config.label} ({count})
          </button>
        );
      })}
    </div>
  );
});
