/**
 * DiMA Desktop - Settings View
 * 
 * Application settings panel with auto-saving.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { useThemeStore } from '@/stores/themeStore';
import { useProjectStore } from '@/stores/projectStore';
import { Button } from '@/components/ui/button';
import { revealInExplorer, getDocumentsPath, saveLayout } from '@/lib/tauri';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { DEFAULT_LAYOUT } from '@/components/dashboard/DashboardGrid';
import { useState, useEffect } from 'react';
import { FolderOpen } from 'lucide-react';

export function SettingsView() {
  const { settings, updateSetting, resetToDefaults } = useSettingsStore();
  const { mode, setMode } = useThemeStore();
  const { currentProject } = useProjectStore();
  const [documentsPath, setDocumentsPath] = useState<string>('');
  const [isResettingLayout, setIsResettingLayout] = useState(false);

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
      console.error('Failed to select directory:', error);
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
      // The layout will be reloaded when user returns to results view
    } catch (error) {
      console.error('Failed to reset layout:', error);
    } finally {
      setIsResettingLayout(false);
    }
  };

  useEffect(() => {
    getDocumentsPath().then(setDocumentsPath).catch(console.error);
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
                <label className="font-medium">Theme</label>
                <p className="text-sm text-muted-foreground">
                  Choose your preferred color scheme
                </p>
              </div>
              <select
                value={mode}
                onChange={(e) => setMode(e.target.value as 'light' | 'dark' | 'system')}
                className="rounded-md border bg-background px-3 py-2"
              >
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </div>

            <div className="flex items-center justify-between">
              <div>
                <label className="font-medium">Decimal Precision</label>
                <p className="text-sm text-muted-foreground">
                  Number of decimal places for entropy values
                </p>
              </div>
              <select
                value={settings.decimalPrecision}
                onChange={(e) => updateSetting('decimalPrecision', Number(e.target.value))}
                className="rounded-md border bg-background px-3 py-2"
              >
                <option value={2}>2</option>
                <option value={3}>3</option>
                <option value={4}>4</option>
                <option value={5}>5</option>
              </select>
            </div>
          </div>
        </section>

        {/* Analysis Defaults */}
        <section>
          <h2 className="mb-4 text-lg font-semibold">Analysis Defaults</h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <label className="font-medium">K-mer Length</label>
                <p className="text-sm text-muted-foreground">
                  Default sliding window size
                </p>
              </div>
              <input
                type="number"
                min={3}
                max={15}
                value={settings.defaultKmerLength}
                onChange={(e) => updateSetting('defaultKmerLength', Number(e.target.value))}
                className="w-20 rounded-md border bg-background px-3 py-2"
              />
            </div>

            <div className="flex items-center justify-between">
              <div>
                <label className="font-medium">Support Threshold</label>
                <p className="text-sm text-muted-foreground">
                  Minimum support for entropy calculation
                </p>
              </div>
              <input
                type="number"
                min={1}
                max={100}
                value={settings.defaultSupportThreshold}
                onChange={(e) => updateSetting('defaultSupportThreshold', Number(e.target.value))}
                className="w-20 rounded-md border bg-background px-3 py-2"
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
                <p className="text-sm text-muted-foreground">
                  {documentsPath}/DiMA Desktop/Projects
                </p>
              </div>
              <Button 
                variant="outline" 
                onClick={() => revealInExplorer(documentsPath)}
              >
                Reveal in Explorer
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
                <label className="font-medium">Default Output Directory</label>
                <p className="text-sm text-muted-foreground">
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

            <div className="flex items-center justify-between">
              <div>
                <label className="font-medium">Default Chart Resolution</label>
                <p className="text-sm text-muted-foreground">
                  DPI for chart exports
                </p>
              </div>
              <select
                value={settings.defaultChartDpi}
                onChange={(e) => updateSetting('defaultChartDpi', Number(e.target.value) as 72 | 300)}
                className="rounded-md border bg-background px-3 py-2"
              >
                <option value={72}>Screen (72 DPI)</option>
                <option value={300}>Print (300 DPI)</option>
              </select>
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
            <Button variant="outline" onClick={resetToDefaults}>
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
    </div>
  );
}
