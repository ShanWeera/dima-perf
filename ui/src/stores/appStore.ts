/**
 * DiMA Desktop - Main Application Store
 * 
 * Manages overall application state including current view and initialization.
 */

import { create } from 'zustand';
import { listRecentProjects, clearRecentProjects as clearRecentProjectsApi } from '@/lib/tauri';
import type { RecentProject } from '@/lib/types';

export type AppView = 
  | 'welcome'
  | 'projects'
  | 'wizard'
  | 'results'
  | 'settings'
  | 'about';

interface AppState {
  // Initialization
  isInitialized: boolean;
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

export const useAppStore = create<AppState>((set, get) => ({
  isInitialized: false,
  isLoading: false,
  error: null,
  currentView: 'welcome',
  recentProjects: [],
  sidebarCollapsed: false,

  initialize: async () => {
    if (get().isInitialized) return;
    
    set({ isLoading: true, error: null });
    
    try {
      // Load recent projects
      const projects = await listRecentProjects();
      
      set({ 
        isInitialized: true, 
        isLoading: false,
        recentProjects: projects,
        currentView: projects.length > 0 ? 'projects' : 'welcome',
      });
    } catch (error) {
      console.error('Failed to initialize app:', error);
      set({ 
        isInitialized: true, 
        isLoading: false, 
        error: String(error),
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
      console.error('Failed to refresh recent projects:', error);
    }
  },

  clearRecentProjects: async () => {
    try {
      await clearRecentProjectsApi();
      set({ recentProjects: [] });
    } catch (error) {
      console.error('Failed to clear recent projects:', error);
    }
  },

  toggleSidebar: () => {
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed }));
  },

  setSidebarCollapsed: (collapsed) => {
    set({ sidebarCollapsed: collapsed });
  },

  setError: (error) => {
    set({ error });
  },
}));
