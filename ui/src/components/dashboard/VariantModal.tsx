/**
 * DiMA Desktop - Variant Detail Modal
 * 
 * Shows detailed information about a selected variant using shadcn Dialog
 * for proper focus trapping, Escape key handling, and accessibility.
 */

import { Copy, Check } from 'lucide-react';
import { useState, useRef, useEffect } from 'react';
import type { Variant } from '@/lib/types';
import { getCharacterColor, getMotifColor } from '@/lib/colors';
import { showErrorToast } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from '@/components/ui/dialog';

interface VariantModalProps {
  variant: Variant;
  alphabet: 'protein' | 'nucleotide';
  onClose: () => void;
}

export function VariantModal({ variant, alphabet, onClose }: VariantModalProps) {
  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout>>();
  useEffect(() => () => { clearTimeout(copyTimerRef.current); }, []);

  const handleCopy = async () => {
    const seq = variant.sequence ?? '';
    const fastaContent = `>variant_${seq}\n${seq}`;
    try {
      await navigator.clipboard.writeText(fastaContent);
      setCopied(true);
      clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      showErrorToast('Failed to copy to clipboard', err);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="max-h-[90vh] max-w-2xl overflow-hidden p-0">
        <DialogHeader className="border-b px-6 py-4">
          <DialogTitle>Variant Details</DialogTitle>
          <DialogDescription className="sr-only">
            Detailed information about the selected k-mer variant
          </DialogDescription>
        </DialogHeader>

        <div className="overflow-auto px-6 py-4">
          {/* Colored Sequence */}
          <div className="mb-6">
            <h3 className="mb-2 text-sm font-medium text-muted-foreground">Sequence</h3>
            <div className="rounded-lg bg-muted p-4 font-mono text-lg leading-relaxed overflow-x-auto break-all">
              {(variant.sequence ?? '').split('').map((char, i) => (
                <span
                  key={i}
                  style={{ color: getCharacterColor(char, alphabet) }}
                >
                  {char}
                </span>
              ))}
            </div>
          </div>

          {/* Statistics */}
          <div className="mb-6 grid grid-cols-3 gap-4">
            <div className="rounded-lg border p-4">
              <p className="text-sm text-muted-foreground">Count</p>
              <p className="text-2xl font-bold">{variant.count}</p>
            </div>
            <div className="rounded-lg border p-4">
              <p className="text-sm text-muted-foreground">Incidence</p>
              <p className="text-2xl font-bold">{Number.isFinite(variant.incidence) ? variant.incidence.toFixed(1) : '0.0'}%</p>
            </div>
            <div className="rounded-lg border p-4">
              <p className="text-sm text-muted-foreground">Motif Type</p>
              <span
                className="inline-block rounded px-3 py-1 text-lg font-medium text-white"
                style={{ backgroundColor: getMotifColor(variant.motif_short) }}
              >
                {variant.motif_long || variant.motif_short || 'Unknown'}
              </span>
            </div>
          </div>

          {/* Metadata */}
          {variant.metadata && Object.keys(variant.metadata).length > 0 && (
            <div className="mb-6">
              <h3 className="mb-3 text-sm font-medium text-muted-foreground">Metadata</h3>
              <div className="space-y-4">
                {Object.entries(variant.metadata).map(([field, values]) => (
                  <div key={field} className="rounded-lg border p-4">
                    <h4 className="mb-2 font-medium capitalize">{field}</h4>
                    <div className="flex flex-wrap gap-2">
                      {Object.entries(values).map(([value, count]) => (
                        <span
                          key={value}
                          className="inline-flex items-center gap-1 rounded-full bg-muted px-3 py-1 text-sm"
                        >
                          <span>{value || '(empty)'}</span>
                          <span className="font-medium text-muted-foreground">
                            ({count})
                          </span>
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        <DialogFooter className="border-t px-6 py-4">
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
          <Button onClick={handleCopy} className="gap-2">
            {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            {copied ? 'Copied!' : 'Copy Sequence'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
