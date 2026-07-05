/**
 * DiMA Desktop - Settings View
 * 
 * Application settings panel with auto-saving.
 */

import { useShallow } from 'zustand/react/shallow';
import { useSettingsStore } from '@/stores/settingsStore';
import { useProjectStore } from '@/stores/projectStore';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { revealInExplorer, getProjectsDirectoryPath, saveLayout } from '@/lib/tauri';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { DEFAULT_LAYOUT } from '@/lib/dashboard-layout';
import { useState, useEffect } from 'react';
import { FolderOpen } from 'lucide-react';
import { showErrorToast } from '@/lib/utils';
import { useToastStore } from '@/stores/toastStore';
import { ConfirmationDialog } from '@/components/dialogs/ConfirmationDialog';

export function SettingsView() {
  const { settings, updateSetting, resetToDefaults, setThemeMode } = useSettingsStore(useShallow((s) => ({
    settings: s.settings,
    updateSetting: s.updateSetting,
    resetToDefaults: s.resetToDefaults,
    setThemeMode: s.setThemeMode,
  })));
  const currentProject = useProjectStore((s) => s.currentProject);
  const bumpLayoutResetVersion = useProjectStore((s) => s.bumpLayoutResetVersion);
  const [projectsPath, setProjectsPath] = useState<string>('');
  const [isResettingLayout, setIsResettingLayout] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);

  const handleSelectOutputDirectory = async () => {
    try {
      const selected = await openDialog({
        directory: true,
        title: 'Select Default Output Directory',
      });
      if (selected) {
        await updateSetting('defaultOutputDirectory', selected as string);
      }
    } catch (error) {
      showErrorToast('Failed to select directory', error);
    }
  };

  const handleClearOutputDirectory = async () => {
    await updateSetting('defaultOutputDirectory', null);
  };

  const handleResetLayout = async () => {
    if (!currentProject) return;
    
    setIsResettingLayout(true);
    try {
      await saveLayout(currentProject.path, {
        layout: DEFAULT_LAYOUT.map(item => ({
          i: item.i,
          x: item.x,
          y: item.y,
          w: item.w,
          h: item.h,
          minW: item.minW,
          minH: item.minH,
        })),
        hidden_panels: [],
      });
      bumpLayoutResetVersion();
      useToastStore.getState().addToast('Dashboard layout reset', 'success');
    } catch (error) {
      showErrorToast('Failed to reset layout', error);
    } finally {
      setIsResettingLayout(false);
    }
  };

  useEffect(() => {
    getProjectsDirectoryPath().then(setProjectsPath).catch((err) => showErrorToast('Failed to get projects path', err));
  }, []);

  return (
    <div className="flex h-full flex-col overflow-auto p-6">
      <h1 className="mb-6 text-2xl font-bold">Settings</h1>

      <div className="max-w-2xl space-y-8">
        {/* Appearance */}
        <section>
          <h2 className="mb-4 text-lg font-semibold">Appearance</h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <label htmlFor="settings-theme" className="font-medium">Theme</label>
                <p className="text-sm text-muted-foreground">
                  Choose your preferred color scheme
                </p>
              </div>
              <Select value={settings.theme} onValueChange={(v) => setThemeMode(v as 'light' | 'dark' | 'system')}>
                <SelectTrigger className="w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="system">System</SelectItem>
                  <SelectItem value="light">Light</SelectItem>
                  <SelectItem value="dark">Dark</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center justify-between">
              <div>
                <label htmlFor="settings-precision" className="font-medium">Decimal Precision</label>
                <p className="text-sm text-muted-foreground">
                  Number of decimal places for entropy values
                </p>
              </div>
              <Select value={String(settings.decimalPrecision)} onValueChange={(v) => updateSetting('decimalPrecision', Number(v))}>
                <SelectTrigger className="w-20">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="2">2</SelectItem>
                  <SelectItem value="3">3</SelectItem>
                  <SelectItem value="4">4</SelectItem>
                  <SelectItem value="5">5</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </section>

        {/* Analysis Defaults */}
        <section>
          <h2 className="mb-4 text-lg font-semibold">Analysis Defaults</h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <label htmlFor="settings-kmer" className="font-medium">K-mer Length</label>
                <p className="text-sm text-muted-foreground">
                  Default sliding window size (protein max: 14)
                </p>
              </div>
              <Input
                id="settings-kmer"
                type="number"
                min={3}
                max={14}
                value={settings.defaultKmerLength}
                onChange={(e) => {
                  const val = Number(e.target.value);
                  if (Number.isFinite(val) && val >= 3 && val <= 14) updateSetting('defaultKmerLength', val);
                }}
                className="w-20"
              />
            </div>

            <div className="flex items-center justify-between">
              <div>
                <label htmlFor="settings-threshold" className="font-medium">Support Threshold</label>
                <p className="text-sm text-muted-foreground">
                  Minimum support for entropy calculation
                </p>
              </div>
              <Input
                id="settings-threshold"
                type="number"
                min={1}
                max={10000}
                value={settings.defaultSupportThreshold}
                onChange={(e) => {
                  const val = Number(e.target.value);
                  if (Number.isFinite(val) && val >= 1 && val <= 10000) updateSetting('defaultSupportThreshold', val);
                }}
                className="w-20"
              />
            </div>
          </div>
        </section>

        {/* Storage */}
        <section>
          <h2 className="mb-4 text-lg font-semibold">Storage</h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <label className="font-medium">Projects Location</label>
                <p className="truncate text-sm text-muted-foreground" title={projectsPath}>
                  {projectsPath || 'Loading...'}
                </p>
              </div>
              <Button 
                variant="outline" 
                onClick={() => projectsPath && revealInExplorer(projectsPath)}
                disabled={!projectsPath}
              >
                Show in file manager
              </Button>
            </div>
          </div>
        </section>

        {/* Export Defaults */}
        <section>
          <h2 className="mb-4 text-lg font-semibold">Export Defaults</h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <label htmlFor="settings-output-dir" className="font-medium">Default Output Directory</label>
                <p className="truncate text-sm text-muted-foreground" title={settings.defaultOutputDirectory || 'Same as project folder'}>
                  {settings.defaultOutputDirectory || 'Same as project folder'}
                </p>
              </div>
              <div className="flex gap-2">
                <Button 
                  variant="outline" 
                  onClick={handleSelectOutputDirectory}
                  className="gap-2"
                >
                  <FolderOpen className="h-4 w-4" />
                  Browse
                </Button>
                {settings.defaultOutputDirectory && (
                  <Button 
                    variant="ghost" 
                    onClick={handleClearOutputDirectory}
                  >
                    Clear
                  </Button>
                )}
              </div>
            </div>

          </div>
        </section>

        {/* Reset */}
        <section>
          <h2 className="mb-4 text-lg font-semibold">Reset</h2>
          <div className="flex flex-wrap gap-4">
            <Button 
              variant="outline" 
              onClick={handleResetLayout}
              disabled={!currentProject || isResettingLayout}
            >
              {isResettingLayout ? 'Resetting...' : 'Reset Dashboard Layout'}
            </Button>
            <Button variant="outline" onClick={() => setShowResetConfirm(true)}>
              Reset All Settings
            </Button>
          </div>
          {!currentProject && (
            <p className="mt-2 text-sm text-muted-foreground">
              Open a project to reset its dashboard layout.
            </p>
          )}
          <p className="mt-2 text-sm text-muted-foreground">
            Settings are automatically saved when you make changes.
          </p>
        </section>
      </div>

      <ConfirmationDialog
        open={showResetConfirm}
        onOpenChange={setShowResetConfirm}
        title="Reset All Settings"
        description="All settings will be reverted to their default values. This cannot be undone."
        confirmLabel="Reset All"
        variant="warning"
        onConfirm={() => { resetToDefaults().then(() => useToastStore.getState().addToast('Settings restored to defaults', 'success')).catch(() => {}); setShowResetConfirm(false); }}
      />
    </div>
  );
}
