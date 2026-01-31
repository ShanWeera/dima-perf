/**
 * DiMA Desktop - Results Step
 * 
 * Dashboard view showing analysis results with interactive visualizations.
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import { BarChart3, Download, RefreshCw, PanelRight, MapPin, Filter, MessageSquare } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { useAppStore } from '@/stores/appStore';
import { useFilterStore } from '@/stores/filterStore';
import type { AnalysisResult } from '@/lib/types';
import type { Layout } from 'react-grid-layout';
import { DashboardGrid, DEFAULT_LAYOUT } from '@/components/dashboard/DashboardGrid';
import { PanelToggle } from '@/components/dashboard/PanelToggle';
import { ExportDialog } from '@/components/export/ExportDialog';
import { FilterPanel } from '@/components/filters/FilterPanel';
import { AnnotationManager } from '@/components/annotations/AnnotationManager';
import { saveLayout, loadLayout } from '@/lib/tauri';

export function ResultsStep() {
  const { 
    currentProject, 
    results, 
    closeProject,
    selectedPosition,
    selectPosition,
    annotations,
    addAnnotation,
    removeAnnotation,
    config,
  } = useProjectStore();
  const { setCurrentView } = useAppStore();
  const { filters, setFilters, presets, savePreset, loadPreset, deletePreset } = useFilterStore();
  const [localResults, setLocalResults] = useState<AnalysisResult | null>(results);
  const [isLoading, setIsLoading] = useState(false);
  const [layout, setLayout] = useState<Layout[]>(DEFAULT_LAYOUT);
  const [hiddenPanels, setHiddenPanels] = useState<string[]>([]);
  const [showPanelToggle, setShowPanelToggle] = useState(false);
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [showFilterPanel, setShowFilterPanel] = useState(false);
  const [showAnnotations, setShowAnnotations] = useState(false);
  const [goToPosition, setGoToPosition] = useState('');
  const layoutLoadedRef = useRef(false);
  const filterLoadedRef = useRef(false);
  const saveTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const { initializeForProject, clearProject: clearFilters } = useFilterStore();

  // Load results and layout from file if not in memory
  useEffect(() => {
    if (!results && currentProject?.hasResults) {
      loadResults();
    } else if (results) {
      setLocalResults(results);
    }
  }, [results, currentProject]);

  // Initialize filter store for this project
  useEffect(() => {
    if (currentProject && !filterLoadedRef.current) {
      filterLoadedRef.current = true;
      initializeForProject(currentProject.path).catch((error) => {
        console.error('Failed to initialize filters:', error);
      });
    }
    
    return () => {
      // Clear filters when unmounting
      if (filterLoadedRef.current) {
        clearFilters();
        filterLoadedRef.current = false;
      }
    };
  }, [currentProject, initializeForProject, clearFilters]);

  // Load layout from project on mount
  useEffect(() => {
    if (currentProject && !layoutLoadedRef.current) {
      layoutLoadedRef.current = true;
      loadLayout(currentProject.path)
        .then((savedLayout) => {
          if (savedLayout) {
            // Convert from backend format to react-grid-layout format
            setLayout(savedLayout.layout.map(item => ({
              i: item.i,
              x: item.x,
              y: item.y,
              w: item.w,
              h: item.h,
              minW: item.minW,
              minH: item.minH,
            })));
            setHiddenPanels(savedLayout.hidden_panels);
          }
        })
        .catch((error) => {
          console.error('Failed to load layout:', error);
        });
    }
  }, [currentProject]);

  // Select first position by default
  useEffect(() => {
    if (localResults && selectedPosition === null && localResults.results.length > 0) {
      selectPosition(localResults.results[0].position);
    }
  }, [localResults, selectedPosition, selectPosition]);

  // Keyboard navigation for positions (arrow keys)
  useEffect(() => {
    if (!localResults || localResults.results.length === 0) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Only handle if not focused on input elements
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
        return;
      }

      const positions = localResults.results;
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
  }, [localResults, selectedPosition, selectPosition]);

  const loadResults = async () => {
    if (!currentProject) return;
    
    setIsLoading(true);
    try {
      // Read results from project folder
      const { readTextFile } = await import('@tauri-apps/plugin-fs');
      const resultsPath = `${currentProject.path}/results.json`;
      const content = await readTextFile(resultsPath);
      const parsed = JSON.parse(content) as AnalysisResult;
      setLocalResults(parsed);
    } catch (error) {
      console.error('Failed to load results:', error);
    } finally {
      setIsLoading(false);
    }
  };

  // Persist layout with debounce
  const persistLayout = useCallback((newLayout: Layout[], newHiddenPanels: string[]) => {
    if (!currentProject) return;

    // Debounce the save to avoid too many writes
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
    }

    saveTimeoutRef.current = setTimeout(async () => {
      try {
        await saveLayout(currentProject.path, {
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
        console.error('Failed to save layout:', error);
      }
    }, 500);
  }, [currentProject]);

  const handleClose = () => {
    closeProject();
    setCurrentView('projects');
  };

  const handleLayoutChange = useCallback((newLayout: Layout[]) => {
    setLayout(newLayout);
    persistLayout(newLayout, hiddenPanels);
  }, [hiddenPanels, persistLayout]);

  const handleTogglePanel = useCallback((panelId: string) => {
    setHiddenPanels((prev) => {
      const newHiddenPanels = prev.includes(panelId)
        ? prev.filter((id) => id !== panelId)
        : [...prev, panelId];
      persistLayout(layout, newHiddenPanels);
      return newHiddenPanels;
    });
  }, [layout, persistLayout]);

  const handleGoToPosition = () => {
    const pos = parseInt(goToPosition, 10);
    if (!isNaN(pos) && localResults) {
      const exists = localResults.results.some((p) => p.position === pos);
      if (exists) {
        selectPosition(pos);
        setGoToPosition('');
      }
    }
  };

  const data = localResults;

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <RefreshCw className="h-8 w-8 animate-spin text-muted-foreground" />
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
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <div>
                <h1 className="text-lg font-semibold">{currentProject?.name}</h1>
                <p className="text-sm text-muted-foreground">
                  {data.sequence_count.toLocaleString()} sequences • {data.results.length} positions
                </p>
              </div>
              {selectedPosition !== null && (
                <div className="flex items-center gap-2 rounded-full bg-primary/10 px-3 py-1 text-sm">
                  <MapPin className="h-4 w-4 text-primary" />
                  <span>Position {selectedPosition}</span>
                </div>
              )}
            </div>
            <div className="flex items-center gap-2">
              {/* Go to Position */}
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  placeholder="Go to position..."
                  value={goToPosition}
                  onChange={(e) => setGoToPosition(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleGoToPosition()}
                  className="w-32 rounded-md border bg-background px-2 py-1 text-sm"
                />
              </div>
              <Button
                variant={showFilterPanel ? 'default' : 'outline'}
                size="sm"
                onClick={() => setShowFilterPanel(!showFilterPanel)}
                className="gap-2"
              >
                <Filter className="h-4 w-4" />
                Filters
              </Button>
              <Button
                variant={showAnnotations ? 'default' : 'outline'}
                size="sm"
                onClick={() => setShowAnnotations(!showAnnotations)}
                className="gap-2"
              >
                <MessageSquare className="h-4 w-4" />
                Notes
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setShowPanelToggle(!showPanelToggle)}
                className="gap-2"
              >
                <PanelRight className="h-4 w-4" />
                Panels
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
          <DashboardGrid
            results={data}
            selectedPosition={selectedPosition}
            onSelectPosition={selectPosition}
            layout={layout}
            onLayoutChange={handleLayoutChange}
            hiddenPanels={hiddenPanels}
            alphabet={config?.alphabet || 'protein'}
            annotations={annotations}
          />
        </div>
      </div>

      {/* Panel Toggle Sidebar */}
      {showPanelToggle && (
        <PanelToggle
          panels={[]}
          hiddenPanels={hiddenPanels}
          onTogglePanel={handleTogglePanel}
        />
      )}

      {/* Filter Panel Sidebar */}
      {showFilterPanel && (
        <div className="w-80 border-l bg-background overflow-auto">
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
        <div className="w-80 border-l bg-background overflow-auto">
          <AnnotationManager
            annotations={annotations}
            selectedPosition={selectedPosition}
            onAddAnnotation={addAnnotation}
            onRemoveAnnotation={removeAnnotation}
            onGoToPosition={selectPosition}
          />
        </div>
      )}

      {/* Export Dialog */}
      {showExportDialog && currentProject && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <ExportDialog
            projectPath={currentProject.path}
            projectName={currentProject.name}
            onClose={() => setShowExportDialog(false)}
          />
        </div>
      )}
    </div>
  );
}
