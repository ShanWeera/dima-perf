/**
 * DiMA Desktop - Variant Detail Modal
 * 
 * Shows detailed information about a selected variant.
 */

import { X, Copy, Check } from 'lucide-react';
import { useState } from 'react';
import type { Variant } from '@/lib/types';
import { getCharacterColor, getMotifColor } from '@/lib/colors';
import { Button } from '@/components/ui/button';

interface VariantModalProps {
  variant: Variant;
  alphabet: 'protein' | 'nucleotide';
  onClose: () => void;
}

export function VariantModal({ variant, alphabet, onClose }: VariantModalProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    const fastaContent = `>variant_${variant.sequence}\n${variant.sequence}`;
    await navigator.clipboard.writeText(fastaContent);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="max-h-[90vh] w-full max-w-2xl overflow-hidden rounded-lg bg-background shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b px-6 py-4">
          <h2 className="text-lg font-semibold">Variant Details</h2>
          <button onClick={onClose} className="rounded-md p-2 hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="overflow-auto p-6">
          {/* Colored Sequence */}
          <div className="mb-6">
            <h3 className="mb-2 text-sm font-medium text-muted-foreground">Sequence</h3>
            <div className="rounded-lg bg-muted p-4 font-mono text-lg leading-relaxed">
              {variant.sequence.split('').map((char, i) => (
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
              <p className="text-2xl font-bold">{(variant.incidence * 100).toFixed(1)}%</p>
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

        {/* Footer */}
        <div className="flex justify-end gap-2 border-t px-6 py-4">
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
          <Button onClick={handleCopy} className="gap-2">
            {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            {copied ? 'Copied!' : 'Copy Sequence'}
          </Button>
        </div>
      </div>
    </div>
  );
}
