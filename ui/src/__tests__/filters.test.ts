/**
 * DiMA Desktop - Filter Utility Tests
 *
 * Tests for applyFiltersToPositions covering:
 * - Default filters (no filtering)
 * - Position range filtering (normal, inverted, NaN)
 * - Entropy range filtering
 * - Motif type filtering (subset, empty array = no filter)
 * - Sequence query filtering
 * - Combined motif + sequence filtering
 * - Low support filtering
 * - Edge cases: empty positions, null bounds
 */

import { describe, it, expect } from 'vitest';
import { applyFiltersToPositions, DEFAULT_FILTERS } from '../lib/filters';
import type { Position, SearchFilters } from '../lib/types';

function makePosition(overrides: Partial<Position> = {}): Position {
  return {
    position: 1,
    entropy: 1.5,
    support: 100,
    low_support: null,
    distinct_variants_count: 3,
    distinct_variants_incidence: 50.0,
    total_variants_incidence: 0.2,
    diversity_motifs: [
      { sequence: 'ABCDE', motif_long: 'Index', motif_short: 'I', count: 80, incidence: 80.0, metadata: null },
      { sequence: 'FGHIJ', motif_long: 'Major', motif_short: 'Ma', count: 15, incidence: 15.0, metadata: null },
      { sequence: 'KLMNO', motif_long: 'Unique', motif_short: 'U', count: 1, incidence: 1.0, metadata: null },
    ],
    ...overrides,
  };
}

describe('applyFiltersToPositions', () => {
  it('returns all positions with default filters', () => {
    const positions = [makePosition({ position: 1 }), makePosition({ position: 2 })];
    const result = applyFiltersToPositions(positions, DEFAULT_FILTERS);
    expect(result).toHaveLength(2);
  });

  it('returns empty array for empty input', () => {
    expect(applyFiltersToPositions([], DEFAULT_FILTERS)).toEqual([]);
  });

  describe('position range', () => {
    const positions = [
      makePosition({ position: 1 }),
      makePosition({ position: 5 }),
      makePosition({ position: 10 }),
    ];

    it('filters by position range', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, positionRange: [3, 7] };
      const result = applyFiltersToPositions(positions, filters);
      expect(result).toHaveLength(1);
      expect(result[0].position).toBe(5);
    });

    it('returns empty for inverted position range', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, positionRange: [10, 1] };
      expect(applyFiltersToPositions(positions, filters)).toEqual([]);
    });

    it('treats NaN position range as unset', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, positionRange: [NaN, NaN] };
      expect(applyFiltersToPositions(positions, filters)).toHaveLength(3);
    });

    it('null position range means no filtering', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, positionRange: null };
      expect(applyFiltersToPositions(positions, filters)).toHaveLength(3);
    });
  });

  describe('entropy range', () => {
    const positions = [
      makePosition({ position: 1, entropy: 0.5 }),
      makePosition({ position: 2, entropy: 1.5 }),
      makePosition({ position: 3, entropy: 2.5 }),
    ];

    it('filters by entropy range', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, entropyRange: [1.0, 2.0] };
      const result = applyFiltersToPositions(positions, filters);
      expect(result).toHaveLength(1);
      expect(result[0].entropy).toBe(1.5);
    });

    it('returns empty for inverted entropy range', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, entropyRange: [3.0, 0.0] };
      expect(applyFiltersToPositions(positions, filters)).toEqual([]);
    });
  });

  describe('motif types', () => {
    const positions = [makePosition()];

    it('filters by specific motif types', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, motifTypes: ['I'] };
      const result = applyFiltersToPositions(positions, filters);
      expect(result).toHaveLength(1);
    });

    it('empty motifTypes array means no motif filter (Fix 6.23)', () => {
      const filters: SearchFilters = { ...DEFAULT_FILTERS, motifTypes: [] };
      const result = applyFiltersToPositions(positions, filters);
      expect(result).toHaveLength(1);
    });

    it('excludes positions without matching motif', () => {
      const positions = [
        makePosition({
          position: 1,
          diversity_motifs: [
            { sequence: 'ABCDE', motif_long: 'Unique', motif_short: 'U', count: 1, incidence: 1.0, metadata: null },
          ],
        }),
      ];
      const filters: SearchFilters = { ...DEFAULT_FILTERS, motifTypes: ['I'] };
      expect(applyFiltersToPositions(positions, filters)).toHaveLength(0);
    });
  });

  describe('sequence query', () => {
    it('filters by sequence substring (case insensitive)', () => {
      const positions = [makePosition()];
      const filters: SearchFilters = { ...DEFAULT_FILTERS, sequenceQuery: 'abc' };
      const result = applyFiltersToPositions(positions, filters);
      expect(result).toHaveLength(1);
    });

    it('excludes positions without matching sequence', () => {
      const positions = [makePosition()];
      const filters: SearchFilters = { ...DEFAULT_FILTERS, sequenceQuery: 'ZZZZZ' };
      expect(applyFiltersToPositions(positions, filters)).toHaveLength(0);
    });
  });

  describe('combined motif + sequence', () => {
    it('requires a single variant to match both motif and sequence', () => {
      const positions = [makePosition()];
      const filters: SearchFilters = {
        ...DEFAULT_FILTERS,
        motifTypes: ['I'],
        sequenceQuery: 'ABC',
      };
      expect(applyFiltersToPositions(positions, filters)).toHaveLength(1);
    });

    it('excludes when motif and sequence match different variants', () => {
      const positions = [makePosition()];
      const filters: SearchFilters = {
        ...DEFAULT_FILTERS,
        motifTypes: ['U'],
        sequenceQuery: 'ABC',
      };
      expect(applyFiltersToPositions(positions, filters)).toHaveLength(0);
    });
  });

  describe('low support', () => {
    it('includes low support by default', () => {
      const positions = [makePosition({ low_support: 'LS' })];
      expect(applyFiltersToPositions(positions, DEFAULT_FILTERS)).toHaveLength(1);
    });

    it('excludes low support when includeLowSupport is false', () => {
      const positions = [makePosition({ low_support: 'LS' })];
      const filters: SearchFilters = { ...DEFAULT_FILTERS, includeLowSupport: false };
      expect(applyFiltersToPositions(positions, filters)).toHaveLength(0);
    });
  });
});
