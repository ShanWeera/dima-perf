/**
 * DiMA Desktop - Input Step
 * 
 * First wizard step for selecting and validating FASTA file input.
 */

import { useState, useEffect } from 'react';
import { Upload, FileText, CheckCircle2, XCircle, AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { useAppStore } from '@/stores/appStore';
import { useShallow } from 'zustand/react/shallow';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { formatBytes, cn, showErrorToast } from '@/lib/utils';
import { useToastStore } from '@/stores/toastStore';

const FASTA_EXTENSIONS = ['fasta', 'fa', 'fna', 'faa', 'txt'];

export function InputStep() {
  const { 
    currentProject,
    inputFilePath, 
    setInputFile, 
    fastaValidation,
    copyInputFile,
    setCopyInputFile,
    goNext,
    closeProject,
  } = useProjectStore(useShallow((s) => ({
    currentProject: s.currentProject,
    inputFilePath: s.inputFilePath,
    setInputFile: s.setInputFile,
    fastaValidation: s.fastaValidation,
    copyInputFile: s.copyInputFile,
    setCopyInputFile: s.setCopyInputFile,
    goNext: s.goNext,
    closeProject: s.closeProject,
  })));
  const { setCurrentView } = useAppStore();
  
  const [isDragging, setIsDragging] = useState(false);
  const [isValidating, setIsValidating] = useState(false);

  // Listen for native Tauri drag-drop events (HTML5 DnD is disabled by Tauri by default).
  // Uses an abort flag to handle the case where the component unmounts before the
  // async listener registration resolves — prevents leaked listeners. (Fix 4.27)
  useEffect(() => {
    const appWindow = getCurrentWebviewWindow();
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    appWindow.onDragDropEvent((event) => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        setIsDragging(true);
      } else if (event.payload.type === 'leave') {
        setIsDragging(false);
      } else if (event.payload.type === 'drop') {
        setIsDragging(false);
        const fastaFile = event.payload.paths.find((p) => {
          const ext = p.split('.').pop()?.toLowerCase() ?? '';
          return FASTA_EXTENSIONS.includes(ext);
        });
        if (fastaFile) {
          // Match the file dialog pattern: show loading state + handle errors. (Fix 5.43)
          setIsValidating(true);
          setInputFile(fastaFile)
            .catch((err) => showErrorToast('Dropped file validation failed', err))
            .finally(() => setIsValidating(false));
        } else if (event.payload.paths.length > 0) {
          // User dropped a file with an unsupported extension (Fix 9.4.5)
          useToastStore.getState().addToast(
            'Unsupported file type. Please use .fasta, .fa, .fna, .faa, or .txt files.',
            'warning',
          );
        }
      }
    }).then((fn) => {
      if (cancelled) {
        // Component unmounted before the listener registered — immediately clean up
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [setInputFile]);

  const handleFileSelect = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: 'FASTA', extensions: FASTA_EXTENSIONS },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      
      if (selected && typeof selected === 'string') {
        setIsValidating(true);
        await setInputFile(selected);
        setIsValidating(false);
      }
    } catch (error) {
      showErrorToast('Failed to select file', error);
      setIsValidating(false);
    }
  };


  const handleBack = () => {
    closeProject();
    setCurrentView('projects');
  };

  const isValid = fastaValidation?.is_valid === true;

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-b px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl font-semibold truncate min-w-0">{currentProject?.name}</h1>
            <p className="text-sm text-muted-foreground">Step 1 of 3: Select Input File</p>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={handleBack}>
              Cancel
            </Button>
            <Button onClick={goNext} disabled={!isValid}>
              Next
            </Button>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        <div className="mx-auto max-w-2xl space-y-6">
          {/* Drop Zone — accessible via keyboard (Enter/Space). (Fix 6.7) */}
          <div
            role="button"
            tabIndex={0}
            onClick={handleFileSelect}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                handleFileSelect();
              }
            }}
            className={cn(
              "flex cursor-pointer flex-col items-center justify-center gap-4 rounded-lg border-2 border-dashed p-12 transition-colors",
              isDragging && "border-primary bg-primary/5",
              !isDragging && "border-muted-foreground/25 hover:border-muted-foreground/50"
            )}
          >
            <Upload className="h-12 w-12 text-muted-foreground" />
            <div className="text-center">
              <p className="font-medium">Drop FASTA file here or click to browse</p>
              <p className="text-sm text-muted-foreground">
                Supports .fasta, .fa, .fna, .faa, .txt files
              </p>
            </div>
          </div>

          {/* Selected File */}
          {inputFilePath && (
            <div className="rounded-lg border p-4">
              <div className="flex items-start gap-3">
                <FileText className="h-5 w-5 text-muted-foreground" />
                <div className="flex-1 min-w-0">
                  <p className="truncate font-medium">
                    {inputFilePath.split(/[\\/]/).pop()}
                  </p>
                  <p className="truncate text-sm text-muted-foreground">
                    {inputFilePath}
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* Validation Results */}
          {isValidating && (
            <div className="flex items-center gap-2 text-muted-foreground">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
              <span>Validating file...</span>
            </div>
          )}

          {fastaValidation && !isValidating && (
            <div className="space-y-4">
              {/* Validation Status */}
              <div className={cn(
                "flex items-center gap-2 rounded-lg p-3",
                fastaValidation.is_valid ? "bg-green-500/10 text-green-600 dark:text-green-400" : "bg-red-500/10 text-red-600 dark:text-red-400"
              )}>
                {fastaValidation.is_valid ? (
                  <CheckCircle2 className="h-5 w-5" />
                ) : (
                  <XCircle className="h-5 w-5" />
                )}
                <span className="font-medium">
                  {fastaValidation.is_valid ? 'File is valid' : 'Validation failed'}
                </span>
              </div>

              {/* File Info */}
              {fastaValidation.is_valid && (
                <div className="grid grid-cols-2 gap-4 text-sm">
                  <div>
                    <p className="text-muted-foreground">Sequences</p>
                    <p className="font-medium">{fastaValidation.sequence_count.toLocaleString()}</p>
                  </div>
                  <div>
                    <p className="text-muted-foreground">Sequence Length</p>
                    <p className="font-medium">{fastaValidation.sequence_length?.toLocaleString() || 'Variable'}</p>
                  </div>
                  <div>
                    <p className="text-muted-foreground">File Size</p>
                    <p className="font-medium">{formatBytes(fastaValidation.file_size_bytes)}</p>
                  </div>
                  <div>
                    <p className="text-muted-foreground">Detected Type</p>
                    <p className="font-medium capitalize">{fastaValidation.detected_alphabet}</p>
                  </div>
                  {fastaValidation.file_modified_at && (
                    <div>
                      <p className="text-muted-foreground">Modified</p>
                      <p className="font-medium">{new Date(Number(fastaValidation.file_modified_at) * 1000).toLocaleDateString()}</p>
                    </div>
                  )}
                </div>
              )}

              {/* Sample Headers */}
              {fastaValidation.sample_headers.length > 0 && (
                <div>
                  <p className="mb-2 text-sm font-medium">Sample Headers</p>
                  <div className="space-y-1 rounded-lg bg-muted p-3 font-mono text-xs">
                    {fastaValidation.sample_headers.map((header, i) => (
                      <p key={i} className="truncate">{'>'}{header}</p>
                    ))}
                  </div>
                </div>
              )}

              {/* Errors */}
              {fastaValidation.errors.length > 0 && (
                <div className="space-y-2">
                  {fastaValidation.errors.map((error, i) => (
                    <div key={i} className="flex items-start gap-2 text-sm text-red-600 dark:text-red-400">
                      <AlertCircle className="h-4 w-4 mt-0.5 shrink-0" />
                      <span>
                        {error.line_number !== null && error.line_number > 0 && (
                          <span className="font-medium">Line {error.line_number}: </span>
                        )}
                        {error.message}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Copy File Option */}
          {inputFilePath && fastaValidation?.is_valid && (
            <label className="flex items-center gap-3 rounded-lg border p-4 cursor-pointer hover:bg-accent">
              <input
                type="checkbox"
                checked={copyInputFile}
                onChange={(e) => setCopyInputFile(e.target.checked)}
                className="h-4 w-4 rounded border-input"
              />
              <div>
                <p className="font-medium">Copy file to project folder</p>
                <p className="text-sm text-muted-foreground">
                  Makes the project self-contained and portable
                </p>
              </div>
            </label>
          )}
        </div>
      </div>
    </div>
  );
}
