/**
 * DiMA Desktop - Position Explorer
 * 
 * Split view with chart on top and variant table below.
 */

import { useState, useMemo, useRef, useCallback } from 'react';
import { MessageSquare } from 'lucide-react';
import { useVirtualizer } from '@tanstack/react-virtual';
import type { Position, Variant, Annotation } from '@/lib/types';
import { ANNOTATION_COLORS } from '@/lib/types';
import { getCharacterColor, getMotifColor } from '@/lib/colors';
import { useSettingsStore } from '@/stores/settingsStore';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';

interface PositionExplorerProps {
  position: Position | null;
  alphabet: 'protein' | 'nucleotide';
  onVariantClick: (variant: Variant) => void;
  annotations?: Annotation[];
}

export function PositionExplorer({
  position,
  alphabet,
  onVariantClick,
  annotations = [],
}: PositionExplorerProps) {
  const { settings } = useSettingsStore();
  const [currentPage, setCurrentPage] = useState(0);
  const [pageSize, setPageSize] = useState(25);
  const [sortBy, setSortBy] = useState<'motif' | 'count' | 'incidence'>('motif');
  const [useVirtualization, setUseVirtualization] = useState(false);
  const parentRef = useRef<HTMLDivElement>(null);

  // Get annotations for current position
  const positionAnnotations = useMemo(() => {
    if (!position) return [];
    return annotations.filter((a) => a.positionNumber === position.position);
  }, [position, annotations]);

  // Sort variants
  const sortedVariants = useMemo(() => {
    if (!position?.diversity_motifs) return [];

    const motifOrder: Record<string, number> = { I: 0, Ma: 1, Mi: 2, U: 3 };

    return [...position.diversity_motifs].sort((a, b) => {
      if (sortBy === 'motif') {
        const orderA = motifOrder[a.motif_short || 'U'] ?? 4;
        const orderB = motifOrder[b.motif_short || 'U'] ?? 4;
        if (orderA !== orderB) return orderA - orderB;
        return b.count - a.count;
      } else if (sortBy === 'count') {
        return b.count - a.count;
      } else {
        return b.incidence - a.incidence;
      }
    });
  }, [position?.diversity_motifs, sortBy]);

  // Paginate (for paginated mode)
  const paginatedVariants = useMemo(() => {
    if (useVirtualization) return sortedVariants;
    const start = currentPage * pageSize;
    return sortedVariants.slice(start, start + pageSize);
  }, [sortedVariants, currentPage, pageSize, useVirtualization]);

  const totalPages = Math.ceil(sortedVariants.length / pageSize);

  // Virtual row renderer
  const rowVirtualizer = useVirtualizer({
    count: useVirtualization ? sortedVariants.length : paginatedVariants.length,
    getScrollElement: () => parentRef.current,
    estimateSize: useCallback(() => 44, []),
    overscan: 10,
  });

  if (!position) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        Select a position to view variants
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b px-4 py-2">
        <div>
          <div className="flex items-center gap-2">
            <h3 className="font-semibold">Position {position.position}</h3>
            {/* Annotation indicators */}
            {positionAnnotations.length > 0 && (
              <TooltipProvider>
                <div className="flex items-center gap-1">
                  {positionAnnotations.map((ann) => (
                    <Tooltip key={ann.id}>
                      <TooltipTrigger asChild>
                        <span
                          className="flex h-5 w-5 items-center justify-center rounded-full"
                          style={{ backgroundColor: ANNOTATION_COLORS[ann.color] }}
                        >
                          <MessageSquare className="h-3 w-3 text-white" />
                        </span>
                      </TooltipTrigger>
                      <TooltipContent side="top" className="max-w-xs">
                        <p className="font-medium">{ann.label || 'Annotation'}</p>
                        {ann.note && <p className="text-sm opacity-90">{ann.note}</p>}
                      </TooltipContent>
                    </Tooltip>
                  ))}
                </div>
              </TooltipProvider>
            )}
          </div>
          <p className="text-sm text-muted-foreground">
            Entropy: {position.entropy.toFixed(settings.decimalPrecision)} | 
            Variants: {position.distinct_variants_count}
          </p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <label className="text-sm">Sort by:</label>
            <select
              value={sortBy}
              onChange={(e) => setSortBy(e.target.value as 'motif' | 'count' | 'incidence')}
              className="rounded border bg-background px-2 py-1 text-sm"
            >
              <option value="motif">Motif Type</option>
              <option value="count">Count</option>
              <option value="incidence">Incidence</option>
            </select>
          </div>
          {sortedVariants.length > 100 && (
            <label className="flex items-center gap-2 text-xs text-muted-foreground">
              <input
                type="checkbox"
                checked={useVirtualization}
                onChange={(e) => setUseVirtualization(e.target.checked)}
                className="h-3 w-3"
              />
              Show all (virtual scroll)
            </label>
          )}
        </div>
      </div>

      {/* Variant Table */}
      <div ref={parentRef} className="flex-1 overflow-auto">
        <table className="w-full">
          <thead className="sticky top-0 z-10 bg-background">
            <tr className="border-b text-left text-sm text-muted-foreground">
              <th className="px-4 py-2">Sequence</th>
              <th className="px-4 py-2 text-right">Count</th>
              <th className="px-4 py-2 text-right">Incidence</th>
              <th className="px-4 py-2">Motif</th>
            </tr>
          </thead>
          <tbody>
            {useVirtualization ? (
              // Virtualized rendering
              <tr>
                <td colSpan={4} style={{ padding: 0 }}>
                  <div
                    style={{
                      height: `${rowVirtualizer.getTotalSize()}px`,
                      width: '100%',
                      position: 'relative',
                    }}
                  >
                    {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                      const variant = sortedVariants[virtualRow.index];
                      return (
                        <div
                          key={virtualRow.index}
                          onClick={() => onVariantClick(variant)}
                          className="absolute left-0 flex w-full cursor-pointer border-b transition-colors hover:bg-accent"
                          style={{
                            height: `${virtualRow.size}px`,
                            transform: `translateY(${virtualRow.start}px)`,
                          }}
                        >
                          <div className="flex-1 px-4 py-2">
                            <span className="font-mono text-sm">
                              {variant.sequence.split('').map((char, j) => (
                                <span
                                  key={j}
                                  style={{ color: getCharacterColor(char, alphabet) }}
                                >
                                  {char}
                                </span>
                              ))}
                            </span>
                          </div>
                          <div className="w-20 px-4 py-2 text-right font-mono text-sm">
                            {variant.count}
                          </div>
                          <div className="w-24 px-4 py-2 text-right font-mono text-sm">
                            {(variant.incidence * 100).toFixed(1)}%
                          </div>
                          <div className="w-32 px-4 py-2">
                            <span
                              className="inline-block rounded px-2 py-0.5 text-xs font-medium text-white"
                              style={{ backgroundColor: getMotifColor(variant.motif_short) }}
                            >
                              {variant.motif_long || variant.motif_short || 'Unknown'}
                            </span>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </td>
              </tr>
            ) : (
              // Paginated rendering
              paginatedVariants.map((variant, i) => (
                <tr
                  key={i}
                  onClick={() => onVariantClick(variant)}
                  className="cursor-pointer border-b transition-colors hover:bg-accent"
                >
                  <td className="px-4 py-2">
                    <span className="font-mono text-sm">
                      {variant.sequence.split('').map((char, j) => (
                        <span
                          key={j}
                          style={{ color: getCharacterColor(char, alphabet) }}
                        >
                          {char}
                        </span>
                      ))}
                    </span>
                  </td>
                  <td className="px-4 py-2 text-right font-mono text-sm">
                    {variant.count}
                  </td>
                  <td className="px-4 py-2 text-right font-mono text-sm">
                    {(variant.incidence * 100).toFixed(1)}%
                  </td>
                  <td className="px-4 py-2">
                    <span
                      className="inline-block rounded px-2 py-0.5 text-xs font-medium text-white"
                      style={{ backgroundColor: getMotifColor(variant.motif_short) }}
                    >
                      {variant.motif_long || variant.motif_short || 'Unknown'}
                    </span>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      {!useVirtualization && totalPages > 1 && (
        <div className="flex items-center justify-between border-t px-4 py-2">
          <div className="flex items-center gap-2 text-sm">
            <span>Page size:</span>
            <select
              value={pageSize}
              onChange={(e) => {
                setPageSize(Number(e.target.value));
                setCurrentPage(0);
              }}
              className="rounded border bg-background px-2 py-1"
            >
              <option value={10}>10</option>
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setCurrentPage((p) => Math.max(0, p - 1))}
              disabled={currentPage === 0}
              className="rounded px-2 py-1 hover:bg-muted disabled:opacity-50"
            >
              Previous
            </button>
            <span className="text-sm">
              {currentPage + 1} / {totalPages}
            </span>
            <button
              onClick={() => setCurrentPage((p) => Math.min(totalPages - 1, p + 1))}
              disabled={currentPage >= totalPages - 1}
              className="rounded px-2 py-1 hover:bg-muted disabled:opacity-50"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
