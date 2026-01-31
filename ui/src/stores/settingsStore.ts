/**
 * DiMA Desktop - Settings Store
 * 
 * Manages application settings with persistence via Tauri commands.
 */

import { create } from 'zustand';
import type { AppSettings } from '@/lib/types';
import { getSettings, updateSettings as saveSettingsToBackend } from '@/lib/tauri';

const DEFAULT_SETTINGS: AppSettings = {
  theme: 'system',
  decimalPrecision: 4,
  defaultOutputDirectory: null,
  defaultChartDpi: 72,
  defaultKmerLength: 9,
  defaultSupportThreshold: 30,
  defaultValidationMode: 'strict',
  lastUsedConfig: null,
};

interface SettingsState {
  settings: AppSettings;
  isLoading: boolean;
  isInitialized: boolean;
  
  // Initialize - load settings from backend
  initialize: () => Promise<void>;
  
  // Update a single setting (auto-saves)
  updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => Promise<void>;
  
  // Set all settings at once
  setSettings: (settings: AppSettings) => Promise<void>;
  
  // Reset to defaults
  resetToDefaults: () => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: { ...DEFAULT_SETTINGS },
  isLoading: false,
  isInitialized: false,

  initialize: async () => {
    if (get().isInitialized) return;
    
    set({ isLoading: true });
    try {
      const settings = await getSettings();
      set({ 
        settings: { ...DEFAULT_SETTINGS, ...settings },
        isInitialized: true,
      });
    } catch (error) {
      console.error('Failed to load settings:', error);
      set({ isInitialized: true });
    } finally {
      set({ isLoading: false });
    }
  },

  updateSetting: async (key, value) => {
    const { settings } = get();
    const newSettings = { ...settings, [key]: value };
    set({ settings: newSettings });
    
    // Auto-save to backend
    try {
      await saveSettingsToBackend(newSettings);
    } catch (error) {
      console.error('Failed to save settings:', error);
      // Revert on error
      set({ settings });
      throw error;
    }
  },

  setSettings: async (newSettings) => {
    const oldSettings = get().settings;
    set({ settings: newSettings });
    
    try {
      await saveSettingsToBackend(newSettings);
    } catch (error) {
      console.error('Failed to save settings:', error);
      // Revert on error
      set({ settings: oldSettings });
      throw error;
    }
  },

  resetToDefaults: async () => {
    const oldSettings = get().settings;
    set({ settings: { ...DEFAULT_SETTINGS } });
    
    try {
      await saveSettingsToBackend(DEFAULT_SETTINGS);
    } catch (error) {
      console.error('Failed to save default settings:', error);
      // Revert on error
      set({ settings: oldSettings });
      throw error;
    }
  },
}));
