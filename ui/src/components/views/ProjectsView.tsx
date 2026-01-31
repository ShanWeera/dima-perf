/**
 * DiMA Desktop - Projects View
 * 
 * Shows recent projects list with the option to create new analysis.
 */

import { FolderOpen, Plus, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';
import { formatDate } from '@/lib/utils';
import { deleteProject as deleteProjectApi } from '@/lib/tauri';

export function ProjectsView() {
  const { recentProjects, setCurrentView, refreshRecentProjects } = useAppStore();
  const { createNewProject, openExistingProject } = useProjectStore();

  const handleNewAnalysis = async () => {
    const name = `Analysis ${new Date().toISOString().slice(0, 19).replace('T', ' ')}`;
    try {
      await createNewProject(name);
      setCurrentView('wizard');
    } catch (error) {
      console.error('Failed to create project:', error);
    }
  };

  const handleOpenProject = async (path: string) => {
    try {
      await openExistingProject(path);
      setCurrentView('wizard');
    } catch (error) {
      console.error('Failed to open project:', error);
    }
  };

  const handleDeleteProject = async (path: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (confirm('Are you sure you want to delete this project? This cannot be undone.')) {
      try {
        await deleteProjectApi(path);
        await refreshRecentProjects();
      } catch (error) {
        console.error('Failed to delete project:', error);
      }
    }
  };

  return (
    <div className="flex h-full flex-col p-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">Recent Projects</h1>
        <Button onClick={handleNewAnalysis} className="gap-2">
          <Plus className="h-4 w-4" />
          New Analysis
        </Button>
      </div>

      {recentProjects.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <FolderOpen className="h-16 w-16 text-muted-foreground/50" />
          <p className="text-muted-foreground">No recent projects</p>
          <Button onClick={handleNewAnalysis} variant="outline" className="gap-2">
            <Plus className="h-4 w-4" />
            Create your first analysis
          </Button>
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {recentProjects.map((project) => (
            <button
              key={project.path}
              onClick={() => handleOpenProject(project.path)}
              className="group relative flex flex-col gap-2 rounded-lg border bg-card p-4 text-left transition-colors hover:bg-accent"
            >
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-2">
                  <FolderOpen className="h-5 w-5 text-muted-foreground" />
                  <h3 className="font-medium">{project.name}</h3>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 opacity-0 group-hover:opacity-100"
                  onClick={(e) => handleDeleteProject(project.path, e)}
                >
                  <Trash2 className="h-4 w-4 text-destructive" />
                </Button>
              </div>
              {project.input_file_name && (
                <p className="text-sm text-muted-foreground">
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
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
