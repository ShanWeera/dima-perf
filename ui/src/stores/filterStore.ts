/**
 * DiMA Desktop - Filter Store
 *
 * Manages search filters and filter presets with persistence.
 * All optimistic mutations revert on IPC failure to keep memory
 * consistent with what the backend actually has on disk.
 */

import { create } from 'zustand';
import type { SearchFilters, FilterPreset } from '@/lib/types';
import {
  saveFilters,
  loadFilters,
  saveFilterPresets,
  loadFilterPresets,
} from '@/lib/tauri';
import {
  DEFAULT_FILTERS,
  createPreset,
} from '@/lib/filters';
import { showErrorToast } from '@/lib/utils';
import { useToastStore } from './toastStore';

/**
 * Deep-clone a SearchFilters object so nested arrays (motifTypes,
 * positionRange, entropyRange) are not shared between store slices.
 */
function cloneFilters(filters: SearchFilters): SearchFilters {
  return {
    ...filters,
    motifTypes: [...filters.motifTypes],
    positionRange: filters.positionRange ? [...filters.positionRange] as [number, number] : null,
    entropyRange: filters.entropyRange ? [...filters.entropyRange] as [number, number] : null,
  };
}

interface FilterState {
  filters: SearchFilters;
  currentProjectPath: string | null;
  presets: FilterPreset[];
  isLoading: boolean;

  initializeForProject: (projectPath: string) => Promise<void>;
  clearProject: () => void;

  setFilters: (filters: SearchFilters) => void;
  resetFilters: () => void;

  savePreset: (name: string) => Promise<void>;
  loadPreset: (preset: FilterPreset) => void;
  deletePreset: (id: string) => Promise<void>;
  loadGlobalPresets: () => Promise<void>;
}

// Monotonic counter to prevent stale filter loads from corrupting state
// during rapid project switches (race condition guard).
let filterInitGeneration = 0;

export const useFilterStore = create<FilterState>((set, get) => ({
  filters: { ...DEFAULT_FILTERS },
  currentProjectPath: null,
  presets: [],
  isLoading: false,

  initializeForProject: async (projectPath) => {
    const thisGen = ++filterInitGeneration;
    set({ isLoading: true, currentProjectPath: projectPath });

    try {
      const savedFilters = await loadFilters(projectPath);
      // Discard if a newer initialization started (project switch race)
      if (thisGen !== filterInitGeneration) return;
      set({ filters: savedFilters ? cloneFilters(savedFilters) : { ...DEFAULT_FILTERS } });
    } catch (error) {
      if (thisGen !== filterInitGeneration) return;
      showErrorToast('Failed to load project filters', error);
      set({ filters: { ...DEFAULT_FILTERS } });
    }

    // Load global presets independently — failure here must not wipe filters
    try {
      const presets = await loadFilterPresets();
      if (thisGen !== filterInitGeneration) return;
      set({ presets });
    } catch (error) {
      if (thisGen !== filterInitGeneration) return;
      showErrorToast('Failed to load filter presets', error);
    }

    if (thisGen !== filterInitGeneration) return;
    set({ isLoading: false });
  },

  clearProject: () => {
    set({
      filters: { ...DEFAULT_FILTERS },
      currentProjectPath: null,
    });
  },

  // (Fix 4.34) Only revert if the current filters still match what this call
  // set — a newer call may have already replaced them.
  setFilters: (filters) => {
    const { currentProjectPath, filters: previousFilters } = get();
    const cloned = cloneFilters(filters);
    set({ filters: cloned });

    if (currentProjectPath) {
      saveFilters(currentProjectPath, cloned).catch((error) => {
        showErrorToast('Failed to save filters', error);
        // Only revert if no newer change has been applied since this call
        if (get().filters === cloned) {
          set({ filters: previousFilters });
        }
      });
    }
  },

  resetFilters: () => {
    const { currentProjectPath, filters: previousFilters } = get();
    const defaultFilters = cloneFilters(DEFAULT_FILTERS);
    set({ filters: defaultFilters });

    if (currentProjectPath) {
      saveFilters(currentProjectPath, defaultFilters).catch((error) => {
        showErrorToast('Failed to save filters', error);
        if (get().filters === defaultFilters) {
          set({ filters: previousFilters });
        }
      });
    }
  },

  // (Fix 4.33 + 4.35) Use createPreset() for proper deep-copy.
  // Revert on IPC failure to keep memory consistent with disk.
  savePreset: async (name) => {
    const { filters, presets } = get();

    const MAX_PRESETS = 50;
    if (presets.length >= MAX_PRESETS) {
      useToastStore.getState().addToast(`Maximum of ${MAX_PRESETS} presets reached.`, 'warning');
      return;
    }

    const newPreset = createPreset(name.slice(0, 100), filters);
    const previousPresets = presets;
    const newPresets = [...presets, newPreset];
    set({ presets: newPresets });

    try {
      await saveFilterPresets(newPresets);
    } catch (error) {
      showErrorToast('Failed to save filter presets', error);
      set({ presets: previousPresets });
    }
  },

  // (Fix 4.35) Deep-clone preset filters before applying to prevent shared-reference mutation
  loadPreset: (preset) => {
    const { currentProjectPath, filters: previousFilters } = get();
    const cloned = cloneFilters(preset.filters);
    set({ filters: cloned });

    if (currentProjectPath) {
      saveFilters(currentProjectPath, cloned).catch((error) => {
        showErrorToast('Failed to save preset filters', error);
        if (get().filters === cloned) {
          set({ filters: previousFilters });
        }
      });
    }
  },

  // (Fix 4.33) Revert presets on IPC failure
  deletePreset: async (id) => {
    const { presets: previousPresets } = get();
    const newPresets = previousPresets.filter((p) => p.id !== id);
    set({ presets: newPresets });

    try {
      await saveFilterPresets(newPresets);
    } catch (error) {
      showErrorToast('Failed to save filter presets', error);
      set({ presets: previousPresets });
    }
  },

  loadGlobalPresets: async () => {
    try {
      const presets = await loadFilterPresets();
      set({ presets });
    } catch (error) {
      showErrorToast('Failed to load filter presets', error);
    }
  },

}));
