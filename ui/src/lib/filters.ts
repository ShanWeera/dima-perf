/**
 * DiMA Desktop - Filter Utilities
 * 
 * Utility functions for working with search filters and filter presets.
 */

import type { SearchFilters, FilterPreset, Position, MotifType } from './types';

const ALL_MOTIF_TYPES: MotifType[] = ['I', 'Ma', 'Mi', 'U'];

/**
 * Default filter values
 */
export const DEFAULT_FILTERS: SearchFilters = {
  positionRange: null,
  sequenceQuery: '',
  entropyRange: null,
  motifTypes: [...ALL_MOTIF_TYPES],
  includeLowSupport: true,
};

/**
 * Check if filters are at default values.
 * Checks motif type content, not just array length.
 */
export function areFiltersDefault(filters: SearchFilters): boolean {
  const motifTypesAreDefault =
    filters.motifTypes.length === ALL_MOTIF_TYPES.length &&
    ALL_MOTIF_TYPES.every((t) => filters.motifTypes.includes(t));

  return (
    filters.positionRange === null &&
    filters.sequenceQuery === '' &&
    filters.entropyRange === null &&
    motifTypesAreDefault &&
    filters.includeLowSupport === true
  );
}

/**
 * Count active filters
 */
export function countActiveFilters(filters: SearchFilters): number {
  let count = 0;
  
  if (filters.positionRange !== null) count++;
  if (filters.sequenceQuery !== '') count++;
  if (filters.entropyRange !== null) count++;
  if (
    filters.motifTypes.length < ALL_MOTIF_TYPES.length ||
    !ALL_MOTIF_TYPES.every((t) => filters.motifTypes.includes(t))
  ) {
    count++;
  }
  if (!filters.includeLowSupport) count++;
  
  return count;
}

/**
 * Apply filters to a list of positions.
 * 
 * Key behaviors:
 * - Positions with null diversity_motifs are EXCLUDED when motif or sequence filters are active
 * - Inverted numeric ranges (from > to) are treated as empty and return no results
 * - NaN filter values are treated as unset (filter disabled)
 */
export function applyFiltersToPositions(
  positions: Position[],
  filters: SearchFilters
): Position[] {
  // Pre-validate numeric ranges — inverted ranges match nothing
  const posRange = filters.positionRange;
  if (posRange && (Number.isNaN(posRange[0]) || Number.isNaN(posRange[1]))) {
    // NaN range — treat as unset (don't filter)
  } else if (posRange && posRange[0] > posRange[1]) {
    return []; // Inverted range = no results
  }

  const entRange = filters.entropyRange;
  if (entRange && (Number.isNaN(entRange[0]) || Number.isNaN(entRange[1]))) {
    // NaN range — treat as unset
  } else if (entRange && entRange[0] > entRange[1]) {
    return [];
  }

  // Treat an empty motifTypes array as "no motif filter" rather than "exclude
  // everything". This prevents accidentally hiding all positions when the user
  // deselects every motif type in the filter panel. (Fix 6.23)
  const motifFilterActive = filters.motifTypes.length > 0 && (
    filters.motifTypes.length < ALL_MOTIF_TYPES.length ||
    !ALL_MOTIF_TYPES.every((t) => filters.motifTypes.includes(t))
  );

  // Cap query length to prevent O(n*m) CPU DoS with very long paste values
  const MAX_SEQUENCE_QUERY_LENGTH = 200;
  const rawQuery = filters.sequenceQuery.trim().slice(0, MAX_SEQUENCE_QUERY_LENGTH);
  const sequenceFilterActive = rawQuery.length > 0;
  const query = sequenceFilterActive ? rawQuery.toUpperCase() : '';

  return positions.filter((pos) => {
    // Position range filter
    if (posRange && !Number.isNaN(posRange[0]) && !Number.isNaN(posRange[1])) {
      if (pos.position < posRange[0] || pos.position > posRange[1]) {
        return false;
      }
    }

    // Entropy range filter — also reject NaN/non-finite entropy values (Fix 5.63).
    // NaN comparisons always return false, so without the isFinite guard, NaN
    // positions would silently pass through the range check.
    if (entRange && !Number.isNaN(entRange[0]) && !Number.isNaN(entRange[1])) {
      if (!Number.isFinite(pos.entropy) || pos.entropy < entRange[0] || pos.entropy > entRange[1]) {
        return false;
      }
    }

    // Low support filter — only exclude NS (No Support) and LS (Low Support).
    // Per PMC11596295: positions at exactly the threshold (formerly tagged "ELS")
    // are scientifically valid and should NOT be excluded. This also ensures
    // backward compatibility with old results that may still contain "ELS" values.
    if (!filters.includeLowSupport && (pos.low_support === 'NS' || pos.low_support === 'LS')) {
      return false;
    }

    // For motif/sequence filters, positions without motif data are excluded
    if ((motifFilterActive || sequenceFilterActive) && !pos.diversity_motifs) {
      return false;
    }

    // Combined motif + sequence filter: require a SINGLE variant to satisfy BOTH
    if (motifFilterActive || sequenceFilterActive) {
      const variants = pos.diversity_motifs!;
      const hasMatch = variants.some((v) => {
        const motifMatch = !motifFilterActive || (
          v.motif_short != null && filters.motifTypes.includes(v.motif_short as MotifType)
        );
        const seqMatch = !sequenceFilterActive || (
          v.sequence != null && v.sequence.toUpperCase().includes(query)
        );
        return motifMatch && seqMatch;
      });
      if (!hasMatch) {
        return false;
      }
    }

    return true;
  });
}

/**
 * Create a preset from current filters.
 * Deep-copies the motifTypes array to prevent shared-reference mutation.
 */
export function createPreset(name: string, filters: SearchFilters): FilterPreset {
  return {
    id: crypto.randomUUID(),
    name,
    filters: {
      ...filters,
      motifTypes: [...filters.motifTypes],
      positionRange: filters.positionRange ? [...filters.positionRange] : null,
      entropyRange: filters.entropyRange ? [...filters.entropyRange] : null,
    },
  };
}

/**
 * Calculate entropy range from positions
 */
export function getEntropyRange(positions: Position[]): [number, number] {
  if (positions.length === 0) {
    return [0, 1];
  }
  
  let min = Infinity;
  let max = -Infinity;
  
  for (const pos of positions) {
    if (Number.isFinite(pos.entropy)) {
      if (pos.entropy < min) min = pos.entropy;
      if (pos.entropy > max) max = pos.entropy;
    }
  }
  
  if (!Number.isFinite(min) || !Number.isFinite(max)) {
    return [0, 1];
  }
  
  return [min, max];
}

/**
 * Calculate position range from positions
 */
export function getPositionRange(positions: Position[]): [number, number] {
  if (positions.length === 0) {
    return [1, 1];
  }
  
  let min = Infinity;
  let max = -Infinity;
  
  for (const pos of positions) {
    if (pos.position < min) min = pos.position;
    if (pos.position > max) max = pos.position;
  }
  
  return [min, max];
}
