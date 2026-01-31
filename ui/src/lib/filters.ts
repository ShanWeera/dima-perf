/**
 * DiMA Desktop - Filter Utilities
 * 
 * Utility functions for working with search filters and filter presets.
 */

import type { SearchFilters, FilterPreset, Position, MotifType } from './types';

/**
 * Default filter values
 */
export const DEFAULT_FILTERS: SearchFilters = {
  positionRange: null,
  sequenceQuery: '',
  entropyRange: null,
  motifTypes: ['I', 'Ma', 'Mi', 'U'],
  includeLowSupport: true,
};

/**
 * Check if filters are at default values
 */
export function areFiltersDefault(filters: SearchFilters): boolean {
  return (
    filters.positionRange === null &&
    filters.sequenceQuery === '' &&
    filters.entropyRange === null &&
    filters.motifTypes.length === 4 &&
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
  if (filters.motifTypes.length < 4) count++;
  if (!filters.includeLowSupport) count++;
  
  return count;
}

/**
 * Create a human-readable description of active filters
 */
export function describeFilters(filters: SearchFilters): string[] {
  const descriptions: string[] = [];
  
  if (filters.positionRange) {
    descriptions.push(`Positions ${filters.positionRange[0]}-${filters.positionRange[1]}`);
  }
  
  if (filters.sequenceQuery) {
    descriptions.push(`Contains "${filters.sequenceQuery}"`);
  }
  
  if (filters.entropyRange) {
    descriptions.push(`Entropy ${filters.entropyRange[0].toFixed(2)}-${filters.entropyRange[1].toFixed(2)}`);
  }
  
  if (filters.motifTypes.length < 4) {
    const motifNames: Record<MotifType, string> = {
      'I': 'Index',
      'Ma': 'Major',
      'Mi': 'Minor',
      'U': 'Unique',
    };
    const selected = filters.motifTypes.map(t => motifNames[t]).join(', ');
    descriptions.push(`Motifs: ${selected}`);
  }
  
  if (!filters.includeLowSupport) {
    descriptions.push('Excluding low support');
  }
  
  return descriptions;
}

/**
 * Apply filters to a list of positions
 */
export function applyFiltersToPositions(
  positions: Position[],
  filters: SearchFilters
): Position[] {
  return positions.filter((pos) => {
    // Position range filter
    if (filters.positionRange) {
      if (
        pos.position < filters.positionRange[0] ||
        pos.position > filters.positionRange[1]
      ) {
        return false;
      }
    }

    // Entropy range filter
    if (filters.entropyRange) {
      if (
        pos.entropy < filters.entropyRange[0] ||
        pos.entropy > filters.entropyRange[1]
      ) {
        return false;
      }
    }

    // Low support filter
    if (!filters.includeLowSupport && pos.low_support) {
      return false;
    }

    // Sequence query filter
    if (filters.sequenceQuery && pos.diversity_motifs) {
      const query = filters.sequenceQuery.toUpperCase();
      const hasMatch = pos.diversity_motifs.some((v) =>
        v.sequence.toUpperCase().includes(query)
      );
      if (!hasMatch) {
        return false;
      }
    }

    // Motif type filter
    if (filters.motifTypes.length < 4 && pos.diversity_motifs) {
      const hasMatchingMotif = pos.diversity_motifs.some((v) => {
        const motif = v.motif_short as MotifType | null;
        return motif && filters.motifTypes.includes(motif);
      });
      if (!hasMatchingMotif) {
        return false;
      }
    }

    return true;
  });
}

/**
 * Merge two filter presets
 */
export function mergePresets(
  existing: FilterPreset[],
  incoming: FilterPreset[]
): FilterPreset[] {
  const merged = [...existing];
  
  for (const preset of incoming) {
    const existingIndex = merged.findIndex(p => p.id === preset.id);
    if (existingIndex >= 0) {
      merged[existingIndex] = preset;
    } else {
      merged.push(preset);
    }
  }
  
  return merged;
}

/**
 * Validate a filter preset
 */
export function isValidPreset(preset: Partial<FilterPreset>): preset is FilterPreset {
  return (
    typeof preset.id === 'string' &&
    preset.id.length > 0 &&
    typeof preset.name === 'string' &&
    preset.name.length > 0 &&
    preset.filters !== undefined
  );
}

/**
 * Create a preset from current filters
 */
export function createPreset(name: string, filters: SearchFilters): FilterPreset {
  return {
    id: crypto.randomUUID(),
    name,
    filters: { ...filters },
  };
}

/**
 * Get suggested filter presets for common use cases
 */
export function getSuggestedPresets(): Array<{ name: string; filters: Partial<SearchFilters> }> {
  return [
    {
      name: 'High Entropy Only',
      filters: {
        entropyRange: [0.5, Number.MAX_VALUE],
      },
    },
    {
      name: 'Index Sequences Only',
      filters: {
        motifTypes: ['I'],
      },
    },
    {
      name: 'Exclude Low Support',
      filters: {
        includeLowSupport: false,
      },
    },
    {
      name: 'Highly Conserved',
      filters: {
        entropyRange: [0, 0.1],
      },
    },
    {
      name: 'Variable Positions',
      filters: {
        motifTypes: ['Ma', 'Mi', 'U'],
      },
    },
  ];
}

/**
 * Calculate entropy range from positions
 */
export function getEntropyRange(positions: Position[]): [number, number] {
  if (positions.length === 0) {
    return [0, 1];
  }
  
  let min = Number.MAX_VALUE;
  let max = Number.MIN_VALUE;
  
  for (const pos of positions) {
    if (pos.entropy < min) min = pos.entropy;
    if (pos.entropy > max) max = pos.entropy;
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
  
  let min = Number.MAX_VALUE;
  let max = Number.MIN_VALUE;
  
  for (const pos of positions) {
    if (pos.position < min) min = pos.position;
    if (pos.position > max) max = pos.position;
  }
  
  return [min, max];
}
