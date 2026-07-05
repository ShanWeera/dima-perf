/**
 * DiMA Desktop - Wizard View
 * 
 * Main wizard for analysis workflow: Input -> Configure -> Results.
 * Tracks step direction for CSS slide/fade transitions (9.3.4).
 */

import { useRef } from 'react';
import { useProjectStore } from '@/stores/projectStore';
import { InputStep } from '@/components/wizard/InputStep';
import { ConfigureStep } from '@/components/wizard/ConfigureStep';
import { AnalyzingStep } from '@/components/wizard/AnalyzingStep';
import { ResultsStep } from '@/components/wizard/ResultsStep';

const STEP_ORDER = ['input', 'configure', 'analyzing', 'results'] as const;

function getStepIndex(step: string): number {
  const idx = STEP_ORDER.indexOf(step as typeof STEP_ORDER[number]);
  return idx >= 0 ? idx : 0;
}

export function WizardView() {
  const { wizardStep } = useProjectStore();
  const prevStepRef = useRef(wizardStep);

  // Determine slide direction based on step order comparison
  const direction = getStepIndex(wizardStep) >= getStepIndex(prevStepRef.current) ? 'forward' : 'backward';
  // Update ref after computing direction (so next render compares to current step)
  if (prevStepRef.current !== wizardStep) {
    prevStepRef.current = wizardStep;
  }

  const animClass = direction === 'forward' ? 'wizard-step-forward' : 'wizard-step-backward';

  return (
    <div className="flex h-full flex-col">
      {/* key forces remount on step change, triggering the CSS animation */}
      <div key={wizardStep} className={`flex-1 min-h-0 flex flex-col ${animClass}`}>
        {wizardStep === 'input' && <InputStep />}
        {wizardStep === 'configure' && <ConfigureStep />}
        {wizardStep === 'analyzing' && <AnalyzingStep />}
        {wizardStep === 'results' && <ResultsStep />}
      </div>
    </div>
  );
}
