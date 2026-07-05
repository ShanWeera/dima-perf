/**
 * DiMA Desktop - Feature Store
 *
 * Manages UniProt protein feature annotations state. Handles:
 * - Auto-detection of UniProt accession from PDB entities
 * - Fetching features from the UniProt Proteins API
 * - Hover/select state for cross-component synchronization
 * - Visible category toggles
 */

import { create } from 'zustand';
import type { UniProtInfo, MappedFeature } from '@/lib/types';
import { fetchUniProtAccession, fetchUniProtFeatures } from '@/lib/tauri';
import { FEATURE_CATEGORY_ORDER } from '@/lib/features';
import { showErrorToast, extractErrorMessage } from '@/lib/utils';

// Generation counter prevents a slow fetch from overwriting results
// from a newer fetch that completed first.
let featureFetchGeneration = 0;

interface FeatureState {
  // Resolved UniProt data
  uniprotInfo: UniProtInfo | null;

  // Features mapped to MSA coordinates (set externally by useFeatureMapping)
  mappedFeatures: MappedFeature[];

  // UI state
  visibleCategories: Set<string>;
  hoveredFeature: MappedFeature | null;
  selectedFeature: MappedFeature | null;

  // Loading / error
  isLoading: boolean;
  error: string | null;

  // Actions (async)
  fetchFeaturesFromPdb: (pdbId: string, entityId: number) => Promise<void>;
  fetchFeaturesByAccession: (accession: string) => Promise<void>;

  // Actions (sync)
  setMappedFeatures: (features: MappedFeature[]) => void;
  setHoveredFeature: (feature: MappedFeature | null) => void;
  setSelectedFeature: (feature: MappedFeature | null) => void;
  toggleCategory: (category: string) => void;
  setAllCategoriesVisible: (visible: boolean) => void;
  clearFeatures: () => void;
}

export const useFeatureStore = create<FeatureState>((set, get) => ({
  // Initial state
  uniprotInfo: null,
  mappedFeatures: [],
  visibleCategories: new Set(FEATURE_CATEGORY_ORDER),
  hoveredFeature: null,
  selectedFeature: null,
  isLoading: false,
  error: null,

  /**
   * Auto-detect flow: PDB ID + entity -> RCSB lookup -> UniProt fetch.
   * Non-blocking; sets loading/error state for the UI to react to.
   * Note: isLoading is set once here; the internal _fetchFeaturesImpl call
   * does NOT re-set it, avoiding a redundant state update + render.
   */
  fetchFeaturesFromPdb: async (pdbId, entityId) => {
    const gen = ++featureFetchGeneration;
    // Clear stale mapped features immediately so the UI shows loading rather
    // than the previous protein's features during the fetch window. (Fix 5.30)
    set({ isLoading: true, error: null, mappedFeatures: [], hoveredFeature: null, selectedFeature: null });
    try {
      const accession = await fetchUniProtAccession(pdbId, entityId);
      if (gen !== featureFetchGeneration) return;
      const info = await fetchUniProtFeatures(accession);
      if (gen !== featureFetchGeneration) return;
      set({
        uniprotInfo: info,
        isLoading: false,
        error: null,
        hoveredFeature: null,
        selectedFeature: null,
        visibleCategories: new Set(FEATURE_CATEGORY_ORDER),
      });
    } catch (err) {
      if (gen !== featureFetchGeneration) return;
      const message = extractErrorMessage(err) ?? 'Failed to load features';
      set({
        uniprotInfo: null,
        mappedFeatures: [],
        isLoading: false,
        error: message,
      });
      showErrorToast('Failed to load UniProt features from PDB', err);
    }
  },

  /**
   * Manual entry flow: directly fetch features for a known accession.
   */
  fetchFeaturesByAccession: async (accession) => {
    const gen = ++featureFetchGeneration;
    set({ isLoading: true, error: null, mappedFeatures: [], hoveredFeature: null, selectedFeature: null });
    try {
      const info = await fetchUniProtFeatures(accession);
      if (gen !== featureFetchGeneration) return;
      set({
        uniprotInfo: info,
        isLoading: false,
        error: null,
        hoveredFeature: null,
        selectedFeature: null,
        visibleCategories: new Set(FEATURE_CATEGORY_ORDER),
      });
    } catch (err) {
      if (gen !== featureFetchGeneration) return;
      const message = extractErrorMessage(err) ?? 'Failed to load features';
      set({
        uniprotInfo: null,
        mappedFeatures: [],
        isLoading: false,
        error: message,
      });
      showErrorToast('Failed to load UniProt features', err);
    }
  },

  setMappedFeatures: (features) => set({ mappedFeatures: features }),

  setHoveredFeature: (feature) => set({ hoveredFeature: feature }),

  setSelectedFeature: (feature) => {
    const current = get().selectedFeature;
    // Toggle off if clicking the same feature.
    // Includes description and categoryKey so features at the same position
    // but with different annotations (e.g. two BINDING sites for different
    // ligands) are distinguished correctly.
    if (
      current &&
      feature &&
      current.begin === feature.begin &&
      current.end === feature.end &&
      current.feature_type === feature.feature_type &&
      current.description === feature.description &&
      current.categoryKey === feature.categoryKey
    ) {
      set({ selectedFeature: null });
    } else {
      set({ selectedFeature: feature });
    }
  },

  toggleCategory: (category) => {
    const prev = get().visibleCategories;
    const next = new Set(prev);
    if (next.has(category)) {
      next.delete(category);
    } else {
      next.add(category);
    }
    set({ visibleCategories: next });
  },

  setAllCategoriesVisible: (visible) => {
    set({
      visibleCategories: visible
        ? new Set(FEATURE_CATEGORY_ORDER)
        : new Set<string>(),
    });
  },

  clearFeatures: () => {
    // Bump generation to invalidate any in-flight fetch that hasn't resolved yet.
    // Without this, a slow fetch could overwrite the cleared state after reset.
    featureFetchGeneration++;
    set({
      uniprotInfo: null,
      mappedFeatures: [],
      hoveredFeature: null,
      selectedFeature: null,
      visibleCategories: new Set(FEATURE_CATEGORY_ORDER),
      error: null,
      isLoading: false,
    });
  },
}));
