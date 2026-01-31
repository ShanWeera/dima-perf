/**
 * DiMA Desktop - Sequence Color Schemes
 * 
 * Color schemes for visualizing protein and nucleotide sequences.
 */

/**
 * RasMol color scheme for amino acids
 * Standard bioinformatics coloring based on chemical properties
 */
export const RASMOL_COLORS: Record<string, string> = {
  // Acidic - Bright Red
  D: '#E60A0A',
  E: '#E60A0A',
  // Basic - Blue
  K: '#145AFF',
  R: '#145AFF',
  // Histidine - Pale Blue
  H: '#8282D2',
  // Aromatic
  F: '#3232AA',
  Y: '#3232AA',
  W: '#B45AB4',
  // Sulfur-containing - Yellow
  C: '#E6E600',
  M: '#E6E600',
  // Hydroxyl - Orange
  S: '#FA9600',
  T: '#FA9600',
  // Amide - Cyan
  N: '#00DCDC',
  Q: '#00DCDC',
  // Small
  G: '#EBEBEB',
  A: '#C8C8C8',
  // Proline - Flesh
  P: '#DC9682',
  // Aliphatic - Green
  V: '#0F820F',
  L: '#0F820F',
  I: '#0F820F',
  // Unknown/Gap
  X: '#BEA06E',
  '-': '#888888',
  '*': '#888888',
};

/**
 * Standard nucleotide colors
 */
export const NUCLEOTIDE_COLORS: Record<string, string> = {
  A: '#33CC33', // Adenine - Green
  T: '#CC3333', // Thymine - Red
  U: '#CC3333', // Uracil - Red (same as T)
  G: '#CCCC33', // Guanine - Yellow
  C: '#3333CC', // Cytosine - Blue
  N: '#888888', // Unknown
  '-': '#888888', // Gap
};

/**
 * Get color for a character based on alphabet type
 */
export function getCharacterColor(
  char: string,
  alphabet: 'protein' | 'nucleotide'
): string {
  const upperChar = char.toUpperCase();
  const colors = alphabet === 'protein' ? RASMOL_COLORS : NUCLEOTIDE_COLORS;
  return colors[upperChar] || '#888888';
}

/**
 * Entropy color scale (blue to red)
 */
export const ENTROPY_COLORS = [
  '#313695', // Very low - Dark blue
  '#4575b4',
  '#74add1',
  '#abd9e9',
  '#e0f3f8', // Low - Light blue
  '#fee090', // Medium - Yellow
  '#fdae61',
  '#f46d43',
  '#d73027',
  '#a50026', // High - Dark red
];

/**
 * Get entropy color based on normalized value (0-1)
 */
export function getEntropyColor(normalizedValue: number): string {
  const index = Math.min(
    Math.floor(normalizedValue * ENTROPY_COLORS.length),
    ENTROPY_COLORS.length - 1
  );
  return ENTROPY_COLORS[index];
}

/**
 * Motif type colors
 */
export const MOTIF_COLORS: Record<string, string> = {
  I: '#4CAF50',  // Index - Green
  Ma: '#2196F3', // Major - Blue
  Mi: '#FF9800', // Minor - Orange
  U: '#9E9E9E',  // Unique - Gray
};

/**
 * Get color for a motif type
 */
export function getMotifColor(motifShort: string | null): string {
  if (!motifShort) return '#888888';
  return MOTIF_COLORS[motifShort] || '#888888';
}
