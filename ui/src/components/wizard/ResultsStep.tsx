/**
 * DiMA Desktop - Results Step
 * 
 * Dashboard view showing analysis results with interactive visualizations.
 */

import { useEffect, useState, useCallback, useRef, useMemo, lazy, Suspense } from 'react';
import { BarChart3, Download, RefreshCw, PanelRight, MapPin, Filter, MessageSquare, RotateCw, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { useAppStore } from '@/stores/appStore';
import { useFilterStore } from '@/stores/filterStore';
import { useShallow } from 'zustand/react/shallow';
import type { Layout } from 'react-grid-layout';
import { DEFAULT_LAYOUT } from '@/lib/dashboard-layout';
import { PanelToggle } from '@/components/dashboard/PanelToggle';
import { ExportDialog } from '@/components/export/ExportDialog';
import { FilterPanel } from '@/components/filters/FilterPanel';
import { AnnotationManager } from '@/components/annotations/AnnotationManager';
import { saveLayout, loadLayout, loadResults } from '@/lib/tauri';
import { applyFiltersToPositions, areFiltersDefault } from '@/lib/filters';
import { useToastStore } from '@/stores/toastStore';
import { showErrorToast } from '@/lib/utils';
import { AriaLive } from '@/components/ui/aria-live';
import { registerShortcuts, SHORTCUTS } from '@/lib/keyboard-shortcuts';

// Lazy-load the dashboard grid and its heavy chart dependencies (ECharts, react-grid-layout)
const DashboardGrid = lazy(() => import('@/components/dashboard/DashboardGrid').then(m => ({ default: m.DashboardGrid })));

export function ResultsStep() {
  const { 
    currentProject, 
    results, 
    closeProject,
    selectedPosition,
    selectPosition,
    annotations,
    addAnnotation,
    updateAnnotation,
    removeAnnotation,
    config,
    setWizardStep,
    setResults,
    layoutResetVersion,
  } = useProjectStore(useShallow((s) => ({
    currentProject: s.currentProject,
    results: s.results,
    closeProject: s.closeProject,
    selectedPosition: s.selectedPosition,
    selectPosition: s.selectPosition,
    annotations: s.annotations,
    addAnnotation: s.addAnnotation,
    updateAnnotation: s.updateAnnotation,
    removeAnnotation: s.removeAnnotation,
    config: s.config,
    setWizardStep: s.setWizardStep,
    setResults: s.setResults,
    layoutResetVersion: s.layoutResetVersion,
  })));
  const { setCurrentView } = useAppStore();
  const { filters, setFilters, presets, savePreset, loadPreset, deletePreset, initializeForProject, clearProject: clearFilters } = useFilterStore(useShallow((s) => ({
    filters: s.filters,
    setFilters: s.setFilters,
    initializeForProject: s.initializeForProject,
    clearProject: s.clearProject,
    presets: s.presets,
    savePreset: s.savePreset,
    loadPreset: s.loadPreset,
    deletePreset: s.deletePreset,
  })));
  const [isLoading, setIsLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [layout, setLayout] = useState<Layout[]>(DEFAULT_LAYOUT);
  const [hiddenPanels, setHiddenPanels] = useState<string[]>([]);
  const [showPanelToggle, setShowPanelToggle] = useState(false);
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [showFilterPanel, setShowFilterPanel] = useState(false);
  const [showAnnotations, setShowAnnotations] = useState(false);
  const [goToPosition, setGoToPosition] = useState('');
  // Chart export: data URL captured from an ECharts instance when user clicks
  // the per-panel "Export chart" button. Cleared when the export dialog closes.
  const [chartExportData, setChartExportData] = useState<{ dataUrl: string; chartType: string } | null>(null);
  const layoutLoadedRef = useRef(false);
  // Refs that always point to the latest layout/hiddenPanels, used by callbacks
  // that persist state to avoid stale closure captures during rapid interactions.
  const layoutRef = useRef(layout);
  layoutRef.current = layout;
  const hiddenPanelsRef = useRef(hiddenPanels);
  hiddenPanelsRef.current = hiddenPanels;
  const filterLoadedRef = useRef(false);
  const saveTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Computed filtered positions — hoisted before effects that depend on it (Fix 5.39).
  const filteredPositions = useMemo(() => {
    if (!results) return [];
    if (areFiltersDefault(filters)) return results.results;
    return applyFiltersToPositions(results.results, filters);
  }, [results, filters]);
  const filterActive = !areFiltersDefault(filters);

  // Load results from backend if not already in memory.
  // Captures the project path before the async call so stale results from
  // a previous project cannot overwrite the current project's store. (Fix 2.8)
  useEffect(() => {
    if (!results && currentProject?.hasResults) {
      const projectPath = currentProject.path;
      refreshResults(projectPath);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refreshResults is stable
  }, [results, currentProject]);

  // Initialize filter store for this project
  useEffect(() => {
    if (currentProject && !filterLoadedRef.current) {
      filterLoadedRef.current = true;
      initializeForProject(currentProject.path).catch((error) => {
        showErrorToast('Failed to initialize filters', error);
      });
    }
    
    return () => {
      if (filterLoadedRef.current) {
        clearFilters();
        filterLoadedRef.current = false;
      }
    };
  }, [currentProject, initializeForProject, clearFilters]);

  // Load layout from project. Reset + reload whenever the project path changes
  // to prevent layout from a previous project persisting into the new one. (Fix 2.5)
  // Also clear the debounce save timeout on project switch to prevent writing
  // the old project's layout to the new project's path. (Fix 2.6)
  useEffect(() => {
    layoutLoadedRef.current = false;

    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
    }

    if (!currentProject) return;

    const projectPath = currentProject.path;
    layoutLoadedRef.current = true;

    loadLayout(projectPath)
      .then((savedLayout) => {
        // Verify we're still on the same project before applying
        if (useProjectStore.getState().currentProject?.path !== projectPath) return;
        if (savedLayout) {
          const loaded = savedLayout.layout.map(item => ({
            i: item.i,
            x: item.x,
            y: item.y,
            w: item.w,
            h: item.h,
            minW: item.minW,
            minH: item.minH,
          }));
          // Merge in any panels from DEFAULT_LAYOUT that are missing from
          // the saved layout (e.g. newly added panels like feature-tracks).
          // Place them below the existing saved layout to prevent overlap. (Fix 4.36)
          const savedIds = new Set(loaded.map(item => item.i));
          const missing = DEFAULT_LAYOUT.filter(item => !savedIds.has(item.i));
          if (missing.length > 0) {
            const maxYH = loaded.reduce((max, item) => Math.max(max, item.y + item.h), 0);
            const repositioned = missing.map((item, idx) => ({
              ...item,
              y: maxYH + idx * item.h, // Stack vertically below existing items
            }));
            setLayout([...loaded, ...repositioned]);
          } else {
            setLayout(loaded);
          }
          setHiddenPanels(savedLayout.hidden_panels);
        } else {
          setLayout(DEFAULT_LAYOUT);
          setHiddenPanels([]);
        }
      })
      .catch((error) => {
        showErrorToast('Failed to load layout', error);
      });

    return () => {
      // On unmount or project change: if there's a pending debounced save,
      // flush it immediately to prevent silent layout data loss. (Fix 4.36)
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = null;
        // Fire-and-forget save with the latest state from refs
        if (projectPath && layoutLoadedRef.current) {
          saveLayout(projectPath, {
            layout: layoutRef.current,
            hidden_panels: hiddenPanelsRef.current,
          }).catch(() => {
            // Best-effort: log but don't surface on cleanup
          });
        }
      }
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- layoutResetVersion triggers reload from SettingsView reset
  }, [currentProject?.path, layoutResetVersion]);

  // Select first position by default when results load or selectedPosition
  // is cleared (e.g., after re-analysis via setResults clearing it). (Fix 2.12)
  // Auto-select first visible position when selection is cleared or filters change (Fix 5.39).
  useEffect(() => {
    if (results && selectedPosition === null) {
      const positions = filteredPositions.length > 0 ? filteredPositions : results.results;
      if (positions.length > 0) {
        selectPosition(positions[0].position);
      }
    }
  }, [results, selectedPosition, selectPosition, filteredPositions]);

  // Keyboard navigation for positions (arrow keys).
  // Disabled when any modal/dialog is open to prevent position changes underneath overlays.
  useEffect(() => {
    if (!results || results.results.length === 0) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Disable keyboard nav when modals or dialogs are open
      if (showExportDialog || showFilterPanel || showAnnotations || showPanelToggle) {
        return;
      }

      // Don't handle if focused on interactive elements
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
        return;
      }

      // Don't handle if inside a dialog/modal element
      if (target.closest('[role="dialog"]') || target.closest('[data-radix-portal]')) {
        return;
      }

      // Navigate within filtered positions when filters are active (Fix 5.39).
      // This prevents keyboard navigation from jumping to positions that are
      // hidden from the entropy chart.
      const positions = filteredPositions.length > 0 ? filteredPositions : results.results;
      const currentIdx = selectedPosition !== null 
        ? positions.findIndex((p) => p.position === selectedPosition)
        : -1;

      if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
        e.preventDefault();
        const newIdx = currentIdx <= 0 ? positions.length - 1 : currentIdx - 1;
        selectPosition(positions[newIdx].position);
      } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
        e.preventDefault();
        const newIdx = currentIdx >= positions.length - 1 ? 0 : currentIdx + 1;
        selectPosition(positions[newIdx].position);
      } else if (e.key === 'Home') {
        e.preventDefault();
        selectPosition(positions[0].position);
      } else if (e.key === 'End') {
        e.preventDefault();
        selectPosition(positions[positions.length - 1].position);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [results, selectedPosition, selectPosition, filteredPositions, showExportDialog, showFilterPanel, showAnnotations, showPanelToggle]);

  // Context-specific keyboard shortcuts: Mod+Shift+F (filters), Mod+Shift+E (export).
  // Separate from arrow-key nav because these should work even when panels are open. (Fix 9.3.3)
  useEffect(() => {
    if (!results) return;

    return registerShortcuts([
      {
        shortcut: SHORTCUTS.toggleFilters,
        handler: () => setShowFilterPanel((prev) => !prev),
      },
      {
        shortcut: SHORTCUTS.export,
        handler: () => setShowExportDialog(true),
      },
    ]);
  }, [results]);

  // Load results from backend with stale-project protection. (Fix 2.8)
  // Accepts the project path captured at call time so async completion can
  // verify the project hasn't changed during the load.
  const refreshResults = async (forProjectPath?: string) => {
    const projectPath = forProjectPath ?? currentProject?.path;
    if (!projectPath) return;
    
    setIsLoading(true);
    setLoadError(null);
    try {
      const parsed = await loadResults(projectPath);

      // Verify project hasn't switched during the async load
      const currentPath = useProjectStore.getState().currentProject?.path;
      if (currentPath !== projectPath) {
        return; // Discard stale results
      }
      setResults(parsed);
    } catch (error) {
      showErrorToast('Failed to load results', error);
      setLoadError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoading(false);
    }
  };

  // Persist layout with debounce. (Fix 2.6)
  // Captures the project path at call time so the debounced timeout
  // cannot write to a different project if a switch occurs within 500ms.
  const persistLayout = useCallback((newLayout: Layout[], newHiddenPanels: string[]) => {
    if (!currentProject) return;
    const capturedPath = currentProject.path;

    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
    }

    saveTimeoutRef.current = setTimeout(async () => {
      // Re-verify project path is still current before writing
      const currentPath = useProjectStore.getState().currentProject?.path;
      if (currentPath !== capturedPath) return;

      try {
        await saveLayout(capturedPath, {
          layout: newLayout.map(item => ({
            i: item.i,
            x: item.x,
            y: item.y,
            w: item.w,
            h: item.h,
            minW: item.minW,
            minH: item.minH,
          })),
          hidden_panels: newHiddenPanels,
        });
      } catch (error) {
        showErrorToast('Failed to save layout', error);
      }
    }, 500);
  }, [currentProject]);

  const handleClose = () => {
    closeProject();
    setCurrentView('projects');
  };

  const handleLayoutChange = useCallback((newLayout: Layout[]) => {
    setLayout(newLayout);
    // Use hiddenPanelsRef to always persist the current hidden state,
    // not a stale closure capture from when this callback was created.
    persistLayout(newLayout, hiddenPanelsRef.current);
  }, [persistLayout]);

  const handleTogglePanel = useCallback((panelId: string) => {
    setHiddenPanels((prev) => {
      const newHiddenPanels = prev.includes(panelId)
        ? prev.filter((id) => id !== panelId)
        : [...prev, panelId];
      // Use layoutRef to always persist the current layout,
      // not a stale closure capture from when this callback was created.
      persistLayout(layoutRef.current, newHiddenPanels);
      return newHiddenPanels;
    });
  }, [persistLayout]);

  const handleGoToPosition = () => {
    const pos = parseInt(goToPosition, 10);
    if (isNaN(pos) || !goToPosition.trim()) return;
    if (!results || results.results.length === 0) return;
    
    const exists = results.results.some((p) => p.position === pos);
    if (exists) {
      selectPosition(pos);
      setGoToPosition('');
    } else {
      const min = results.results[0].position;
      const max = results.results[results.results.length - 1].position;
      useToastStore.getState().addToast(
        `Position ${pos} not found. Valid range: ${min}–${max}`,
        'warning',
      );
    }
  };

  const data = results;

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <RefreshCw className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <div className="rounded-full bg-destructive/10 p-4">
          <BarChart3 className="h-12 w-12 text-destructive" />
        </div>
        <h2 className="text-lg font-semibold">Failed to Load Results</h2>
        <p className="max-w-md text-center text-sm text-muted-foreground">{loadError}</p>
        <div className="flex gap-3">
          <Button variant="outline" onClick={handleClose}>
            Close Project
          </Button>
          <Button onClick={() => refreshResults()} className="gap-2">
            <RotateCw className="h-4 w-4" />
            Retry
          </Button>
        </div>
      </div>
    );
  }

  if (!data) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <BarChart3 className="h-16 w-16 text-muted-foreground/50" />
        <p className="text-muted-foreground">No results available</p>
        <Button variant="outline" onClick={handleClose}>
          Close Project
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Main Content */}
      <div className="flex flex-1 flex-col overflow-hidden">
        {/* Header */}
        <div className="border-b px-6 py-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-4 min-w-0">
              <div>
                <h1 className="text-lg font-semibold truncate min-w-0">{currentProject?.name}</h1>
                <p className="text-sm text-muted-foreground">
                  {data.sequence_count.toLocaleString()} sequences • {data.results.length} positions
                  {filterActive && (
                    <span className="ml-2 text-primary font-medium">
                      ({filteredPositions.length} shown)
                    </span>
                  )}
                </p>
              </div>
              {selectedPosition !== null && (
                <div className="flex items-center gap-2 rounded-full bg-primary/10 px-3 py-1 text-sm">
                  <MapPin className="h-4 w-4 text-primary" />
                  <span>Position {selectedPosition}</span>
                </div>
              )}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {/* Go to Position */}
              <div className="flex items-center gap-2">
                <label htmlFor="goto-position-input" className="sr-only">Go to position</label>
                <input
                  id="goto-position-input"
                  type="number"
                  placeholder="Go to position..."
                  value={goToPosition}
                  onChange={(e) => setGoToPosition(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleGoToPosition()}
                  className="w-32 rounded-md border bg-background px-2 py-1 text-sm"
                  aria-label="Go to position number"
                />
              </div>
              <Button
                variant={showFilterPanel ? 'default' : 'outline'}
                size="sm"
                onClick={() => {
                  const next = !showFilterPanel;
                  setShowFilterPanel(next);
                  if (next) { setShowAnnotations(false); setShowPanelToggle(false); }
                }}
                className="gap-2"
                aria-expanded={showFilterPanel}
                aria-controls="filter-panel"
              >
                <Filter className="h-4 w-4" />
                Filters
                {filterActive && (
                  <span className="h-2 w-2 rounded-full bg-primary" aria-label="Filters active" />
                )}
              </Button>
              <Button
                variant={showAnnotations ? 'default' : 'outline'}
                size="sm"
                onClick={() => {
                  const next = !showAnnotations;
                  setShowAnnotations(next);
                  if (next) { setShowFilterPanel(false); setShowPanelToggle(false); }
                }}
                className="gap-2"
                aria-expanded={showAnnotations}
                aria-controls="annotations-panel"
              >
                <MessageSquare className="h-4 w-4" />
                Notes
              </Button>
              <Button
                variant={showPanelToggle ? 'default' : 'outline'}
                size="sm"
                onClick={() => {
                  const next = !showPanelToggle;
                  setShowPanelToggle(next);
                  if (next) { setShowFilterPanel(false); setShowAnnotations(false); }
                }}
                className="gap-2"
                aria-expanded={showPanelToggle}
              >
                <PanelRight className="h-4 w-4" />
                Panels
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setWizardStep('configure')}
                className="gap-2"
              >
                <RotateCw className="h-4 w-4" />
                Re-analyze
              </Button>
              <Button variant="outline" size="sm" onClick={handleClose}>
                Close
              </Button>
              <Button size="sm" className="gap-2" onClick={() => setShowExportDialog(true)}>
                <Download className="h-4 w-4" />
                Export
              </Button>
            </div>
          </div>
        </div>

        {/* Dashboard */}
        <div className="flex-1 overflow-auto">
          <Suspense fallback={
            <div className="flex h-full items-center justify-center">
              <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
            </div>
          }>
            <DashboardGrid
              results={data}
              filteredPositions={filteredPositions}
              filterActive={filterActive}
              selectedPosition={selectedPosition}
              onSelectPosition={selectPosition}
              layout={layout}
              onLayoutChange={handleLayoutChange}
              hiddenPanels={hiddenPanels}
              alphabet={config?.alphabet || 'protein'}
              annotations={annotations}
              onExportChart={(dataUrl, chartType) => {
                setChartExportData({ dataUrl, chartType });
                setShowExportDialog(true);
              }}
            />
          </Suspense>
        </div>
      </div>

      {/* Panel Toggle Sidebar */}
      {showPanelToggle && (
        <PanelToggle
          hiddenPanels={hiddenPanels}
          onTogglePanel={handleTogglePanel}
          onResetLayout={() => {
            setLayout(DEFAULT_LAYOUT);
            setHiddenPanels([]);
            persistLayout(DEFAULT_LAYOUT, []);
          }}
        />
      )}

      {/* Filter Panel Sidebar */}
      {showFilterPanel && (
        <div id="filter-panel" className="w-80 border-l bg-background overflow-auto">
          <FilterPanel
            positions={data.results}
            filters={filters}
            onFiltersChange={setFilters}
            presets={presets}
            onSavePreset={savePreset}
            onLoadPreset={loadPreset}
            onDeletePreset={deletePreset}
          />
        </div>
      )}

      {/* Annotation Manager Sidebar */}
      {showAnnotations && (
        <div id="annotations-panel" className="w-80 border-l bg-background overflow-auto">
          <AnnotationManager
            annotations={annotations}
            selectedPosition={selectedPosition}
            onAddAnnotation={addAnnotation}
            onUpdateAnnotation={updateAnnotation}
            onRemoveAnnotation={removeAnnotation}
            onGoToPosition={selectPosition}
          />
        </div>
      )}

      {/* Export Dialog */}
      {showExportDialog && currentProject && (
        <ExportDialog
          projectPath={currentProject.path}
          projectName={currentProject.name}
          chartDataUrl={chartExportData?.dataUrl}
          chartType={chartExportData?.chartType}
          onClose={() => {
            setShowExportDialog(false);
            setChartExportData(null);
          }}
        />
      )}

      {/* Screen reader announcements for dynamic state changes */}
      <AriaLive
        message={
          selectedPosition !== null
            ? `Position ${selectedPosition} selected. ${filteredPositions.length} positions shown.`
            : `${filteredPositions.length} positions shown.`
        }
      />
    </div>
  );
}
