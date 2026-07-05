/**
 * DiMA Desktop - Analyzing Step
 * 
 * Shows progress during analysis with stall detection.
 */

import { useState, useEffect, useRef } from 'react';
import { XCircle, Loader2, CheckCircle2, RotateCw, ArrowLeft, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { useShallow } from 'zustand/react/shallow';
import { cn } from '@/lib/utils';
import { AriaLive } from '@/components/ui/aria-live';

// If no progress update arrives within this duration, show a stall warning.
// Large datasets with many positions can take time per stage, but >90s of
// silence strongly suggests the backend is hung.
const STALL_TIMEOUT_MS = 90_000;

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
    startAnalysis,
    setWizardStep,
    analysisError,
  } = useProjectStore(useShallow((s) => ({
    currentProject: s.currentProject,
    progress: s.progress,
    cancelCurrentAnalysis: s.cancelCurrentAnalysis,
    startAnalysis: s.startAnalysis,
    setWizardStep: s.setWizardStep,
    analysisError: s.analysisError,
  })));

  const currentStageIndex = progress 
    ? STAGES.findIndex(s => s.id === progress.stage)
    : 0;

  // Stall detection: warn the user if no progress arrives for STALL_TIMEOUT_MS.
  // Resets whenever `progress` changes (new progress event received).
  const [isStalled, setIsStalled] = useState(false);
  const stallTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Only run stall detection while analysis is active (no error state)
    if (analysisError) {
      setIsStalled(false);
      return;
    }

    // Clear previous timer and reset stall flag on new progress
    setIsStalled(false);
    if (stallTimerRef.current) clearTimeout(stallTimerRef.current);

    stallTimerRef.current = setTimeout(() => {
      setIsStalled(true);
    }, STALL_TIMEOUT_MS);

    return () => {
      if (stallTimerRef.current) clearTimeout(stallTimerRef.current);
    };
  }, [progress, analysisError]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-b px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl font-semibold truncate min-w-0">{currentProject?.name}</h1>
            <p className="text-sm text-muted-foreground">Running Analysis...</p>
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
            <div className="flex items-center gap-3 mt-2">
              <Button variant="outline" onClick={() => setWizardStep('configure')} className="gap-2">
                <ArrowLeft className="h-4 w-4" />
                Back to Configure
              </Button>
              <Button onClick={() => startAnalysis()} className="gap-2">
                <RotateCw className="h-4 w-4" />
                Retry Analysis
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-8">
            {/* Progress Indicator */}
            <div className="flex items-center justify-center">
              <div className="relative">
                <Loader2 className="h-16 w-16 animate-spin text-primary" />
                <div className="absolute inset-0 flex items-center justify-center">
                  <span className="text-sm font-medium">
                    {progress && progress.total > 0
                      ? `${Math.min(100, Math.round((progress.current / progress.total) * 100))}%`
                      : '0%'}
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

            {/* Overall Progress Bar */}
            {progress && progress.total > 0 && (
              <div className="w-full max-w-md">
                <div
                  className="h-2 w-full overflow-hidden rounded-full bg-muted"
                  role="progressbar"
                  aria-valuenow={Math.min(100, Math.round((progress.current / progress.total) * 100))}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-label="Analysis progress"
                >
                  <div
                    className="h-full rounded-full bg-primary transition-all duration-300"
                    style={{ width: `${Math.min(100, (progress.current / progress.total) * 100)}%` }}
                  />
                </div>
                <p className="mt-1 text-xs text-muted-foreground text-right">
                  {progress.current.toLocaleString()} / {progress.total.toLocaleString()}
                  {progress.throughput ? ` • ${progress.throughput.toLocaleString()} pos/s` : ''}
                </p>
              </div>
            )}

            {/* Current Message */}
            {progress?.message && (
              <p className="text-sm text-muted-foreground">{progress.message}</p>
            )}

            {/* Stall Warning */}
            {isStalled && (
              <div className="flex items-center gap-2 rounded-lg border border-yellow-500/50 bg-yellow-500/10 px-4 py-2 text-sm text-yellow-700 dark:text-yellow-300">
                <AlertTriangle className="h-4 w-4 flex-shrink-0" />
                <span>
                  No progress received for over 90 seconds. The analysis may be stuck.
                  You can cancel and retry, or continue waiting.
                </span>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Screen reader announcement for analysis status */}
      <AriaLive
        message={
          analysisError
            ? `Analysis failed: ${analysisError}`
            : progress?.message || 'Analysis in progress'
        }
        politeness={analysisError ? 'assertive' : 'polite'}
      />
    </div>
  );
}
