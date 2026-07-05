/**
 * DiMA Desktop - Settings Store
 *
 * Manages application settings with persistence via Tauri commands.
 * Includes unified theme management (replaces the former standalone themeStore)
 * to ensure backend-persisted settings and runtime theme state are always in sync.
 */

import { create } from 'zustand';
import type { AppSettings } from '@/lib/types';
import { DEFAULT_APP_SETTINGS } from '@/lib/types';
import { getSettings, updateSettings as saveSettingsToBackend } from '@/lib/tauri';
import { showErrorToast } from '@/lib/utils';

// Serializes backend persistence calls to prevent out-of-order overwrites (Fix 5.102).
// Without this, two rapid updateSetting calls can race: the first call's save
// may land AFTER the second's, overwriting newer changes on disk.
let saveQueue: Promise<void> = Promise.resolve();
function enqueueSettingsSave(settings: AppSettings): Promise<void> {
  const saveTask = saveQueue.then(() => saveSettingsToBackend(settings));
  // Chain the next save after this one (regardless of success/failure)
  saveQueue = saveTask.catch(() => {});
  return saveTask;
}

type ThemeMode = 'light' | 'dark' | 'system';

// Persist the effective theme (light/dark) to localStorage so it can be read
// synchronously at startup, preventing a flash of wrong-theme content (FOUC)
// while the async Tauri settings are loading.
const THEME_STORAGE_KEY = 'dima-effective-theme';

function persistEffectiveTheme(theme: 'light' | 'dark'): void {
  try { localStorage.setItem(THEME_STORAGE_KEY, theme); } catch { /* noop */ }
}

function getCachedOrSystemTheme(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light';
  try {
    const cached = localStorage.getItem(THEME_STORAGE_KEY);
    if (cached === 'light' || cached === 'dark') return cached;
  } catch { /* noop */ }
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function getSystemTheme(): 'light' | 'dark' {
  if (typeof window !== 'undefined') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return 'light';
}

const DEFAULT_SETTINGS = DEFAULT_APP_SETTINGS;

interface SettingsState {
  settings: AppSettings;
  isLoading: boolean;
  isInitialized: boolean;

  // Derived theme state — computed from settings.theme + system preference
  effectiveTheme: 'light' | 'dark';

  initialize: () => Promise<void>;
  updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => Promise<void>;
  setSettings: (settings: AppSettings) => Promise<void>;
  resetToDefaults: () => Promise<void>;

  // Theme-specific convenience action (delegates to updateSetting)
  setThemeMode: (mode: ThemeMode) => Promise<void>;
  // Re-evaluate effectiveTheme from current mode + system pref
  updateEffectiveTheme: () => void;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: { ...DEFAULT_SETTINGS },
  isLoading: false,
  isInitialized: false,
  effectiveTheme: getCachedOrSystemTheme(),

  initialize: async () => {
    if (get().isInitialized) return;

    set({ isLoading: true });
    try {
      const settings = await getSettings();
      const merged = { ...DEFAULT_SETTINGS, ...settings };
      const effective = merged.theme === 'system' ? getSystemTheme() : merged.theme;
      persistEffectiveTheme(effective);
      set({
        settings: merged,
        effectiveTheme: effective,
        isInitialized: true,
      });
    } catch (error) {
      showErrorToast('Failed to load settings', error);
      set({ isInitialized: true });
    } finally {
      set({ isLoading: false });
    }
  },

  updateSetting: async (key, value) => {
    // Snapshot the previous value for this specific key (not the whole object)
    // so a failed save only reverts our mutation, not concurrent ones.
    const previousValue = get().settings[key];
    set((state) => ({
      settings: { ...state.settings, [key]: value },
    }));

    if (key === 'theme') {
      const mode = value as ThemeMode;
      const newEffective = mode === 'system' ? getSystemTheme() : mode;
      persistEffectiveTheme(newEffective);
      set({ effectiveTheme: newEffective });
    }

    try {
      // Serialize saves via queue to prevent out-of-order disk writes (Fix 5.102).
      // Read current state INSIDE the queue callback to always save the latest snapshot.
      await enqueueSettingsSave(get().settings);
    } catch (error) {
      showErrorToast('Failed to save settings', error);
      // CAS-style revert: only roll back if our optimistic write is still current
      // (another successful update may have overwritten our key already)
      set((state) => {
        if (state.settings[key] === value) {
          return { settings: { ...state.settings, [key]: previousValue } };
        }
        return state;
      });
      if (key === 'theme' && get().settings.theme !== value) {
        const currentMode = get().settings.theme;
        set({ effectiveTheme: currentMode === 'system' ? getSystemTheme() : currentMode });
      }
      throw error;
    }
  },

  setSettings: async (newSettings) => {
    const oldSettings = get().settings;
    const effective = newSettings.theme === 'system' ? getSystemTheme() : newSettings.theme;
    set({ settings: newSettings, effectiveTheme: effective });

    try {
      await enqueueSettingsSave(newSettings);
    } catch (error) {
      showErrorToast('Failed to save settings', error);
      const oldEffective = oldSettings.theme === 'system' ? getSystemTheme() : oldSettings.theme;
      set({ settings: oldSettings, effectiveTheme: oldEffective });
      throw error;
    }
  },

  resetToDefaults: async () => {
    const oldSettings = get().settings;
    const effective = DEFAULT_SETTINGS.theme === 'system' ? getSystemTheme() : DEFAULT_SETTINGS.theme;
    set({ settings: { ...DEFAULT_SETTINGS }, effectiveTheme: effective });

    try {
      await enqueueSettingsSave(DEFAULT_SETTINGS);
    } catch (error) {
      showErrorToast('Failed to save default settings', error);
      const oldEffective = oldSettings.theme === 'system' ? getSystemTheme() : oldSettings.theme;
      set({ settings: oldSettings, effectiveTheme: oldEffective });
      throw error;
    }
  },

  setThemeMode: async (mode) => {
    await get().updateSetting('theme', mode);
  },

  updateEffectiveTheme: () => {
    const { settings } = get();
    const effective = settings.theme === 'system' ? getSystemTheme() : settings.theme;
    persistEffectiveTheme(effective);
    set({ effectiveTheme: effective });
  },
}));

// Listen for OS-level theme changes so 'system' mode stays in sync
if (typeof window !== 'undefined') {
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    useSettingsStore.getState().updateEffectiveTheme();
  });
}
