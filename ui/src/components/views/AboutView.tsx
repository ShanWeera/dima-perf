/**
 * DiMA Desktop - About View
 * 
 * Application information and developer console access.
 */

import { Dna, Terminal } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

export function AboutView() {
  const handleOpenDevTools = async () => {
    try {
      const webview = getCurrentWebviewWindow();
      // In development builds, this opens the Chrome DevTools
      // In production, devtools may not be available unless explicitly enabled
      const webviewAny = webview as unknown as { openDevtools?: () => Promise<void> };
      if (typeof webviewAny.openDevtools === 'function') {
        await webviewAny.openDevtools();
      } else {
        // Fallback: Show keyboard shortcut instructions
        console.log('DevTools: Press Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux)');
        alert('Developer Console: Press Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux) to open DevTools.');
      }
    } catch (error) {
      console.error('Failed to open DevTools:', error);
      alert('Developer Console: Press Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux) to open DevTools.');
    }
  };

  return (
    <div className="flex h-full flex-col items-center justify-center gap-8 p-8">
      <div className="flex flex-col items-center gap-4">
        <div className="rounded-full bg-primary/10 p-6">
          <Dna className="h-16 w-16 text-primary" />
        </div>
        <h1 className="text-3xl font-bold">DiMA Desktop</h1>
        <p className="text-lg text-muted-foreground">
          Diversity Motif Analyser
        </p>
        <p className="text-sm text-muted-foreground">Version 0.1.0</p>
      </div>

      <div className="max-w-md text-center text-sm text-muted-foreground">
        <p>
          A high-performance tool for analyzing protein and nucleotide sequence 
          diversity using k-mer based Shannon entropy analysis.
        </p>
      </div>

      <div className="mt-4">
        <Button variant="outline" onClick={handleOpenDevTools} className="gap-2">
          <Terminal className="h-4 w-4" />
          Developer Console
        </Button>
      </div>

      <div className="mt-8 text-center text-xs text-muted-foreground">
        <p>Built with Tauri, React, and Rust</p>
        <p className="mt-1">MIT License</p>
      </div>
    </div>
  );
}
