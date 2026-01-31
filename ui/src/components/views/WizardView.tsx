/**
 * DiMA Desktop - Wizard View
 * 
 * Main wizard for analysis workflow: Input -> Configure -> Results
 */

import { useProjectStore } from '@/stores/projectStore';
import { InputStep } from '@/components/wizard/InputStep';
import { ConfigureStep } from '@/components/wizard/ConfigureStep';
import { AnalyzingStep } from '@/components/wizard/AnalyzingStep';
import { ResultsStep } from '@/components/wizard/ResultsStep';

export function WizardView() {
  const { wizardStep } = useProjectStore();

  return (
    <div className="flex h-full flex-col">
      {wizardStep === 'input' && <InputStep />}
      {wizardStep === 'configure' && <ConfigureStep />}
      {wizardStep === 'analyzing' && <AnalyzingStep />}
      {wizardStep === 'results' && <ResultsStep />}
    </div>
  );
}
