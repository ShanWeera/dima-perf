/**
 * DiMA Desktop - HCS (Highly Conserved Sequences) Utilities
 *
 * Shared computation and types for HCS region detection.
 * Used by HCSMap, DashboardGrid, PDBViewer, and FeatureTrack components.
 *
 * Algorithm per PMC11596295:
 * 1. Walk positions in order; at each position select the lexicographically
 *    first Index variant above the threshold (determinism on ties).
 * 2. A position without a qualifying Index variant breaks the current HCS region.
 * 3. Adjacent k-mers are stitched by their prefix-suffix overlap: the non-overlapping
 *    suffix is appended. If no overlap exists, the current region ends and a new one begins.
 * 4. Single-position Index k-mers ARE valid HCS regions (length = k).
 */

import type { Position } from './types';

/** Minimal HCS region with just positional bounds */
export interface HCSRegionSimple {
  startPosition: number;
  endPosition: number;
}

/** Full HCS region with stitched sequence and position indices */
export interface HCSRegion extends HCSRegionSimple {
  sequence: string;
  indices: number[];
  /** Positions within this region that have low support (NS or LS) */
  lowSupportPositions: number[];
}

/**
 * Find the length of the prefix-suffix overlap between the end of `acc` and the start of `kmer`.
 * For k-mers of length k at adjacent sliding-window positions, the overlap is typically k-1.
 * Returns 0 if no valid overlap is found (indicating non-contiguous sequences).
 */
function findOverlapLength(acc: string, kmer: string): number {
  const kmerLen = kmer.length;
  const maxCheck = Math.min(acc.length, kmerLen - 1);
  for (let overlap = maxCheck; overlap >= 1; overlap--) {
    if (acc.endsWith(kmer.slice(0, overlap))) {
      return overlap;
    }
  }
  return 0;
}

/**
 * Compute HCS regions from analysis positions.
 * An HCS region is a contiguous run of positions where the Index motif
 * has incidence >= the given threshold. Stitching uses proper overlap
 * detection (matching the Rust backend) to handle edge cases where
 * lexicographically-chosen Index k-mers at adjacent positions may not
 * share a k-1 overlap.
 *
 * @param positions - The analysis result positions (1-based)
 * @param threshold - Minimum Index motif incidence to qualify (0-100)
 * @returns Array of HCS regions with stitched sequences and indices
 */
export function computeHCSRegions(
  positions: Position[],
  threshold: number,
): HCSRegion[] {
  // Clamp threshold to valid range
  const clampedThreshold = Math.max(0, Math.min(100, threshold));

  // Sort by position ascending — HCS stitching logic requires ordered input
  const sorted = [...positions].sort((a, b) => a.position - b.position);

  const regions: HCSRegion[] = [];
  let currentRegion: HCSRegion | null = null;

  for (const pos of sorted) {
    // When multiple Index variants exist (tied max count), use the lexicographically
    // first sequence for determinism — matches the Rust backend's behavior.
    const indexCandidates = (pos.diversity_motifs ?? [])
      .filter((v) => v.motif_short === 'I' && v.incidence >= clampedThreshold)
      .sort((a, b) => a.sequence.localeCompare(b.sequence));
    const indexVariant = indexCandidates[0] ?? null;

    if (indexVariant) {
      // Only NS/LS are truly unreliable. "ELS" (from old results) is at-threshold and valid.
      const isLowSupport = pos.low_support === 'NS' || pos.low_support === 'LS';
      const kmerLength = indexVariant.sequence.length;
      // Match Rust backend: for k>1, adjacent k-mers share a (k-1)-character overlap.
      // For k=1, there's no overlap between single-char k-mers but adjacent positions
      // are still contiguous (overlap requirement = 0, always satisfied).
      const minRequiredOverlap = kmerLength > 1 ? kmerLength - 1 : 0;

      if (currentRegion) {
        // Only attempt stitching between consecutive positions. A gap in position
        // numbers (e.g., 1,2,4) means there's a non-Index position in between —
        // stitching across that gap would produce biologically invalid sequences.
        const isConsecutive = pos.position === currentRegion.endPosition + 1;
        const overlapLen = isConsecutive
          ? findOverlapLength(currentRegion.sequence, indexVariant.sequence)
          : 0;
        if (isConsecutive && overlapLen >= minRequiredOverlap) {
          currentRegion.endPosition = pos.position;
          currentRegion.sequence += indexVariant.sequence.slice(overlapLen);
          currentRegion.indices.push(pos.position);
          if (isLowSupport) currentRegion.lowSupportPositions.push(pos.position);
        } else {
          // Non-consecutive or insufficient overlap — break region
          regions.push(currentRegion);
          currentRegion = {
            startPosition: pos.position,
            endPosition: pos.position,
            sequence: indexVariant.sequence,
            indices: [pos.position],
            lowSupportPositions: isLowSupport ? [pos.position] : [],
          };
        }
      } else {
        currentRegion = {
          startPosition: pos.position,
          endPosition: pos.position,
          sequence: indexVariant.sequence,
          indices: [pos.position],
          lowSupportPositions: isLowSupport ? [pos.position] : [],
        };
      }
    } else {
      // No qualifying Index at this position — break contiguity
      if (currentRegion) {
        regions.push(currentRegion);
      }
      currentRegion = null;
    }
  }

  if (currentRegion !== null) {
    regions.push(currentRegion);
  }

  return regions;
}
