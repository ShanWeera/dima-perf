/**
 * DiMA Desktop - Main Application Component
 * 
 * Root component that manages the overall application layout and navigation.
 * Uses a sidebar-based navigation approach with no traditional menu bar.
 */

import { useEffect, useCallback, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { confirm } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from './stores/appStore';
import { useThemeStore } from './stores/themeStore';
import { useSettingsStore } from './stores/settingsStore';
import { useProjectStore } from './stores/projectStore';
import { Sidebar } from './components/layout/Sidebar';
import { MainContent } from './components/layout/MainContent';
import { ImportDimaDialog } from './components/dialogs/ImportDimaDialog';

function App() {
  const { initialize, isInitialized } = useAppStore();
  const { effectiveTheme } = useThemeStore();
  const { initialize: initializeSettings } = useSettingsStore();
  const { isAnalyzing } = useProjectStore();
  const [pendingDimaFile, setPendingDimaFile] = useState<string | null>(null);

  // Initialize app and settings on mount
  useEffect(() => {
    initialize();
    initializeSettings();
  }, [initialize, initializeSettings]);

  // Listen for file open events (when app is opened with a .dima file)
  useEffect(() => {
    const setupFileListener = async () => {
      // Listen for file drop events
      const unlistenFileDrop = await listen<{ paths: string[] }>('tauri://file-drop', (event) => {
        const dimaFiles = event.payload.paths.filter(p => p.endsWith('.dima'));
        if (dimaFiles.length > 0) {
          setPendingDimaFile(dimaFiles[0]);
        }
      });

      // Listen for deep link / file association open
      const unlistenFileOpen = await listen<string>('file-open', (event) => {
        if (event.payload.endsWith('.dima')) {
          setPendingDimaFile(event.payload);
        }
      });

      return () => {
        unlistenFileDrop();
        unlistenFileOpen();
      };
    };

    let cleanup: (() => void) | undefined;
    setupFileListener().then(fn => { cleanup = fn; });

    return () => {
      if (cleanup) cleanup();
    };
  }, []);

  // Handle window close with confirmation if analysis is in progress
  const handleCloseRequested = useCallback(async () => {
    if (isAnalyzing) {
      const shouldClose = await confirm(
        'An analysis is currently in progress. Are you sure you want to close? All progress will be lost.',
        { title: 'Close DiMA Desktop?', kind: 'warning' }
      );
      return shouldClose;
    }
    return true;
  }, [isAnalyzing]);

  // Set up window close listener
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupCloseListener = async () => {
      const appWindow = getCurrentWindow();
      unlisten = await appWindow.onCloseRequested(async (event) => {
        const canClose = await handleCloseRequested();
        if (!canClose) {
          event.preventDefault();
        }
      });
    };

    setupCloseListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [handleCloseRequested]);

  // Apply theme class to document
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(effectiveTheme);
  }, [effectiveTheme]);

  if (!isInitialized) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-sm text-muted-foreground">Loading DiMA Desktop...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <Sidebar />
      <MainContent />
      
      {/* Import .dima file dialog */}
      {pendingDimaFile && (
        <ImportDimaDialog
          filePath={pendingDimaFile}
          onClose={() => setPendingDimaFile(null)}
        />
      )}
    </div>
  );
}

export default App;
