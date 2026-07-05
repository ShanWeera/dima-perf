/**
 * FeatureTrackViewer - Container component for the Protein Features panel.
 *
 * Connects to the feature store, computes derived data, and delegates
 * rendering to pure presentational children. Handles empty/loading/error
 * states and the category chip bar.
 */

import { useState, useMemo, useCallback, useRef, useEffect, memo } from 'react';
import { Layers, Loader2, AlertCircle, Search, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import type { MappedFeature } from '@/lib/types';
import type { HCSRegionSimple } from '@/lib/hcs';
import { FEATURE_CATEGORY_ORDER } from '@/lib/features';
import { useFeatureStore } from '@/stores/featureStore';
import { useDebouncedHover } from '@/hooks/useDebouncedHover';
import { FeatureCategoryChips } from './FeatureCategoryChips';
import { FeatureTrackSVG } from './FeatureTrackSVG';
import { FeatureDetailCard } from './FeatureDetailCard';

interface FeatureTrackViewerProps {
  /** HCS regions for the background overlay */
  hcsRegions: HCSRegionSimple[];
  /** Total MSA sequence length */
  sequenceLength: number;
}

export const FeatureTrackViewer = memo(function FeatureTrackViewer({
  hcsRegions,
  sequenceLength,
}: FeatureTrackViewerProps) {
  // Store state — individual selectors prevent cascade re-renders (Fix 5.82)
  const uniprotInfo = useFeatureStore((s) => s.uniprotInfo);
  const mappedFeatures = useFeatureStore((s) => s.mappedFeatures);
  const visibleCategories = useFeatureStore((s) => s.visibleCategories);
  const hoveredFeature = useFeatureStore((s) => s.hoveredFeature);
  const selectedFeature = useFeatureStore((s) => s.selectedFeature);
  const isLoading = useFeatureStore((s) => s.isLoading);
  const error = useFeatureStore((s) => s.error);
  const setHoveredFeature = useFeatureStore((s) => s.setHoveredFeature);
  const setSelectedFeature = useFeatureStore((s) => s.setSelectedFeature);
  const toggleCategory = useFeatureStore((s) => s.toggleCategory);
  const setAllCategoriesVisible = useFeatureStore((s) => s.setAllCategoriesVisible);
  const fetchFeaturesByAccession = useFeatureStore((s) => s.fetchFeaturesByAccession);

  // Local state for manual accession input in empty state
  const [manualAccession, setManualAccession] = useState('');

  // Feature search filter
  const [searchQuery, setSearchQuery] = useState('');

  // Container width measurement. (Fix 2.11)
  // Uses a callback ref + ResizeObserver instead of useLayoutEffect with [] deps.
  // The old approach ran once on mount when containerRef was null (due to early-return
  // UI paths for loading/error/empty states), and never re-ran when data loaded and
  // the actual container div mounted — so the ResizeObserver never attached.
  // Mutable ref (initialized to null explicitly) so we can assign .current from
  // the callback ref without TS readonly complaints.
  const containerRef = useRef<HTMLDivElement | null>(null);
  const observerRef = useRef<ResizeObserver | null>(null);
  const [containerWidth, setContainerWidth] = useState(0);

  const containerCallbackRef = useCallback((node: HTMLDivElement | null) => {
    if (observerRef.current) {
      observerRef.current.disconnect();
      observerRef.current = null;
    }

    containerRef.current = node;
    if (!node) return;

    // Measure immediately to avoid 0px → actual width flash
    const initialWidth = node.getBoundingClientRect().width;
    if (initialWidth > 0) setContainerWidth(initialWidth);

    // Observe future resizes
    observerRef.current = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width ?? 0;
      if (w > 0) setContainerWidth(w);
    });
    observerRef.current.observe(node);
  }, []);

  // Cleanup observer on unmount
  useEffect(() => {
    return () => {
      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }
    };
  }, []);

  // Filter features by search query (matches description or feature type)
  const filteredFeatures = useMemo(() => {
    if (!searchQuery.trim()) return mappedFeatures;
    const q = searchQuery.toLowerCase();
    return mappedFeatures.filter(
      (f) =>
        f.description.toLowerCase().includes(q) ||
        f.feature_type.toLowerCase().includes(q) ||
        f.categoryKey.toLowerCase().includes(q)
    );
  }, [mappedFeatures, searchQuery]);

  // Feature counts per category (for chips).
  // Uses ALL mapped features (not search-filtered) so chips remain visible even
  // when the current search query filters out all features in a category. This
  // lets users always see and toggle categories regardless of search state.
  const featureCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const f of mappedFeatures) {
      counts.set(f.categoryKey, (counts.get(f.categoryKey) ?? 0) + 1);
    }
    return counts;
  }, [mappedFeatures]);

  // Debounced hover for performance
  const { handleMouseEnter, handleMouseLeave } = useDebouncedHover<MappedFeature>(
    setHoveredFeature
  );

  // Click handler: toggle selection
  const handleClick = useCallback(
    (feature: MappedFeature) => setSelectedFeature(feature),
    [setSelectedFeature]
  );

  // Stable memoized list of features that are navigable via keyboard.
  // Only features that have valid MSA positions and are in a visible category qualify.
  // This avoids re-computing the list inline on every key press and prevents
  // focusedIndex from drifting when the list changes between presses.
  const navigableFeatures = useMemo(
    () => filteredFeatures.filter(
      (f) => f.msaBegin !== null && visibleCategories.has(f.categoryKey)
    ),
    [filteredFeatures, visibleCategories]
  );

  // Keyboard navigation: Tab focuses the track panel, then arrow keys cycle
  // through features, Enter selects, Escape deselects.
  const [focusedIndex, setFocusedIndex] = useState<number>(-1);

  // Reset focused index when the navigable list changes (search, category toggle, etc.)
  // to prevent the index from pointing to a stale or non-existent feature.
  useEffect(() => {
    setFocusedIndex(-1);
  }, [navigableFeatures]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (navigableFeatures.length === 0) return;

      switch (e.key) {
        case 'ArrowRight':
        case 'ArrowDown': {
          e.preventDefault();
          // When unfocused (-1), start at the first feature
          const next = focusedIndex === -1
            ? 0
            : Math.min(focusedIndex + 1, navigableFeatures.length - 1);
          setFocusedIndex(next);
          setHoveredFeature(navigableFeatures[next]);
          break;
        }
        case 'ArrowLeft':
        case 'ArrowUp': {
          e.preventDefault();
          // When unfocused (-1), start at the last feature (natural for "back")
          const prev = focusedIndex === -1
            ? navigableFeatures.length - 1
            : Math.max(focusedIndex - 1, 0);
          setFocusedIndex(prev);
          setHoveredFeature(navigableFeatures[prev]);
          break;
        }
        case 'Enter': {
          e.preventDefault();
          if (focusedIndex >= 0 && focusedIndex < navigableFeatures.length) {
            setSelectedFeature(navigableFeatures[focusedIndex]);
          }
          break;
        }
        case 'Escape': {
          e.preventDefault();
          setSelectedFeature(null);
          setHoveredFeature(null);
          setFocusedIndex(-1);
          break;
        }
      }
    },
    [navigableFeatures, focusedIndex, setHoveredFeature, setSelectedFeature]
  );

  // ------- Empty state: no features loaded -------
  if (!uniprotInfo && !isLoading && !error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-4 text-center">
        <Layers className="h-12 w-12 text-muted-foreground/50" />
        <div>
          <p className="font-medium text-sm">No protein features loaded</p>
          <p className="text-xs text-muted-foreground mt-1">
            Load a PDB structure to auto-detect UniProt features,
            or enter a UniProt accession manually.
          </p>
        </div>
        <div className="flex gap-2 items-center">
          <Input
            value={manualAccession}
            onChange={(e) => setManualAccession(e.target.value.toUpperCase())}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && manualAccession.trim()) {
                fetchFeaturesByAccession(manualAccession.trim());
                setManualAccession('');
              }
            }}
            placeholder="UniProt accession"
            className="h-8 w-36 text-sm"
          />
          <Button
            size="sm"
            className="h-8"
            disabled={!manualAccession.trim()}
            onClick={() => {
              fetchFeaturesByAccession(manualAccession.trim());
              setManualAccession('');
            }}
          >
            Fetch
          </Button>
        </div>
      </div>
    );
  }

  // ------- Loading state -------
  if (isLoading) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-4">
        <div className="space-y-2 w-full max-w-md">
          <div className="h-5 w-full animate-pulse rounded bg-muted" />
          <div className="h-5 w-full animate-pulse rounded bg-muted" />
          <div className="h-5 w-3/4 animate-pulse rounded bg-muted" />
        </div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Loader2 className="h-3 w-3 animate-spin" />
          Loading protein features from UniProt...
        </div>
      </div>
    );
  }

  // ------- Error state -------
  if (error && !uniprotInfo) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-4 text-center">
        <AlertCircle className="h-10 w-10 text-amber-500" />
        <div>
          <p className="font-medium text-sm">Could not load protein features</p>
          <p className="text-xs text-muted-foreground mt-1">
            {error}
          </p>
        </div>
        <div className="flex gap-2 items-center">
          <Input
            value={manualAccession}
            onChange={(e) => setManualAccession(e.target.value.toUpperCase())}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && manualAccession.trim()) {
                fetchFeaturesByAccession(manualAccession.trim());
                setManualAccession('');
              }
            }}
            placeholder="UniProt accession"
            className="h-8 w-36 text-sm"
          />
          <Button
            size="sm"
            className="h-8"
            disabled={!manualAccession.trim()}
            onClick={() => {
              fetchFeaturesByAccession(manualAccession.trim());
              setManualAccession('');
            }}
          >
            Try Again
          </Button>
        </div>
      </div>
    );
  }

  // ------- Features loaded: render tracks -------
  const allHidden = FEATURE_CATEGORY_ORDER.every((k) => !visibleCategories.has(k));

  return (
    <div ref={containerCallbackRef} className="flex h-full flex-col overflow-hidden">
      {/* Toolbar: category chips + search */}
      <div className="flex items-center gap-2 flex-wrap">
        <FeatureCategoryChips
          featureCounts={featureCounts}
          visibleCategories={visibleCategories}
          onToggle={toggleCategory}
          onToggleAll={setAllCategoriesVisible}
        />
        <div className="relative ml-auto mr-4 mt-2">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search features..."
            className="h-7 w-40 pl-7 pr-6 text-xs"
            aria-label="Search protein features"
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery('')}
              className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-sm p-0.5 text-muted-foreground hover:text-foreground"
              aria-label="Clear search"
            >
              <X className="h-3 w-3" />
            </button>
          )}
        </div>
      </div>

      {/* Track area (keyboard-focusable for arrow key navigation) */}
      <div
        className="flex-1 overflow-auto min-h-0 px-2 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 rounded outline-none"
        tabIndex={0}
        onKeyDown={handleKeyDown}
        role="group"
        aria-label="Protein feature tracks"
      >
        {allHidden ? (
          <p className="text-center text-xs text-muted-foreground py-4">
            All feature categories are hidden. Click a category chip above to show features.
          </p>
        ) : filteredFeatures.length === 0 ? (
          <p className="text-center text-xs text-muted-foreground py-4">
            {searchQuery.trim()
              ? `No features match "${searchQuery}". Try a different search.`
              : 'No features could be mapped to MSA positions. Check the sequence alignment.'}
          </p>
        ) : (
          <FeatureTrackSVG
            features={filteredFeatures}
            visibleCategories={visibleCategories}
            sequenceLength={sequenceLength}
            width={containerWidth > 0 ? containerWidth - 16 : 600}
            hcsRegions={hcsRegions}
            hoveredFeature={hoveredFeature}
            selectedFeature={selectedFeature}
            onHover={handleMouseEnter}
            onLeave={handleMouseLeave}
            onClick={handleClick}
          />
        )}
      </div>

      {/* Selected feature detail card (anchored at bottom) */}
      {selectedFeature && (
        <div className="shrink-0 px-4 pb-3">
          <FeatureDetailCard
            feature={selectedFeature}
            onDeselect={() => setSelectedFeature(null)}
            onViewIn3D={() => {
              // Re-trigger the 3D zoom by briefly clearing and re-setting the
              // selected feature. This forces useFeatureHighlight3D to re-run
              // its zoom animation even if the user panned away in the 3D viewer.
              const feat = selectedFeature;
              setSelectedFeature(null);
              requestAnimationFrame(() => setSelectedFeature(feat));
            }}
          />
        </div>
      )}
    </div>
  );
});
