/**
 * UniProtStatusIndicator - Inline status display for UniProt feature loading.
 *
 * Shows a compact indicator in the PDB Viewer control bar:
 * - Loading spinner while fetching
 * - Green check + protein name on success (clickable for details popover)
 * - Amber info icon + "Enter manually" link on failure
 */

import { useState } from 'react';
import { Loader2, CheckCircle2, Info } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import type { UniProtInfo } from '@/lib/types';

interface UniProtStatusIndicatorProps {
  uniprotInfo: UniProtInfo | null;
  isLoading: boolean;
  error: string | null;
  onManualFetch: (accession: string) => void;
}

export function UniProtStatusIndicator({
  uniprotInfo,
  isLoading,
  error,
  onManualFetch,
}: UniProtStatusIndicatorProps) {
  const [manualAccession, setManualAccession] = useState('');
  const [showManualInput, setShowManualInput] = useState(false);

  // Loading state
  if (isLoading) {
    return (
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Loader2 className="h-3 w-3 animate-spin" />
        <span>Fetching features...</span>
      </div>
    );
  }

  // Success state: show protein info with popover for details
  if (uniprotInfo) {
    return (
      <Popover>
        <PopoverTrigger asChild>
          <button className="flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400 hover:underline">
            <CheckCircle2 className="h-3 w-3 shrink-0" />
            <span className="truncate max-w-[200px]">
              {uniprotInfo.accession} &ndash; {uniprotInfo.protein_name}
            </span>
          </button>
        </PopoverTrigger>
        <PopoverContent className="w-72 text-xs" align="start">
          <div className="space-y-2">
            <div>
              <span className="font-medium">Accession:</span>{' '}
              {uniprotInfo.accession}
            </div>
            <div>
              <span className="font-medium">Protein:</span>{' '}
              {uniprotInfo.protein_name}
            </div>
            <div>
              <span className="font-medium">Organism:</span>{' '}
              {uniprotInfo.organism}
            </div>
            <div>
              <span className="font-medium">Length:</span>{' '}
              {uniprotInfo.sequence_length} residues
            </div>
            <div>
              <span className="font-medium">Features:</span>{' '}
              {uniprotInfo.features.length} annotations loaded
            </div>
            <Button
              variant="outline"
              size="sm"
              className="w-full mt-2 h-7 text-xs"
              onClick={() => setShowManualInput(true)}
            >
              Change accession
            </Button>
            {showManualInput && (
              <div className="flex gap-1.5 mt-1">
                <Input
                  value={manualAccession}
                  onChange={(e) => setManualAccession(e.target.value.toUpperCase())}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && manualAccession.trim()) {
                      onManualFetch(manualAccession.trim());
                      setShowManualInput(false);
                      setManualAccession('');
                    }
                  }}
                  placeholder="e.g. P0DTC2"
                  className="h-7 text-xs"
                />
                <Button
                  size="sm"
                  className="h-7 text-xs shrink-0"
                  disabled={!manualAccession.trim()}
                  onClick={() => {
                    onManualFetch(manualAccession.trim());
                    setShowManualInput(false);
                    setManualAccession('');
                  }}
                >
                  Fetch
                </Button>
              </div>
            )}
          </div>
        </PopoverContent>
      </Popover>
    );
  }

  // Error / no data state: show message + manual input
  return (
    <div className="flex items-center gap-1.5 text-xs">
      {error ? (
        <>
          <Info className="h-3 w-3 text-amber-500 shrink-0" />
          <span className="text-muted-foreground truncate max-w-[160px]">
            {error.includes('No UniProt') ? 'No UniProt mapping found' : 'Feature fetch failed'}
          </span>
        </>
      ) : null}
      {!showManualInput ? (
        <button
          onClick={() => setShowManualInput(true)}
          className="text-primary hover:underline shrink-0"
        >
          Enter manually
        </button>
      ) : (
        <div className="flex gap-1.5">
          <Input
            value={manualAccession}
            onChange={(e) => setManualAccession(e.target.value.toUpperCase())}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && manualAccession.trim()) {
                onManualFetch(manualAccession.trim());
                setShowManualInput(false);
                setManualAccession('');
              }
            }}
            placeholder="UniProt accession"
            className="h-7 w-28 text-xs"
          />
          <Button
            size="sm"
            className="h-7 text-xs shrink-0"
            disabled={!manualAccession.trim()}
            onClick={() => {
              onManualFetch(manualAccession.trim());
              setShowManualInput(false);
              setManualAccession('');
            }}
          >
            Fetch
          </Button>
        </div>
      )}
    </div>
  );
}
