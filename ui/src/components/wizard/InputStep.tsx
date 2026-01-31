/**
 * DiMA Desktop - Input Step
 * 
 * First wizard step for selecting and validating FASTA file input.
 */

import { useCallback, useState } from 'react';
import { Upload, FileText, CheckCircle2, XCircle, AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { useAppStore } from '@/stores/appStore';
import { open } from '@tauri-apps/plugin-dialog';
import { formatBytes } from '@/lib/utils';
import { cn } from '@/lib/utils';

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
  } = useProjectStore();
  const { setCurrentView } = useAppStore();
  
  const [isDragging, setIsDragging] = useState(false);
  const [isValidating, setIsValidating] = useState(false);

  const handleFileSelect = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: 'FASTA', extensions: ['fasta', 'fa', 'fna', 'faa', 'txt'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      
      if (selected && typeof selected === 'string') {
        setIsValidating(true);
        await setInputFile(selected);
        setIsValidating(false);
      }
    } catch (error) {
      console.error('Failed to select file:', error);
      setIsValidating(false);
    }
  };

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    
    const files = e.dataTransfer.files;
    if (files.length > 0) {
      // In Tauri, we need to get the path differently
      // For now, prompt user to use file dialog
      handleFileSelect();
    }
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

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
            <h1 className="text-xl font-semibold">{currentProject?.name}</h1>
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
          {/* Drop Zone */}
          <div
            onClick={handleFileSelect}
            onDrop={handleDrop}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
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
                Supports .fasta, .fa, .fna, .faa files
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
                    {inputFilePath.split('/').pop()}
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
                      <p className="font-medium">{new Date(fastaValidation.file_modified_at).toLocaleDateString()}</p>
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
                className="h-4 w-4 rounded border-gray-300"
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
