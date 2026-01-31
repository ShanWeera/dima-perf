/**
 * DiMA Desktop - Welcome View
 * 
 * Shown on first run when there are no projects.
 */

import { Dna, Plus, ArrowRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';

export function WelcomeView() {
  const { setCurrentView } = useAppStore();
  const { createNewProject } = useProjectStore();

  const handleGetStarted = async () => {
    // For now, create with a default name
    const name = `Analysis ${new Date().toISOString().slice(0, 19).replace('T', ' ')}`;
    try {
      await createNewProject(name);
      setCurrentView('wizard');
    } catch (error) {
      console.error('Failed to create project:', error);
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

      <Button size="lg" onClick={handleGetStarted} className="gap-2">
        <Plus className="h-5 w-5" />
        Get Started
        <ArrowRight className="h-5 w-5" />
      </Button>

      <div className="mt-8 grid grid-cols-3 gap-8 text-center">
        <div className="flex flex-col items-center gap-2">
          <div className="rounded-lg bg-muted p-3">
            <span className="text-2xl">📊</span>
          </div>
          <h3 className="font-medium">Interactive Visualizations</h3>
          <p className="text-sm text-muted-foreground">
            Explore entropy and diversity with synchronized charts
          </p>
        </div>
        <div className="flex flex-col items-center gap-2">
          <div className="rounded-lg bg-muted p-3">
            <span className="text-2xl">🔬</span>
          </div>
          <h3 className="font-medium">Detailed Analysis</h3>
          <p className="text-sm text-muted-foreground">
            K-mer extraction, entropy calculation, and motif classification
          </p>
        </div>
        <div className="flex flex-col items-center gap-2">
          <div className="rounded-lg bg-muted p-3">
            <span className="text-2xl">📁</span>
          </div>
          <h3 className="font-medium">Project Management</h3>
          <p className="text-sm text-muted-foreground">
            Organize and annotate your analyses
          </p>
        </div>
      </div>
    </div>
  );
}
