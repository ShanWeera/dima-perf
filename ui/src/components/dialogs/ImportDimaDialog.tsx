/**
 * DiMA Desktop - Import .dima File Dialog
 * 
 * Dialog for importing a .dima binary file into a new or existing project.
 */

import { useState } from 'react';
import { X, FileInput, FolderPlus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { importDimaFile, createProject } from '@/lib/tauri';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';

interface ImportDimaDialogProps {
  filePath: string;
  onClose: () => void;
}

export function ImportDimaDialog({ filePath, onClose }: ImportDimaDialogProps) {
  const [projectName, setProjectName] = useState(() => {
    // Extract filename without extension as default project name
    const fileName = filePath.split(/[/\\]/).pop() || 'Imported Analysis';
    return fileName.replace(/\.dima$/, '');
  });
  const [isImporting, setIsImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const { refreshRecentProjects, setCurrentView } = useAppStore();
  const { openExistingProject } = useProjectStore();

  const handleImport = async () => {
    if (!projectName.trim()) {
      setError('Project name is required');
      return;
    }

    setIsImporting(true);
    setError(null);

    try {
      // Create a new project
      const project = await createProject(projectName.trim());
      
      // Import the .dima file into the project
      await importDimaFile({
        file_path: filePath,
        project_path: project.path,
      });

      // Refresh recent projects and open the new project
      await refreshRecentProjects();
      await openExistingProject(project.path);
      setCurrentView('results');
      
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsImporting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg bg-background shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b px-6 py-4">
          <div className="flex items-center gap-2">
            <FileInput className="h-5 w-5" />
            <h2 className="text-lg font-semibold">Import .dima File</h2>
          </div>
          <button onClick={onClose} className="rounded-md p-2 hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-4">
          <div className="rounded-lg bg-muted p-3 text-sm">
            <p className="font-medium">File to import:</p>
            <p className="font-mono text-xs text-muted-foreground truncate mt-1">
              {filePath}
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="projectName">Create New Project</Label>
            <div className="flex gap-2">
              <FolderPlus className="h-5 w-5 text-muted-foreground mt-2" />
              <Input
                id="projectName"
                value={projectName}
                onChange={(e) => setProjectName(e.target.value)}
                placeholder="Enter project name..."
              />
            </div>
            <p className="text-xs text-muted-foreground">
              The .dima file will be imported into a new project with this name.
            </p>
          </div>

          {error && (
            <div className="rounded-lg bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 border-t px-6 py-4">
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={handleImport} disabled={isImporting} className="gap-2">
            {isImporting ? (
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
            ) : (
              <FileInput className="h-4 w-4" />
            )}
            Import
          </Button>
        </div>
      </div>
    </div>
  );
}
