/**
 * DiMA Desktop - Main Content Area
 * 
 * Renders the appropriate view based on current app state.
 */

import { Loader2 } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';
import { WelcomeView } from '@/components/views/WelcomeView';
import { ProjectsView } from '@/components/views/ProjectsView';
import { WizardView } from '@/components/views/WizardView';
import { SettingsView } from '@/components/views/SettingsView';
import { AboutView } from '@/components/views/AboutView';

export function MainContent() {
  const currentView = useAppStore((s) => s.currentView);
  const { currentProject, isLoadingProject } = useProjectStore(useShallow((s) => ({ currentProject: s.currentProject, isLoadingProject: s.isLoadingProject })));

  const isGlobalView = currentView === 'settings' || currentView === 'about';

  if (isLoadingProject) {
    return (
      <div className="flex flex-1 items-center justify-center overflow-hidden" role="status" aria-live="polite" aria-busy="true">
        <div className="flex flex-col items-center gap-3 text-muted-foreground">
          <Loader2 className="h-8 w-8 animate-spin" />
          <p className="text-sm">Loading project...</p>
        </div>
      </div>
    );
  }

  // Determine which view to show. If no project is open and we're not on a
  // recognized global/standalone view, fall back to WelcomeView to prevent
  // a blank main area (e.g. if closeProject() is called without updating
  // currentView in appStore). (Fix 2.7)
  const showWizard = currentProject && !isGlobalView
    && currentView !== 'welcome' && currentView !== 'projects';
  const showFallback = !currentProject && !isGlobalView
    && currentView !== 'welcome' && currentView !== 'projects';

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden min-w-[400px]">
      {currentView === 'settings' && <SettingsView />}
      {currentView === 'about' && <AboutView />}
      {(currentView === 'welcome' || showFallback) && <WelcomeView />}
      {currentView === 'projects' && <ProjectsView />}
      {showWizard && <WizardView />}
    </div>
  );
}
