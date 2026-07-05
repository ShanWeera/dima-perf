/**
 * DiMA Desktop - Protein Feature Constants & Utilities
 *
 * Single source of truth for feature category configuration (colors, labels,
 * shapes, UniProt type mapping) and pure utility functions for feature
 * processing. No React imports -- everything here is independently testable.
 */

import type {
  FeatureCategoryConfig,
  ProteinFeature,
  MappedFeature,
} from './types';

// ============================================================================
// Feature Category Configuration
// ============================================================================

/**
 * Maps internal category keys to their display configuration.
 * Colors are Tailwind-500 palette values chosen to be visually distinct
 * from HCS green (#22C55E) and active-HCS gold (#FFD700).
 */
export const FEATURE_CATEGORIES: Record<string, FeatureCategoryConfig> = {
  DOMAIN:   { color: '#3B82F6', label: 'Domains',         shape: 'rect',   uniprotTypes: ['DOMAIN'] },
  REGION:   { color: '#8B5CF6', label: 'Regions',         shape: 'rect',   uniprotTypes: ['REGION'] },
  BINDING:  { color: '#EF4444', label: 'Binding Sites',   shape: 'circle', uniprotTypes: ['BINDING', 'NP_BIND'] },
  ACT_SITE: { color: '#F97316', label: 'Active Sites',    shape: 'circle', uniprotTypes: ['ACT_SITE'] },
  SIGNAL:   { color: '#14B8A6', label: 'Signal Peptide',  shape: 'rect',   uniprotTypes: ['SIGNAL'] },
  TRANSMEM: { color: '#EC4899', label: 'Transmembrane',   shape: 'rect',   uniprotTypes: ['TRANSMEM'] },
  CARBOHYD: { color: '#A855F7', label: 'Glycosylation',   shape: 'circle', uniprotTypes: ['CARBOHYD'] },
  DISULFID: { color: '#F59E0B', label: 'Disulfide Bonds', shape: 'circle', uniprotTypes: ['DISULFID'] },
  TOPO_DOM: { color: '#06B6D4', label: 'Topology',        shape: 'rect',   uniprotTypes: ['TOPO_DOM'] },
  MOTIF:    { color: '#84CC16', label: 'Motifs',           shape: 'rect',   uniprotTypes: ['MOTIF'] },
};

/** Ordered list of category keys for consistent rendering */
export const FEATURE_CATEGORY_ORDER = Object.keys(FEATURE_CATEGORIES);

// ============================================================================
// Pure Utility Functions
// ============================================================================

/**
 * Determine the internal category key for a given UniProt feature.
 * Returns null if the feature type is not in any known category.
 */
export function categorizeFeature(feature: ProteinFeature): string | null {
  for (const [key, config] of Object.entries(FEATURE_CATEGORIES)) {
    if (config.uniprotTypes.includes(feature.feature_type)) {
      return key;
    }
  }
  return null;
}

/**
 * Whether a feature represents a single residue (point) or a range.
 */
export function isPointFeature(feature: ProteinFeature): boolean {
  return feature.begin === feature.end;
}

/**
 * Group mapped features by their category key.
 */
export function groupFeaturesByCategory(
  features: MappedFeature[]
): Map<string, MappedFeature[]> {
  const grouped = new Map<string, MappedFeature[]>();
  for (const f of features) {
    const existing = grouped.get(f.categoryKey);
    if (existing) {
      existing.push(f);
    } else {
      grouped.set(f.categoryKey, [f]);
    }
  }
  return grouped;
}

/**
 * Compact range overlap check: do [aStart, aEnd] and [bStart, bEnd] intersect?
 */
function rangesOverlap(
  aStart: number, aEnd: number,
  bStart: number, bEnd: number
): boolean {
  return aStart <= bEnd && bStart <= aEnd;
}

/**
 * For each HCS region (identified by its index), compute which mapped
 * features overlap with it. Returns a Map from HCS region index to
 * the array of overlapping features.
 *
 * IMPORTANT (Fix 5.65): HCS startPosition/endPosition are k-mer window
 * START positions. The region actually covers alignment columns from
 * startPosition to endPosition + kmerLength - 1. Without the k-mer
 * correction, features overlapping the trailing (k-1) residues of a
 * region would be missed.
 */
export function computeFeatureOverlaps(
  hcsRegions: Array<{ startPosition: number; endPosition: number }>,
  mappedFeatures: MappedFeature[],
  kmerLength: number = 1
): Map<number, MappedFeature[]> {
  const result = new Map<number, MappedFeature[]>();

  if (hcsRegions.length === 0 || mappedFeatures.length === 0) {
    return result;
  }

  const validFeatures = mappedFeatures.filter(
    (f) => f.msaBegin !== null && f.msaEnd !== null
  );

  // The actual last column covered by an HCS region extends (k-1)
  // residues beyond the last window start position.
  const kExtension = Math.max(0, kmerLength - 1);

  for (let i = 0; i < hcsRegions.length; i++) {
    const hcs = hcsRegions[i];
    const regionEnd = hcs.endPosition + kExtension;
    const overlapping: MappedFeature[] = [];

    for (const f of validFeatures) {
      if (rangesOverlap(hcs.startPosition, regionEnd, f.msaBegin!, f.msaEnd!)) {
        overlapping.push(f);
      }
    }

    if (overlapping.length > 0) {
      result.set(i, overlapping);
    }
  }

  return result;
}
