/**
 * DiMA Desktop - Main Content Area
 * 
 * Renders the appropriate view based on current app state.
 */

import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';
import { WelcomeView } from '@/components/views/WelcomeView';
import { ProjectsView } from '@/components/views/ProjectsView';
import { WizardView } from '@/components/views/WizardView';
import { SettingsView } from '@/components/views/SettingsView';
import { AboutView } from '@/components/views/AboutView';

export function MainContent() {
  const { currentView } = useAppStore();
  const { currentProject } = useProjectStore();

  // If we have a current project, show wizard or results
  if (currentProject) {
    return (
      <main className="flex-1 overflow-hidden">
        <WizardView />
      </main>
    );
  }

  // Otherwise, show based on currentView
  return (
    <main className="flex-1 overflow-hidden">
      {currentView === 'welcome' && <WelcomeView />}
      {currentView === 'projects' && <ProjectsView />}
      {currentView === 'settings' && <SettingsView />}
      {currentView === 'about' && <AboutView />}
    </main>
  );
}
