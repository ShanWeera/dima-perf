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
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from './stores/appStore';
import { useSettingsStore } from './stores/settingsStore';
import { useProjectStore, flushPendingAnnotationSave } from './stores/projectStore';
import { takePendingOpenPaths } from './lib/tauri';
import { TooltipProvider } from './components/ui/tooltip';
import { Sidebar } from './components/layout/Sidebar';
import { MainContent } from './components/layout/MainContent';
import { ImportDimaDialog } from './components/dialogs/ImportDimaDialog';
import { KeyboardShortcutsDialog } from './components/dialogs/KeyboardShortcutsDialog';
import { ErrorBoundary } from './components/ErrorBoundary';
import { ToastContainer } from './components/ToastContainer';
import { registerShortcuts, SHORTCUTS } from './lib/keyboard-shortcuts';

function App() {
  const { initialize, isInitialized, appError } = useAppStore(useShallow((s) => ({
    initialize: s.initialize,
    isInitialized: s.isInitialized,
    appError: s.error,
  })));
  const { effectiveTheme, initializeSettings, isSettingsInitialized } = useSettingsStore(useShallow((s) => ({
    effectiveTheme: s.effectiveTheme,
    initializeSettings: s.initialize,
    isSettingsInitialized: s.isInitialized,
  })));
  const isAnalyzing = useProjectStore((s) => s.isAnalyzing);
  const [pendingDimaFile, setPendingDimaFile] = useState<string | null>(null);
  const [showShortcutsDialog, setShowShortcutsDialog] = useState(false);
  const setCurrentView = useAppStore((s) => s.setCurrentView);

  // Initialize app and settings on mount
  useEffect(() => {
    initialize();
    initializeSettings();
  }, [initialize, initializeSettings]);

  // Listen for file open events (when app is opened with a .dima file).
  // Uses an abort flag to handle the race where the component unmounts before
  // the async listener registration resolves — prevents leaked listeners. (Fix 5.48)
  // Also pulls any pending paths queued during cold-start (Fix 4.42).
  useEffect(() => {
    let aborted = false;
    let cleanup: (() => void) | null = null;

    const setupFileListener = async () => {
      // Pull-based cold-start recovery: drain paths queued before the listener
      // was ready. This eliminates the 500ms race condition entirely. (Fix 4.42)
      try {
        const pendingPaths = await takePendingOpenPaths();
        if (!aborted && pendingPaths.length > 0) {
          const firstDima = pendingPaths.find(p => p.toLowerCase().endsWith('.dima'));
          if (firstDima) {
            setPendingDimaFile(firstDima);
          }
        }
      } catch {
        // Non-critical — app still works, just misses the cold-start file
      }

      const unlistenDragDrop = await listen<{ paths: string[]; position: { x: number; y: number } }>(
        'tauri://drag-drop',
        (event) => {
          const paths = event.payload.paths || [];
          const dimaFiles = paths.filter(p =>
            p.toLowerCase().endsWith('.dima')
          );
          if (dimaFiles.length > 0) {
            setPendingDimaFile(dimaFiles[0]);
          }
        }
      );

      // Runtime file-open events (second instance, or warm start associations)
      const unlistenFileOpen = await listen<{ path: string }>('dima://file-open', (event) => {
        const filePath = event.payload?.path;
        if (typeof filePath === 'string' && filePath.toLowerCase().endsWith('.dima')) {
          setPendingDimaFile(filePath);
        }
      });

      if (aborted) {
        unlistenDragDrop();
        unlistenFileOpen();
      } else {
        cleanup = () => { unlistenDragDrop(); unlistenFileOpen(); };
      }
    };

    setupFileListener();

    return () => {
      aborted = true;
      cleanup?.();
    };
  }, []);

  // Handle window close: flush pending state and confirm if analysis running
  const handleCloseRequested = useCallback(async () => {
    if (isAnalyzing) {
      const shouldClose = await confirm(
        'An analysis is currently in progress. Are you sure you want to close? All progress will be lost.',
        { title: 'Close DiMA Desktop?', kind: 'warning' }
      );
      if (!shouldClose) return false;
    }
    // Flush any debounced saves before the window closes
    flushPendingAnnotationSave();
    return true;
  }, [isAnalyzing]);

  // Set up window close listener with abort flag to prevent leaked listener
  // if the effect re-runs before the async registration resolves. (Fix 5.48)
  useEffect(() => {
    let aborted = false;
    let unlisten: (() => void) | null = null;

    const setupCloseListener = async () => {
      const appWindow = getCurrentWindow();
      const fn = await appWindow.onCloseRequested(async (event) => {
        const canClose = await handleCloseRequested();
        if (!canClose) {
          event.preventDefault();
        }
      });
      if (aborted) {
        fn();
      } else {
        unlisten = fn;
      }
    };

    setupCloseListener();

    return () => {
      aborted = true;
      unlisten?.();
    };
  }, [handleCloseRequested]);

  // Apply theme class to document
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(effectiveTheme);
  }, [effectiveTheme]);

  // Global keyboard shortcuts (Fix 9.3.3)
  useEffect(() => {
    return registerShortcuts([
      {
        shortcut: SHORTCUTS.settings,
        handler: () => setCurrentView('settings'),
      },
      {
        shortcut: SHORTCUTS.newAnalysis,
        handler: () => setCurrentView('projects'),
      },
      {
        shortcut: SHORTCUTS.help,
        handler: () => setShowShortcutsDialog((prev) => !prev),
      },
    ]);
  }, [setCurrentView]);

  if (!isInitialized || !isSettingsInitialized) {
    // Show error with retry option if initialization failed, rather than
    // trapping the user in an infinite loading spinner. (Fix 5.47)
    if (appError) {
      return (
        <div className="flex h-screen items-center justify-center bg-background">
          <div className="flex flex-col items-center gap-4 max-w-md text-center px-6">
            <div className="text-destructive text-lg font-semibold">Initialization Failed</div>
            <p className="text-sm text-muted-foreground">{appError}</p>
            <button
              onClick={() => { initialize(); initializeSettings(); }}
              className="px-4 py-2 rounded-md bg-primary text-primary-foreground text-sm hover:bg-primary/90 transition-colors"
            >
              Retry
            </button>
          </div>
        </div>
      );
    }
    return (
      <div className="flex h-screen items-center justify-center bg-background" role="status" aria-live="polite" aria-busy="true">
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-sm text-muted-foreground">Loading DiMA Desktop...</p>
        </div>
      </div>
    );
  }

  return (
    <ErrorBoundary label="DiMA Desktop">
      <TooltipProvider delayDuration={300} skipDelayDuration={150}>
      <div className="flex h-screen flex-col overflow-hidden bg-background">
        {/* App-level initialization error banner (Fix 4.3) */}
        {appError && (
          <div className="flex items-center gap-2 border-b bg-destructive/10 px-4 py-1.5 text-xs text-destructive" role="alert">
            <span>Initialization error: {appError}</span>
          </div>
        )}
        {/* Global analysis indicator — visible even when navigating to Settings/About */}
        {isAnalyzing && (
          <div className="flex items-center gap-2 border-b bg-primary/5 px-4 py-1.5 text-xs text-primary" role="status" aria-live="polite">
            <div className="h-2 w-2 animate-pulse rounded-full bg-primary" />
            <span>Analysis in progress...</span>
          </div>
        )}
        <div className="flex flex-1 overflow-hidden">
          <Sidebar />
          <main className="min-w-0 flex-1">
            <MainContent />
          </main>
        </div>
        
        {/* Import .dima file dialog */}
        {pendingDimaFile && (
          <ImportDimaDialog
            filePath={pendingDimaFile}
            onClose={() => setPendingDimaFile(null)}
          />
        )}

        {/* Keyboard shortcuts reference dialog (Fix 9.3.3) */}
        <KeyboardShortcutsDialog
          open={showShortcutsDialog}
          onOpenChange={setShowShortcutsDialog}
        />
      </div>
      <ErrorBoundary label="ToastContainer" compact>
        <ToastContainer />
      </ErrorBoundary>
      </TooltipProvider>
    </ErrorBoundary>
  );
}

export default App;
