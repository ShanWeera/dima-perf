/**
 * useFeatureHighlight3D - Manages 3Dmol feature overlay rendering.
 *
 * This is a CHEAP effect (split from the expensive structure-loading effect).
 * It applies incremental setStyle() calls to color feature residues without
 * rebuilding the entire scene. Runs on hover/select/category-toggle changes.
 *
 * IMPORTANT: Does NOT call viewer.removeAllModels() — that would destroy
 * the scene. Only re-styles targeted residue selections, then renders once.
 */

import { useEffect, useRef } from 'react';
import type { GLViewer } from '3dmol';
import type { MappedFeature } from '@/lib/types';
import type { HCSRegion } from '@/lib/hcs';
import { FEATURE_CATEGORIES } from '@/lib/features';

interface UseFeatureHighlight3DArgs {
  viewer: GLViewer | null;
  /** PDB chain currently being viewed */
  selectedChain: string;
  /** Existing MSA→PDB mapping */
  msaToPdb: Record<number, number> | null;
  /** All mapped features */
  mappedFeatures: MappedFeature[];
  /** Which categories are visible */
  visibleCategories: Set<string>;
  /** Currently hovered or selected feature (active feature) */
  activeFeature: MappedFeature | null;
  /** The persistently selected feature (zoom only on this, not hover) (Fix 5.78) */
  selectedFeature: MappedFeature | null;
  /** Whether the feature overlay toggle is ON */
  showFeatures: boolean;
  /** HCS regions — needed to restore green coloring when features are toggled off */
  hcsRegions: HCSRegion[];
  /** Monotonic counter that increments after each main scene rebuild (Fix 5.75).
   *  Forces re-application of feature overlay after the base scene is reconstructed. */
  sceneVersion?: number;
}

/**
 * Resolve a mapped feature's MSA positions to PDB residue numbers.
 */
function featureToPdbResidues(
  feature: MappedFeature,
  msaToPdb: Record<number, number>
): number[] {
  if (feature.msaBegin === null || feature.msaEnd === null) return [];
  const residues: number[] = [];
  for (let pos = feature.msaBegin; pos <= feature.msaEnd; pos++) {
    const pdbResi = msaToPdb[pos];
    if (pdbResi !== undefined) {
      residues.push(pdbResi);
    }
  }
  return residues;
}

export function useFeatureHighlight3D({
  viewer,
  selectedChain,
  msaToPdb,
  mappedFeatures,
  visibleCategories,
  activeFeature,
  selectedFeature,
  showFeatures,
  sceneVersion,
  hcsRegions,
}: UseFeatureHighlight3DArgs) {
  // Track previous state to know when to clean up labels
  const prevActiveRef = useRef<MappedFeature | null>(null);
  // Track which features were previously rendered so we can undo their coloring
  const prevRenderedFeaturesRef = useRef<MappedFeature[]>([]);
  // Track the last zoomed-to feature to avoid redundant zoom calls
  const lastZoomedFeatureRef = useRef<MappedFeature | null>(null);

  useEffect(() => {
    if (!viewer || !msaToPdb || !selectedChain) return;

    // Remove previous feature labels. This is safe because the main scene
    // effect in PDBViewer.tsx does not add persistent labels. If that changes,
    // labels should be tracked by group to avoid cross-effect interference.
    viewer.removeAllLabels();

    if (!showFeatures || mappedFeatures.length === 0) {
      // When features are toggled off, restore previously-colored residues back
      // to their base color (gray for non-HCS, green for HCS). This prevents
      // feature colors from "sticking" when the overlay is disabled.
      if (prevRenderedFeaturesRef.current.length > 0) {
        // Collect all HCS PDB residue numbers for quick lookup
        const hcsPdbResidues = new Set<number>();
        for (const region of hcsRegions) {
          for (const msaPos of region.indices) {
            const pdbResi = msaToPdb[msaPos];
            if (pdbResi !== undefined) hcsPdbResidues.add(pdbResi);
          }
        }

        for (const feature of prevRenderedFeaturesRef.current) {
          const pdbResidues = featureToPdbResidues(feature, msaToPdb);
          if (pdbResidues.length === 0) continue;

          // Split residues into HCS (restore green) and non-HCS (restore gray)
          const hcsResidues = pdbResidues.filter((r) => hcsPdbResidues.has(r));
          const nonHcsResidues = pdbResidues.filter((r) => !hcsPdbResidues.has(r));

          if (nonHcsResidues.length > 0) {
            viewer.setStyle(
              { chain: selectedChain, resi: nonHcsResidues },
              { cartoon: { color: 'lightgray' } }
            );
          }
          if (hcsResidues.length > 0) {
            viewer.setStyle(
              { chain: selectedChain, resi: hcsResidues },
              { cartoon: { color: 'green' }, stick: { color: 'green', radius: 0.15 } }
            );
          }
        }
        prevRenderedFeaturesRef.current = [];
      }
      viewer.render();
      prevActiveRef.current = null;
      return;
    }

    // Apply category-colored styles to visible feature residues.
    // NOTE: This intentionally overrides HCS green styling for residues that
    // have feature annotations, since feature colors provide more specific
    // functional context. When features are toggled OFF, the main scene effect
    // restores HCS coloring on the next full re-render cycle.
    for (const feature of mappedFeatures) {
      if (!visibleCategories.has(feature.categoryKey)) continue;

      const pdbResidues = featureToPdbResidues(feature, msaToPdb);
      if (pdbResidues.length === 0) continue;

      const config = FEATURE_CATEGORIES[feature.categoryKey];
      if (!config) continue;

      viewer.setStyle(
        { chain: selectedChain, resi: pdbResidues },
        {
          cartoon: { color: config.color },
          stick: { color: config.color, radius: 0.12 },
        }
      );
    }

    // Apply distinct highlight for the active (hovered/selected) feature
    if (activeFeature && activeFeature.msaBegin !== null && activeFeature.msaEnd !== null) {
      const activeResidues = featureToPdbResidues(activeFeature, msaToPdb);
      if (activeResidues.length > 0) {
        const config = FEATURE_CATEGORIES[activeFeature.categoryKey];
        const highlightColor = config?.color ?? '#FFD700';

        // Brighter style + thicker sticks for the active feature
        viewer.setStyle(
          { chain: selectedChain, resi: activeResidues },
          {
            cartoon: { color: highlightColor },
            stick: { color: highlightColor, radius: 0.25 },
          }
        );

        // Add a label at the midpoint residue
        const midIdx = Math.floor(activeResidues.length / 2);
        const labelText = activeFeature.description || activeFeature.feature_type;
        viewer.addLabel(labelText, {
          position: { resi: activeResidues[midIdx], chain: selectedChain },
          backgroundColor: 'rgba(0,0,0,0.6)',
          fontColor: 'white',
          fontSize: 11,
          backgroundOpacity: 0.7,
        } as Record<string, unknown>);

        // Only zoom on persistent SELECTION changes, not transient hovers (Fix 5.78).
        // Zoom on every hover is jarring during rapid track scrubbing.
        if (selectedFeature && selectedFeature === activeFeature &&
            lastZoomedFeatureRef.current !== selectedFeature) {
          viewer.zoomTo({ chain: selectedChain, resi: activeResidues }, 500);
          lastZoomedFeatureRef.current = selectedFeature;
        }
      }
    }

    viewer.render();
    prevActiveRef.current = activeFeature;
    // Track rendered features for cleanup when showFeatures is toggled off
    prevRenderedFeaturesRef.current = mappedFeatures.filter(
      (f) => visibleCategories.has(f.categoryKey)
    );

    // Cleanup on unmount: remove labels to prevent stale overlays (Fix 5.96)
    return () => {
      if (viewer) {
        viewer.removeAllLabels();
        viewer.render();
      }
    };
  }, [viewer, selectedChain, msaToPdb, mappedFeatures, visibleCategories, activeFeature, selectedFeature, showFeatures, hcsRegions, sceneVersion]);
}
