/**
 * DiMA Desktop - Analyzing Step
 * 
 * Shows progress during analysis.
 */

import { XCircle, Loader2, CheckCircle2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { cn } from '@/lib/utils';

const STAGES = [
  { id: 'reading_fasta', label: 'Reading FASTA' },
  { id: 'kmer_extraction', label: 'K-mer Extraction' },
  { id: 'entropy_calculation', label: 'Entropy Calculation' },
  { id: 'output_generation', label: 'Output Generation' },
];

export function AnalyzingStep() {
  const { 
    currentProject,
    progress, 
    cancelCurrentAnalysis,
    analysisError,
  } = useProjectStore();

  const currentStageIndex = progress 
    ? STAGES.findIndex(s => s.id === progress.stage)
    : 0;

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-b px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl font-semibold">{currentProject?.name}</h1>
            <p className="text-sm text-muted-foreground">Step 3 of 3: Running Analysis</p>
          </div>
          <Button variant="destructive" onClick={cancelCurrentAnalysis} className="gap-2">
            <XCircle className="h-4 w-4" />
            Cancel
          </Button>
        </div>
      </div>

      {/* Content */}
      <div className="flex flex-1 flex-col items-center justify-center p-6">
        {analysisError ? (
          <div className="flex flex-col items-center gap-4 text-center">
            <div className="rounded-full bg-destructive/10 p-4">
              <XCircle className="h-12 w-12 text-destructive" />
            </div>
            <h2 className="text-xl font-semibold">Analysis Failed</h2>
            <p className="max-w-md text-muted-foreground">{analysisError}</p>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-8">
            {/* Progress Indicator */}
            <div className="flex items-center justify-center">
              <div className="relative">
                <Loader2 className="h-16 w-16 animate-spin text-primary" />
                <div className="absolute inset-0 flex items-center justify-center">
                  <span className="text-sm font-medium">
                    {progress?.current ?? 0}%
                  </span>
                </div>
              </div>
            </div>

            {/* Stage Progress */}
            <div className="w-full max-w-md space-y-4">
              {STAGES.map((stage, index) => {
                const isComplete = index < currentStageIndex || 
                  (progress?.stage === 'complete');
                const isCurrent = index === currentStageIndex && 
                  progress?.stage !== 'complete';
                
                return (
                  <div 
                    key={stage.id} 
                    className={cn(
                      "flex items-center gap-3 rounded-lg p-3 transition-colors",
                      isComplete && "bg-green-500/10",
                      isCurrent && "bg-primary/10",
                      !isComplete && !isCurrent && "opacity-50"
                    )}
                  >
                    {isComplete ? (
                      <CheckCircle2 className="h-5 w-5 text-green-600 dark:text-green-400" />
                    ) : isCurrent ? (
                      <Loader2 className="h-5 w-5 animate-spin text-primary" />
                    ) : (
                      <div className="h-5 w-5 rounded-full border-2" />
                    )}
                    <span className={cn(
                      "font-medium",
                      isComplete && "text-green-600 dark:text-green-400",
                      isCurrent && "text-primary"
                    )}>
                      {stage.label}
                    </span>
                  </div>
                );
              })}
            </div>

            {/* Current Message */}
            {progress?.message && (
              <p className="text-sm text-muted-foreground">{progress.message}</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
