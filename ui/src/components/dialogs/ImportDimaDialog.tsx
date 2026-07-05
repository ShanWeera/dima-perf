/**
 * DiMA Desktop - Import .dima File Dialog
 * 
 * Dialog for importing a .dima binary file into a new project.
 * Uses shadcn Dialog for focus trapping and accessibility.
 */

import { useState } from 'react';
import { FileInput, FolderPlus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { validateProjectName, MAX_PROJECT_NAME_LENGTH } from '@/lib/utils';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from '@/components/ui/dialog';
import { importDimaFile, createProject, deleteProject } from '@/lib/tauri';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore, flushPendingAnnotationSave } from '@/stores/projectStore';

interface ImportDimaDialogProps {
  filePath: string;
  onClose: () => void;
}

export function ImportDimaDialog({ filePath, onClose }: ImportDimaDialogProps) {
  const [projectName, setProjectName] = useState(() => {
    const fileName = filePath.split(/[/\\]/).pop() || 'Imported Analysis';
    return fileName.replace(/\.dima$/, '');
  });
  const [isImporting, setIsImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const { refreshRecentProjects, setCurrentView } = useAppStore();
  const { openExistingProject } = useProjectStore();

  const handleImport = async () => {
    const validation = validateProjectName(projectName);
    if (!validation.valid) {
      setError(validation.error!);
      return;
    }

    setIsImporting(true);
    setError(null);

    // Flush any unsaved annotations from the current project before importing,
    // preventing data loss during the project transition. (Fix 5.83)
    flushPendingAnnotationSave();

    let createdProjectPath: string | null = null;
    try {
      const project = await createProject(projectName.trim());
      createdProjectPath = project.path;
      
      await importDimaFile({
        file_path: filePath,
        project_path: project.path,
      });

      await refreshRecentProjects();
      await openExistingProject(project.path);
      setCurrentView('wizard');
      
      onClose();
    } catch (e) {
      if (createdProjectPath) {
        deleteProject(createdProjectPath).catch(() => {});
        refreshRecentProjects().catch(() => {});
      }
      setError(String(e));
    } finally {
      setIsImporting(false);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="max-w-md p-0">
        <DialogHeader className="border-b px-6 py-4">
          <div className="flex items-center gap-2">
            <FileInput className="h-5 w-5" />
            <DialogTitle>Import .dima File</DialogTitle>
          </div>
          <DialogDescription className="sr-only">
            Import a .dima binary analysis file into a new project
          </DialogDescription>
        </DialogHeader>

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
                maxLength={MAX_PROJECT_NAME_LENGTH}
                onKeyDown={(e) => { if (e.key === 'Enter' && !isImporting) handleImport(); }}
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

        <DialogFooter className="border-t px-6 py-4">
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
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
