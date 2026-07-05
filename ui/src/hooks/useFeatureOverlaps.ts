/**
 * useFeatureOverlaps - Computes HCS ↔ feature overlap mappings.
 *
 * For each HCS region, returns which mapped features overlap with it.
 * Uses useMemo with early-exit when inputs are empty (O(1) fast path).
 *
 * kmerLength is required to correctly compute the actual alignment columns
 * covered by each HCS region (window start + k-1 trailing residues).
 */

import { useMemo } from 'react';
import type { MappedFeature } from '@/lib/types';
import type { HCSRegionSimple } from '@/lib/hcs';
import { computeFeatureOverlaps } from '@/lib/features';

export function useFeatureOverlaps(
  hcsRegions: HCSRegionSimple[],
  mappedFeatures: MappedFeature[],
  kmerLength: number = 1
): Map<number, MappedFeature[]> {
  return useMemo(
    () => computeFeatureOverlaps(hcsRegions, mappedFeatures, kmerLength),
    [hcsRegions, mappedFeatures, kmerLength]
  );
}
