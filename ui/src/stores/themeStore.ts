/**
 * DiMA Desktop - Theme Store
 * 
 * Manages application theme (light/dark/system).
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

type ThemeMode = 'light' | 'dark' | 'system';

interface ThemeState {
  mode: ThemeMode;
  effectiveTheme: 'light' | 'dark';
  
  setMode: (mode: ThemeMode) => void;
  updateEffectiveTheme: () => void;
}

function getSystemTheme(): 'light' | 'dark' {
  if (typeof window !== 'undefined') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return 'light';
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      mode: 'system',
      effectiveTheme: getSystemTheme(),

      setMode: (mode) => {
        set({ mode });
        get().updateEffectiveTheme();
      },

      updateEffectiveTheme: () => {
        const { mode } = get();
        const effective = mode === 'system' ? getSystemTheme() : mode;
        set({ effectiveTheme: effective });
      },
    }),
    {
      name: 'dima-theme',
      partialize: (state) => ({ mode: state.mode }),
    }
  )
);

// Listen for system theme changes
if (typeof window !== 'undefined') {
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    useThemeStore.getState().updateEffectiveTheme();
  });
}
