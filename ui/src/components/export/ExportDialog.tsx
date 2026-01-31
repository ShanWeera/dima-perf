/**
 * DiMA Desktop - Export Dialog
 * 
 * Modal for exporting results and charts.
 */

import { useState } from 'react';
import { X, Download, FileJson, FileImage, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { save } from '@tauri-apps/plugin-dialog';
import { exportResults, exportChart } from '@/lib/tauri';

interface ExportDialogProps {
  projectPath: string;
  projectName: string;
  chartDataUrl?: string;
  chartType?: string;
  onClose: () => void;
}

type ExportType = 'json' | 'dima' | 'chart';
type ChartFormat = 'png' | 'svg';
type ChartDpi = 72 | 300;

export function ExportDialog({
  projectPath,
  projectName,
  chartDataUrl,
  chartType,
  onClose,
}: ExportDialogProps) {
  const [exportType, setExportType] = useState<ExportType>(chartDataUrl ? 'chart' : 'json');
  const [chartFormat, setChartFormat] = useState<ChartFormat>('png');
  const [chartDpi, setChartDpi] = useState<ChartDpi>(72);
  const [chartTitle, setChartTitle] = useState<string>('');
  const [isExporting, setIsExporting] = useState(false);
  const [success, setSuccess] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleExport = async () => {
    setIsExporting(true);
    setError(null);

    try {
      let filePath: string | null = null;

      if (exportType === 'json') {
        filePath = await save({
          defaultPath: `${projectName}.json`,
          filters: [{ name: 'JSON', extensions: ['json'] }],
        });
        if (filePath) {
          await exportResults({
            project_path: projectPath,
            output_path: filePath,
            format: 'json',
          });
        }
      } else if (exportType === 'dima') {
        filePath = await save({
          defaultPath: `${projectName}.dima`,
          filters: [{ name: 'DiMA Binary', extensions: ['dima'] }],
        });
        if (filePath) {
          await exportResults({
            project_path: projectPath,
            output_path: filePath,
            format: 'dima',
            compression: 1,
          });
        }
      } else if (exportType === 'chart' && chartDataUrl) {
        const ext = chartFormat;
        filePath = await save({
          defaultPath: `${projectName}_${chartType || 'chart'}.${ext}`,
          filters: [{ name: chartFormat.toUpperCase(), extensions: [ext] }],
        });
        if (filePath) {
          await exportChart({
            data_url: chartDataUrl,
            output_path: filePath,
            format: chartFormat,
            title: chartTitle || undefined,
          });
        }
      }

      if (filePath) {
        setSuccess(true);
        setTimeout(() => {
          onClose();
        }, 1500);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg bg-background shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b px-6 py-4">
          <h2 className="text-lg font-semibold">Export</h2>
          <button onClick={onClose} className="rounded-md p-2 hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6">
          {success ? (
            <div className="flex flex-col items-center gap-4 py-8">
              <div className="rounded-full bg-green-500/10 p-4">
                <Check className="h-12 w-12 text-green-600" />
              </div>
              <p className="text-lg font-medium">Export successful!</p>
            </div>
          ) : (
            <div className="space-y-6">
              {/* Export Type Selection */}
              <div className="space-y-2">
                <label className="text-sm font-medium">Export Type</label>
                <div className="grid grid-cols-3 gap-2">
                  <button
                    onClick={() => setExportType('json')}
                    className={`flex flex-col items-center gap-2 rounded-lg border p-4 transition-colors ${
                      exportType === 'json'
                        ? 'border-primary bg-primary/5'
                        : 'hover:bg-muted'
                    }`}
                  >
                    <FileJson className="h-8 w-8" />
                    <span className="text-sm font-medium">JSON</span>
                  </button>
                  <button
                    onClick={() => setExportType('dima')}
                    className={`flex flex-col items-center gap-2 rounded-lg border p-4 transition-colors ${
                      exportType === 'dima'
                        ? 'border-primary bg-primary/5'
                        : 'hover:bg-muted'
                    }`}
                  >
                    <Download className="h-8 w-8" />
                    <span className="text-sm font-medium">Binary</span>
                  </button>
                  {chartDataUrl && (
                    <button
                      onClick={() => setExportType('chart')}
                      className={`flex flex-col items-center gap-2 rounded-lg border p-4 transition-colors ${
                        exportType === 'chart'
                          ? 'border-primary bg-primary/5'
                          : 'hover:bg-muted'
                      }`}
                    >
                      <FileImage className="h-8 w-8" />
                      <span className="text-sm font-medium">Chart</span>
                    </button>
                  )}
                </div>
              </div>

              {/* Chart Options */}
              {exportType === 'chart' && (
                <div className="space-y-4">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Chart Title (optional)</label>
                    <input
                      type="text"
                      value={chartTitle}
                      onChange={(e) => setChartTitle(e.target.value)}
                      placeholder="Enter custom title for export..."
                      className="w-full rounded-md border bg-background px-3 py-2 text-sm"
                    />
                    <p className="text-xs text-muted-foreground">
                      This title will appear on the exported image only
                    </p>
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">Format</label>
                    <div className="flex gap-2">
                      <button
                        onClick={() => setChartFormat('png')}
                        className={`rounded-md px-4 py-2 text-sm ${
                          chartFormat === 'png'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted'
                        }`}
                      >
                        PNG
                      </button>
                      <button
                        onClick={() => setChartFormat('svg')}
                        className={`rounded-md px-4 py-2 text-sm ${
                          chartFormat === 'svg'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted'
                        }`}
                      >
                        SVG
                      </button>
                    </div>
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">Resolution</label>
                    <div className="flex gap-2">
                      <button
                        onClick={() => setChartDpi(72)}
                        className={`rounded-md px-4 py-2 text-sm ${
                          chartDpi === 72
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted'
                        }`}
                      >
                        Screen (72 DPI)
                      </button>
                      <button
                        onClick={() => setChartDpi(300)}
                        className={`rounded-md px-4 py-2 text-sm ${
                          chartDpi === 300
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted'
                        }`}
                      >
                        Print (300 DPI)
                      </button>
                    </div>
                  </div>
                </div>
              )}

              {/* Error Message */}
              {error && (
                <div className="rounded-lg bg-destructive/10 p-3 text-sm text-destructive">
                  {error}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        {!success && (
          <div className="flex justify-end gap-2 border-t px-6 py-4">
            <Button variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button onClick={handleExport} disabled={isExporting} className="gap-2">
              {isExporting ? (
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
              ) : (
                <Download className="h-4 w-4" />
              )}
              Export
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
