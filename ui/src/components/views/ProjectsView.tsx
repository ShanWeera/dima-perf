/**
 * DiMA Desktop - Projects View
 * 
 * Shows recent projects list with the option to create new analysis.
 */

import { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import { FolderOpen, Plus, Trash2, AlertTriangle, Search, X, FileUp } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';
import { useShallow } from 'zustand/react/shallow';
import { formatDate, showErrorToast, validateProjectName } from '@/lib/utils';
import { deleteProject as deleteProjectApi } from '@/lib/tauri';
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { emit } from '@tauri-apps/api/event';

/**
 * Accessible delete confirmation dialog with focus trap and keyboard support.
 * Traps Tab within the dialog, dismisses on Escape, and auto-focuses Cancel.
 */
function DeleteConfirmDialog({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;

    // Auto-focus the Cancel button (safe default for destructive action)
    const cancelBtn = dialog.querySelector<HTMLButtonElement>('[data-cancel]');
    cancelBtn?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCancel();
        return;
      }
      // Focus trap: cycle Tab within the dialog's focusable elements
      if (e.key === 'Tab') {
        const focusable = dialog.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onCancel]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        ref={dialogRef}
        className="mx-4 w-full max-w-sm rounded-lg border bg-card p-6 shadow-lg"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="delete-dialog-title"
        aria-describedby="delete-dialog-desc"
      >
        <div className="flex items-center gap-3 mb-4">
          <AlertTriangle className="h-6 w-6 text-destructive" />
          <h2 id="delete-dialog-title" className="text-lg font-semibold">Delete Project</h2>
        </div>
        <p id="delete-dialog-desc" className="text-sm text-muted-foreground mb-6">
          Are you sure you want to delete this project? This action cannot be undone.
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onCancel} data-cancel>
            Cancel
          </Button>
          <Button variant="destructive" onClick={onConfirm}>
            Delete
          </Button>
        </div>
      </div>
    </div>
  );
}

export function ProjectsView() {
  const { recentProjects, setCurrentView, refreshRecentProjects } = useAppStore(useShallow((s) => ({
    recentProjects: s.recentProjects,
    setCurrentView: s.setCurrentView,
    refreshRecentProjects: s.refreshRecentProjects,
  })));
  const { createNewProject, openExistingProject, currentProject, closeProject, isAnalyzing, cancelCurrentAnalysis } = useProjectStore(useShallow((s) => ({
    createNewProject: s.createNewProject,
    openExistingProject: s.openExistingProject,
    currentProject: s.currentProject,
    closeProject: s.closeProject,
    isAnalyzing: s.isAnalyzing,
    cancelCurrentAnalysis: s.cancelCurrentAnalysis,
  })));
  const [showNameInput, setShowNameInput] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [pendingDeletePath, setPendingDeletePath] = useState<string | null>(null);
  const [nameError, setNameError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  // Client-side project filtering by name or input file
  const filteredProjects = useMemo(() => {
    if (!searchQuery.trim()) return recentProjects;
    const query = searchQuery.toLowerCase();
    return recentProjects.filter(
      (p) => p.name.toLowerCase().includes(query) ||
             (p.input_file_name && p.input_file_name.toLowerCase().includes(query))
    );
  }, [recentProjects, searchQuery]);

  // Cancel any running analysis before switching projects (Fix 4.19)
  const cancelIfAnalyzing = async () => {
    if (isAnalyzing) {
      try {
        await cancelCurrentAnalysis();
      } catch {
        // Proceed even if cancel fails — closeProject bumps generation counter
      }
    }
  };

  const handleNewAnalysis = async () => {
    const name = projectName.trim() || `Analysis ${new Date().toISOString().slice(0, 10)}`;
    const validation = validateProjectName(name);
    if (!validation.valid) {
      setNameError(validation.error ?? 'Invalid project name');
      return;
    }
    await cancelIfAnalyzing();
    setIsCreating(true);
    try {
      await createNewProject(name);
      setCurrentView('wizard');
    } catch (error) {
      showErrorToast('Failed to create project', error);
    } finally {
      setIsCreating(false);
      setShowNameInput(false);
      setProjectName('');
      setNameError(null);
    }
  };

  const handleOpenProject = async (path: string) => {
    await cancelIfAnalyzing();
    try {
      await openExistingProject(path);
      setCurrentView('wizard');
    } catch (error) {
      showErrorToast('Failed to open project', error);
    }
  };

  const handleDeleteClick = useCallback((path: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setPendingDeletePath(path);
  }, []);

  const confirmDelete = useCallback(async () => {
    if (!pendingDeletePath) return;
    try {
      if (currentProject?.path === pendingDeletePath) {
        closeProject();
      }
      await deleteProjectApi(pendingDeletePath);
      await refreshRecentProjects();
    } catch (error) {
      showErrorToast('Failed to delete project', error);
    } finally {
      setPendingDeletePath(null);
    }
  }, [pendingDeletePath, currentProject?.path, closeProject, refreshRecentProjects]);

  const handleImportDima = async () => {
    try {
      const selected = await openFileDialog({
        title: 'Import .dima File',
        filters: [{ name: 'DiMA Results', extensions: ['dima'] }],
        multiple: false,
      });
      if (selected) {
        await emit('dima://file-open', { path: selected });
      }
    } catch (error) {
      showErrorToast('Failed to open file dialog', error);
    }
  };

  return (
    <div className="flex h-full flex-col p-6">
      <div className="mb-6 flex items-center justify-between gap-4">
        <h1 className="text-2xl font-bold">Recent Projects</h1>
        {showNameInput ? (
          <div className="flex items-center gap-2">
            <div className="flex flex-col gap-1">
              <Input
                placeholder="Project name..."
                value={projectName}
                onChange={(e) => { setProjectName(e.target.value); setNameError(null); }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleNewAnalysis();
                  if (e.key === 'Escape') setShowNameInput(false);
                }}
                className="w-48"
                autoFocus
                maxLength={100}
              />
              {nameError && <p className="text-xs text-destructive">{nameError}</p>}
            </div>
            <Button onClick={handleNewAnalysis} disabled={isCreating} size="sm">
              {isCreating ? 'Creating...' : 'Create'}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setShowNameInput(false)}>
              Cancel
            </Button>
          </div>
        ) : (
          <div className="flex gap-2">
            <Button variant="outline" onClick={handleImportDima} className="gap-2">
              <FileUp className="h-4 w-4" />
              Import .dima
            </Button>
            <Button onClick={() => setShowNameInput(true)} className="gap-2">
              <Plus className="h-4 w-4" />
              New Analysis
            </Button>
          </div>
        )}
      </div>

      {recentProjects.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <FolderOpen className="h-16 w-16 text-muted-foreground/50" />
          <p className="text-muted-foreground">No recent projects</p>
          <Button onClick={() => setShowNameInput(true)} variant="outline" className="gap-2">
            <Plus className="h-4 w-4" />
            Create your first analysis
          </Button>
        </div>
      ) : (
        <>
          {recentProjects.length > 5 && (
            <div className="relative mb-4">
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                placeholder="Search projects..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9 pr-8"
                aria-label="Search recent projects"
              />
              {searchQuery && (
                <button
                  onClick={() => setSearchQuery('')}
                  className="absolute right-2 top-1/2 -translate-y-1/2 rounded-sm p-0.5 text-muted-foreground hover:text-foreground"
                  aria-label="Clear search"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          )}
          {filteredProjects.length === 0 ? (
            <p className="text-center text-muted-foreground py-8">No projects match "{searchQuery}"</p>
          ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {filteredProjects.map((project) => (
            <div
              key={project.path}
              tabIndex={0}
              onClick={() => handleOpenProject(project.path)}
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleOpenProject(project.path); }}}
              className="group relative flex flex-col gap-2 rounded-lg border bg-card p-4 text-left transition-colors hover:bg-accent cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-label={`Open project ${project.name}`}
            >
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-2">
                  <FolderOpen className="h-5 w-5 text-muted-foreground" />
                  <h3 className="font-medium truncate">{project.name}</h3>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100"
                  onClick={(e) => handleDeleteClick(project.path, e)}
                  aria-label={`Delete project ${project.name}`}
                >
                  <Trash2 className="h-4 w-4 text-destructive" />
                </Button>
              </div>
              {project.input_file_name && (
                <p className="truncate text-sm text-muted-foreground" title={project.input_file_name}>
                  {project.input_file_name}
                </p>
              )}
              <p className="text-xs text-muted-foreground">
                Last opened: {formatDate(project.last_opened)}
              </p>
              {project.sequence_count && (
                <p className="text-xs text-muted-foreground">
                  {project.sequence_count.toLocaleString()} sequences
                </p>
              )}
            </div>
          ))}
        </div>
          )}
        </>
      )}

      {/* Delete confirmation dialog with focus trap + Escape dismiss (Fix 5.50) */}
      {pendingDeletePath && (
        <DeleteConfirmDialog
          onConfirm={confirmDelete}
          onCancel={() => setPendingDeletePath(null)}
        />
      )}
    </div>
  );
}
