/**
 * DiMA Desktop - Main Application Store
 * 
 * Manages overall application state including current view and initialization.
 */

import { create } from 'zustand';
import { listRecentProjects, clearRecentProjects as clearRecentProjectsApi } from '@/lib/tauri';
import type { RecentProject } from '@/lib/types';
import { showErrorToast, extractErrorMessage } from '@/lib/utils';
import { useToastStore } from './toastStore';

export type AppView = 
  | 'welcome'
  | 'projects'
  | 'wizard'
  | 'settings'
  | 'about';

interface AppState {
  // Initialization
  isInitialized: boolean;
  isInitializing: boolean;
  isLoading: boolean;
  error: string | null;

  // Current view
  currentView: AppView;
  
  // Recent projects
  recentProjects: RecentProject[];

  // Sidebar state
  sidebarCollapsed: boolean;

  // Actions
  initialize: () => Promise<void>;
  setCurrentView: (view: AppView) => void;
  refreshRecentProjects: () => Promise<void>;
  clearRecentProjects: () => Promise<void>;
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setError: (error: string | null) => void;
}

const SIDEBAR_STORAGE_KEY = 'dima-sidebar-collapsed';

function getPersistedSidebarState(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_STORAGE_KEY) === 'true';
  } catch {
    return false;
  }
}

export const useAppStore = create<AppState>((set, get) => ({
  isInitialized: false,
  isInitializing: false,
  isLoading: false,
  error: null,
  currentView: 'welcome',
  recentProjects: [],
  sidebarCollapsed: getPersistedSidebarState(),

  initialize: async () => {
    const state = get();
    // Guard: skip if already initialized or currently initializing (React Strict Mode)
    if (state.isInitialized || state.isInitializing) return;
    
    set({ isInitializing: true, isLoading: true, error: null });
    
    try {
      const projects = await listRecentProjects();
      
      set({ 
        isInitialized: true, 
        isInitializing: false,
        isLoading: false,
        recentProjects: projects,
        currentView: projects.length > 0 ? 'projects' : 'welcome',
      });
    } catch (error) {
      showErrorToast('Failed to initialize app', error);
      // On failure, allow retry by NOT setting isInitialized to true
      set({ 
        isInitialized: false, 
        isInitializing: false,
        isLoading: false, 
        error: extractErrorMessage(error) ?? 'Initialization failed unexpectedly',
        currentView: 'welcome',
      });
    }
  },

  setCurrentView: (view) => {
    set({ currentView: view });
  },

  refreshRecentProjects: async () => {
    try {
      const projects = await listRecentProjects();
      set({ recentProjects: projects });
    } catch (error) {
      showErrorToast('Failed to refresh recent projects', error);
    }
  },

  clearRecentProjects: async () => {
    // Delayed-commit pattern: optimistically clear UI, defer backend call,
    // and show an undo toast. If user clicks undo within 6s the clear is
    // cancelled. If the timer expires the backend is called. (Fix 9.4.3)
    const snapshot = [...get().recentProjects];
    if (snapshot.length === 0) return;

    set({ recentProjects: [] });

    let committed = false;
    const timer = setTimeout(async () => {
      committed = true;
      try {
        await clearRecentProjectsApi();
      } catch (error) {
        // Backend failed — restore from snapshot so data isn't lost
        set({ recentProjects: snapshot });
        showErrorToast('Failed to clear recent projects', error);
      }
    }, 6000);

    useToastStore.getState().addToast(
      'Recent projects cleared',
      'info',
      6000,
      {
        label: 'Undo',
        onClick: () => {
          if (committed) return;
          clearTimeout(timer);
          set({ recentProjects: snapshot });
        },
      },
    );
  },

  toggleSidebar: () => {
    set((state) => {
      const newVal = !state.sidebarCollapsed;
      try { localStorage.setItem(SIDEBAR_STORAGE_KEY, String(newVal)); } catch { /* noop */ }
      return { sidebarCollapsed: newVal };
    });
  },

  setSidebarCollapsed: (collapsed) => {
    try { localStorage.setItem(SIDEBAR_STORAGE_KEY, String(collapsed)); } catch { /* noop */ }
    set({ sidebarCollapsed: collapsed });
  },

  setError: (error) => {
    set({ error });
  },
}));
