/**
 * DiMA Desktop - About View
 * 
 * Application information, links, keyboard shortcuts reference, and developer console access.
 */

import { useState, useEffect } from 'react';
import { Dna, Terminal, ExternalLink, Keyboard } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getVersion } from '@tauri-apps/api/app';
import { message } from '@tauri-apps/plugin-dialog';
import { SHORTCUTS } from '@/lib/keyboard-shortcuts';

export function AboutView() {
  const [appVersion, setAppVersion] = useState('');

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion('0.1.0'));
  }, []);

  const handleOpenDevTools = async () => {
    try {
      const webview = getCurrentWebviewWindow();
      const webviewAny = webview as unknown as { openDevtools?: () => Promise<void> };
      if (typeof webviewAny.openDevtools === 'function') {
        await webviewAny.openDevtools();
      } else {
        await message(
          'Press Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux) to open DevTools.',
          { title: 'Developer Console', kind: 'info' }
        );
      }
    } catch (error) {
      console.error('Failed to open DevTools:', error);
      await message(
        'Press Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux) to open DevTools.',
        { title: 'Developer Console', kind: 'info' }
      );
    }
  };

  return (
    <div className="flex h-full flex-col items-center overflow-auto py-12 px-8">
      {/* Header */}
      <div className="flex flex-col items-center gap-4">
        <div className="rounded-full bg-primary/10 p-6">
          <Dna className="h-16 w-16 text-primary" />
        </div>
        <h1 className="text-3xl font-bold">DiMA Desktop</h1>
        <p className="text-lg text-muted-foreground">
          Diversity Motif Analyser
        </p>
        <p className="text-sm text-muted-foreground">Version {appVersion || '...'}</p>
      </div>

      {/* Description */}
      <div className="mt-6 max-w-md text-center text-sm text-muted-foreground">
        <p>
          A high-performance tool for analyzing protein and nucleotide sequence 
          diversity using k-mer based Shannon entropy analysis with rarefaction-based
          bias correction.
        </p>
      </div>

      {/* Links */}
      <div className="mt-6 flex flex-wrap justify-center gap-3">
        <Button variant="outline" size="sm" asChild className="gap-2">
          <a href="https://github.com/AliYmworwormed/dima" target="_blank" rel="noopener noreferrer">
            <ExternalLink className="h-3.5 w-3.5" />
            GitHub
          </a>
        </Button>
        <Button variant="outline" size="sm" asChild className="gap-2">
          <a href="https://pubmed.ncbi.nlm.nih.gov/39796368/" target="_blank" rel="noopener noreferrer">
            <ExternalLink className="h-3.5 w-3.5" />
            Publication (PMC11596295)
          </a>
        </Button>
        <Button variant="outline" size="sm" onClick={handleOpenDevTools} className="gap-2">
          <Terminal className="h-3.5 w-3.5" />
          Developer Console
        </Button>
      </div>

      {/* Keyboard Shortcuts */}
      <div className="mt-8 w-full max-w-sm">
        <div className="flex items-center gap-2 mb-3">
          <Keyboard className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Keyboard Shortcuts</h2>
        </div>
        <div className="rounded-lg border bg-card p-3">
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
            {Object.values(SHORTCUTS).map((shortcut) => (
              <div key={shortcut.key + (shortcut.mod ? 'm' : '') + (shortcut.shift ? 's' : '')} className="contents">
                <dt>
                  <kbd className="rounded border bg-muted px-1.5 py-0.5 text-xs font-mono">
                    {shortcut.display}
                  </kbd>
                </dt>
                <dd className="text-muted-foreground">{shortcut.description}</dd>
              </div>
            ))}
            <div className="contents">
              <dt>
                <kbd className="rounded border bg-muted px-1.5 py-0.5 text-xs font-mono">
                  ←/→
                </kbd>
              </dt>
              <dd className="text-muted-foreground">Navigate positions</dd>
            </div>
          </dl>
        </div>
      </div>

      {/* Footer */}
      <div className="mt-8 text-center text-xs text-muted-foreground">
        <p>Built with Tauri, React, and Rust</p>
        <p className="mt-1">MIT License</p>
      </div>
    </div>
  );
}
