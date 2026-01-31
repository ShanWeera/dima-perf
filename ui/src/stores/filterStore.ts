/**
 * DiMA Desktop - Filter Store
 * 
 * Manages search filters and filter presets with persistence.
 */

import { create } from 'zustand';
import type { SearchFilters, FilterPreset, Position } from '@/lib/types';
import { 
  saveFilters, 
  loadFilters, 
  saveFilterPresets, 
  loadFilterPresets 
} from '@/lib/tauri';
import { 
  DEFAULT_FILTERS, 
  applyFiltersToPositions 
} from '@/lib/filters';

interface FilterState {
  // Current filters
  filters: SearchFilters;
  
  // Current project path (for per-project persistence)
  currentProjectPath: string | null;
  
  // Global presets
  presets: FilterPreset[];
  
  // Loading state
  isLoading: boolean;

  // Actions - Initialize
  initializeForProject: (projectPath: string) => Promise<void>;
  clearProject: () => void;

  // Actions - Filters
  setFilters: (filters: SearchFilters) => void;
  resetFilters: () => void;
  
  // Preset actions
  savePreset: (name: string) => Promise<void>;
  loadPreset: (preset: FilterPreset) => void;
  deletePreset: (id: string) => Promise<void>;
  loadGlobalPresets: () => Promise<void>;

  // Apply filters to positions
  applyFilters: (positions: Position[]) => Position[];
}

export const useFilterStore = create<FilterState>((set, get) => ({
  filters: { ...DEFAULT_FILTERS },
  currentProjectPath: null,
  presets: [],
  isLoading: false,

  initializeForProject: async (projectPath) => {
    set({ isLoading: true, currentProjectPath: projectPath });
    
    try {
      // Load project-specific filters
      const savedFilters = await loadFilters(projectPath);
      if (savedFilters) {
        set({ filters: savedFilters });
      } else {
        set({ filters: { ...DEFAULT_FILTERS } });
      }
      
      // Load global presets
      const presets = await loadFilterPresets();
      set({ presets });
    } catch (error) {
      console.error('Failed to load filters:', error);
      set({ filters: { ...DEFAULT_FILTERS } });
    } finally {
      set({ isLoading: false });
    }
  },

  clearProject: () => {
    set({
      filters: { ...DEFAULT_FILTERS },
      currentProjectPath: null,
    });
  },

  setFilters: (filters) => {
    const { currentProjectPath } = get();
    set({ filters });
    
    // Persist to project if we have a project path
    if (currentProjectPath) {
      saveFilters(currentProjectPath, filters).catch((error) => {
        console.error('Failed to save filters:', error);
      });
    }
  },

  resetFilters: () => {
    const { currentProjectPath } = get();
    const defaultFilters = { ...DEFAULT_FILTERS };
    set({ filters: defaultFilters });
    
    // Persist to project
    if (currentProjectPath) {
      saveFilters(currentProjectPath, defaultFilters).catch((error) => {
        console.error('Failed to save filters:', error);
      });
    }
  },

  savePreset: async (name) => {
    const { filters, presets } = get();
    const newPreset: FilterPreset = {
      id: crypto.randomUUID(),
      name,
      filters: { ...filters },
    };
    const newPresets = [...presets, newPreset];
    set({ presets: newPresets });
    
    // Persist global presets
    try {
      await saveFilterPresets(newPresets);
    } catch (error) {
      console.error('Failed to save filter presets:', error);
    }
  },

  loadPreset: (preset) => {
    const { currentProjectPath } = get();
    set({ filters: { ...preset.filters } });
    
    // Persist to project
    if (currentProjectPath) {
      saveFilters(currentProjectPath, preset.filters).catch((error) => {
        console.error('Failed to save filters:', error);
      });
    }
  },

  deletePreset: async (id) => {
    const { presets } = get();
    const newPresets = presets.filter((p) => p.id !== id);
    set({ presets: newPresets });
    
    // Persist global presets
    try {
      await saveFilterPresets(newPresets);
    } catch (error) {
      console.error('Failed to save filter presets:', error);
    }
  },

  loadGlobalPresets: async () => {
    try {
      const presets = await loadFilterPresets();
      set({ presets });
    } catch (error) {
      console.error('Failed to load filter presets:', error);
    }
  },

  applyFilters: (positions) => {
    const { filters } = get();
    return applyFiltersToPositions(positions, filters);
  },
}));
