/**
 * DiMA Desktop - Welcome View
 * 
 * Shown on first run when there are no projects.
 */

import { useState } from 'react';
import { Dna, Plus, ArrowRight, BarChart3, Microscope, FolderOpen } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';
import { showErrorToast, validateProjectName } from '@/lib/utils';

export function WelcomeView() {
  const { setCurrentView } = useAppStore();
  const { createNewProject, isAnalyzing, cancelCurrentAnalysis } = useProjectStore();
  const [showNameInput, setShowNameInput] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [nameError, setNameError] = useState<string | null>(null);

  const handleCreate = async () => {
    const name = projectName.trim() || `Analysis ${new Date().toISOString().slice(0, 10)}`;
    const validation = validateProjectName(name);
    if (!validation.valid) {
      setNameError(validation.error ?? 'Invalid project name');
      return;
    }

    // Cancel any running analysis before creating a new project (Fix 4.19)
    if (isAnalyzing) {
      try {
        await cancelCurrentAnalysis();
      } catch {
        // Proceed even if cancel fails — the new project creation
        // will call closeProject which bumps the generation counter
      }
    }

    setIsCreating(true);
    try {
      await createNewProject(name);
      setCurrentView('wizard');
    } catch (error) {
      showErrorToast('Failed to create project', error);
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <div className="flex h-full flex-col items-center justify-center gap-8 p-8">
      <div className="flex flex-col items-center gap-4">
        <div className="rounded-full bg-primary/10 p-6">
          <Dna className="h-16 w-16 text-primary" />
        </div>
        <h1 className="text-3xl font-bold">Welcome to DiMA Desktop</h1>
        <p className="max-w-md text-center text-muted-foreground">
          Analyze protein and nucleotide sequence diversity using k-mer based 
          entropy analysis. Get started by creating your first analysis.
        </p>
      </div>

      {showNameInput ? (
        <div className="flex flex-col items-center gap-3 w-full max-w-sm">
          <div className="w-full space-y-2">
            <Label htmlFor="welcome-project-name">Project Name</Label>
            <Input
              id="welcome-project-name"
              placeholder="My Analysis"
              value={projectName}
              onChange={(e) => { setProjectName(e.target.value); setNameError(null); }}
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
              autoFocus
              maxLength={100}
            />
            {nameError && <p className="text-sm text-destructive">{nameError}</p>}
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => setShowNameInput(false)}>
              Cancel
            </Button>
            <Button onClick={handleCreate} disabled={isCreating} className="gap-2">
              <Plus className="h-4 w-4" />
              {isCreating ? 'Creating...' : 'Create'}
            </Button>
          </div>
        </div>
      ) : (
        <Button size="lg" onClick={() => setShowNameInput(true)} className="gap-2">
          <Plus className="h-5 w-5" />
          Get Started
          <ArrowRight className="h-5 w-5" />
        </Button>
      )}

      <div className="mt-8 grid grid-cols-3 gap-8 text-center">
        <div className="flex flex-col items-center gap-2">
          <div className="rounded-lg bg-muted p-3">
            <BarChart3 className="h-6 w-6 text-primary" />
          </div>
          <h2 className="font-medium">Interactive Visualizations</h2>
          <p className="text-sm text-muted-foreground">
            Explore entropy and diversity with synchronized charts
          </p>
        </div>
        <div className="flex flex-col items-center gap-2">
          <div className="rounded-lg bg-muted p-3">
            <Microscope className="h-6 w-6 text-primary" />
          </div>
          <h2 className="font-medium">Detailed Analysis</h2>
          <p className="text-sm text-muted-foreground">
            K-mer extraction, entropy calculation, and motif classification
          </p>
        </div>
        <div className="flex flex-col items-center gap-2">
          <div className="rounded-lg bg-muted p-3">
            <FolderOpen className="h-6 w-6 text-primary" />
          </div>
          <h2 className="font-medium">Project Management</h2>
          <p className="text-sm text-muted-foreground">
            Organize and annotate your analyses
          </p>
        </div>
      </div>
    </div>
  );
}
