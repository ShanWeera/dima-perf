/**
 * DiMA Desktop - Position Explorer
 * 
 * Split view with chart on top and variant table below.
 */

import { useState, useMemo, useRef, useCallback, useEffect, memo } from 'react';
import { MessageSquare } from 'lucide-react';
import { useVirtualizer } from '@tanstack/react-virtual';
import type { Position, Variant, Annotation } from '@/lib/types';
import { ANNOTATION_COLORS } from '@/lib/types';
import { getCharacterColor, getMotifColor } from '@/lib/colors';
import { useSettingsStore } from '@/stores/settingsStore';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

interface PositionExplorerProps {
  position: Position | null;
  alphabet: 'protein' | 'nucleotide';
  onVariantClick: (variant: Variant) => void;
  annotations?: Annotation[];
}

export const PositionExplorer = memo(function PositionExplorer({
  position,
  alphabet,
  onVariantClick,
  annotations = [],
}: PositionExplorerProps) {
  const decimalPrecision = useSettingsStore((s) => s.settings.decimalPrecision);
  const [currentPage, setCurrentPage] = useState(0);
  const [pageSize, setPageSize] = useState(25);
  const [sortBy, setSortBy] = useState<'motif' | 'count' | 'incidence'>('motif');
  const [useVirtualization, setUseVirtualization] = useState(false);
  const parentRef = useRef<HTMLDivElement>(null);

  // Reset pagination when position changes (Fix 6.3)
  useEffect(() => {
    setCurrentPage(0);
  }, [position?.position]);

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
    <div className="flex h-full min-h-0 flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b px-4 py-2">
        <div>
          <div className="flex items-center gap-2">
            <h3 className="font-semibold">Position {position.position}</h3>
            {position.low_support && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className={`rounded px-1.5 py-0.5 text-xs font-medium ${
                    position.low_support === 'NS' ? 'bg-destructive/15 text-destructive' :
                    position.low_support === 'LS' ? 'bg-yellow-500/15 text-yellow-700 dark:text-yellow-400' :
                    'bg-blue-500/15 text-blue-700 dark:text-blue-400'
                  }`}>
                    {position.low_support}
                  </span>
                </TooltipTrigger>
                <TooltipContent side="top">
                  {position.low_support === 'NS' ? 'No Support — zero sequences at this position' :
                   position.low_support === 'LS' ? 'Low Support — below the threshold; entropy estimated via rarefaction' :
                   'Exactly at Low Support threshold — entropy estimated via rarefaction'}
                </TooltipContent>
              </Tooltip>
            )}
            {positionAnnotations.length > 0 && (
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
            )}
          </div>
          <p className="text-sm text-muted-foreground">
            Entropy: {Number.isFinite(position.entropy) ? position.entropy.toFixed(decimalPrecision) : 'N/A'} | 
            Variants: {position.distinct_variants_count ?? 0}
          </p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <label htmlFor="variant-sort" className="text-sm">Sort by:</label>
            <select
              id="variant-sort"
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
              <th scope="col" className="px-4 py-2">Sequence</th>
              <th scope="col" className="px-4 py-2 text-right">Count</th>
              <th scope="col" className="px-4 py-2 text-right">Incidence</th>
              <th scope="col" className="px-4 py-2">Motif</th>
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
                      if (!variant) return null;
                      return (
                        <div
                          key={virtualRow.index}
                          role="row"
                          tabIndex={0}
                          onClick={() => onVariantClick(variant)}
                          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onVariantClick(variant); } }}
                          className="absolute left-0 flex w-full cursor-pointer border-b transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                          style={{
                            height: `${virtualRow.size}px`,
                            transform: `translateY(${virtualRow.start}px)`,
                          }}
                        >
                          <div className="flex-1 px-4 py-2">
                            <span className="font-mono text-sm">
                              {(variant.sequence ?? '').split('').map((char, j) => (
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
                            {Number.isFinite(variant.incidence) ? variant.incidence.toFixed(1) : '0.0'}%
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
              paginatedVariants.map((variant) => (
                <tr
                  key={`${variant.sequence}-${variant.motif_short ?? 'x'}`}
                  tabIndex={0}
                  onClick={() => onVariantClick(variant)}
                  onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onVariantClick(variant); } }}
                  className="cursor-pointer border-b transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <td className="px-4 py-2">
                    <span className="font-mono text-sm">
                      {(variant.sequence ?? '').split('').map((char, j) => (
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
                    {Number.isFinite(variant.incidence) ? variant.incidence.toFixed(1) : '0.0'}%
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
            <label htmlFor="page-size-select">Page size:</label>
            <select
              id="page-size-select"
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
              aria-label="Previous page"
            >
              Previous
            </button>
            <span className="text-sm" aria-live="polite">
              {currentPage + 1} / {totalPages}
            </span>
            <button
              onClick={() => setCurrentPage((p) => Math.min(totalPages - 1, p + 1))}
              disabled={currentPage >= totalPages - 1}
              className="rounded px-2 py-1 hover:bg-muted disabled:opacity-50"
              aria-label="Next page"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
});
