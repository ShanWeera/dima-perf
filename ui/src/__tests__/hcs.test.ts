/**
 * DiMA Desktop - HCS (Highly Conserved Sequences) Tests
 *
 * Tests for computeHCSRegions covering:
 * - Single position HCS
 * - Adjacent overlapping k-mers (standard stitching)
 * - Non-overlapping adjacent positions (breaks region)
 * - Threshold filtering
 * - Multiple tied Index variants (lexicographic tie-break)
 * - Unsorted position input (should still work correctly)
 * - Empty positions
 * - Threshold clamping (negative, >100)
 */

import { describe, it, expect } from 'vitest';
import { computeHCSRegions } from '../lib/hcs';
import type { Position } from '../lib/types';

function makeHCSPosition(
  position: number,
  indexSequence: string,
  incidence: number,
  extraMotifs: Position['diversity_motifs'] = [],
): Position {
  return {
    position,
    entropy: 0.5,
    support: 100,
    low_support: null,
    distinct_variants_count: 1,
    distinct_variants_incidence: 0,
    total_variants_incidence: 0,
    diversity_motifs: [
      {
        sequence: indexSequence,
        motif_long: 'Index',
        motif_short: 'I',
        count: 80,
        incidence,
        metadata: null,
      },
      ...(extraMotifs ?? []),
    ],
  };
}

describe('computeHCSRegions', () => {
  it('returns empty for empty positions', () => {
    expect(computeHCSRegions([], 90)).toEqual([]);
  });

  it('creates a single-position HCS region', () => {
    const positions = [makeHCSPosition(1, 'ABCDE', 95)];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(1);
    expect(regions[0]).toEqual({
      startPosition: 1,
      endPosition: 1,
      sequence: 'ABCDE',
      indices: [1],
      lowSupportPositions: [],
    });
  });

  it('stitches adjacent overlapping k-mers', () => {
    const positions = [
      makeHCSPosition(1, 'ABCDE', 95),
      makeHCSPosition(2, 'BCDEF', 95),
      makeHCSPosition(3, 'CDEFG', 95),
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(1);
    expect(regions[0].sequence).toBe('ABCDEFG');
    expect(regions[0].startPosition).toBe(1);
    expect(regions[0].endPosition).toBe(3);
    expect(regions[0].indices).toEqual([1, 2, 3]);
  });

  it('breaks region when no overlap exists', () => {
    const positions = [
      makeHCSPosition(1, 'ABCDE', 95),
      makeHCSPosition(2, 'ZZZZZ', 95),
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(2);
    expect(regions[0].sequence).toBe('ABCDE');
    expect(regions[1].sequence).toBe('ZZZZZ');
  });

  it('filters out positions below threshold', () => {
    const positions = [
      makeHCSPosition(1, 'ABCDE', 95),
      makeHCSPosition(2, 'BCDEF', 80),
      makeHCSPosition(3, 'CDEFG', 95),
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(2);
    expect(regions[0].sequence).toBe('ABCDE');
    expect(regions[0].indices).toEqual([1]);
    expect(regions[1].sequence).toBe('CDEFG');
    expect(regions[1].indices).toEqual([3]);
  });

  it('picks lexicographically first Index when tied', () => {
    const positions: Position[] = [
      {
        position: 1,
        entropy: 0.5,
        support: 100,
        low_support: null,
        distinct_variants_count: 2,
        distinct_variants_incidence: 0,
        total_variants_incidence: 0,
        diversity_motifs: [
          { sequence: 'ZZZAA', motif_long: 'Index', motif_short: 'I', count: 50, incidence: 95, metadata: null },
          { sequence: 'AAAAA', motif_long: 'Index', motif_short: 'I', count: 50, incidence: 95, metadata: null },
        ],
      },
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(1);
    expect(regions[0].sequence).toBe('AAAAA');
  });

  it('handles unsorted position input', () => {
    const positions = [
      makeHCSPosition(3, 'CDEFG', 95),
      makeHCSPosition(1, 'ABCDE', 95),
      makeHCSPosition(2, 'BCDEF', 95),
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(1);
    expect(regions[0].sequence).toBe('ABCDEFG');
  });

  it('clamps negative threshold to 0', () => {
    const positions = [makeHCSPosition(1, 'ABCDE', 0.1)];
    const regions = computeHCSRegions(positions, -10);
    expect(regions).toHaveLength(1);
  });

  it('clamps threshold above 100 to 100', () => {
    const positions = [makeHCSPosition(1, 'ABCDE', 99.9)];
    const regions = computeHCSRegions(positions, 150);
    expect(regions).toHaveLength(0);
  });

  it('tracks low-support positions within HCS regions', () => {
    // ELS (at-threshold) is scientifically valid after rarefaction correction
    // and NOT tracked as low support. Only NS/LS are truly unreliable.
    const positions: Position[] = [
      makeHCSPosition(1, 'ABCDE', 95),
      { ...makeHCSPosition(2, 'BCDEF', 95), low_support: 'LS' },
      makeHCSPosition(3, 'CDEFG', 95),
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(1);
    expect(regions[0].lowSupportPositions).toEqual([2]);
  });

  it('tracks multiple low-support positions in a region', () => {
    const positions: Position[] = [
      { ...makeHCSPosition(1, 'ABCDE', 95), low_support: 'LS' },
      { ...makeHCSPosition(2, 'BCDEF', 95), low_support: 'NS' },
      makeHCSPosition(3, 'CDEFG', 95),
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(1);
    expect(regions[0].lowSupportPositions).toEqual([1, 2]);
  });

  it('creates separate regions when Index is absent at some positions', () => {
    const positions: Position[] = [
      makeHCSPosition(1, 'ABCDE', 95),
      makeHCSPosition(2, 'BCDEF', 95),
      {
        position: 3,
        entropy: 1.5,
        support: 100,
        low_support: null,
        distinct_variants_count: 1,
        distinct_variants_incidence: 0,
        total_variants_incidence: 0,
        diversity_motifs: [
          { sequence: 'XXXXX', motif_long: 'Unique', motif_short: 'U', count: 1, incidence: 1.0, metadata: null },
        ],
      },
      makeHCSPosition(4, 'HIJKL', 95),
    ];
    const regions = computeHCSRegions(positions, 90);
    expect(regions).toHaveLength(2);
    expect(regions[0].indices).toEqual([1, 2]);
    expect(regions[1].indices).toEqual([4]);
  });
});
