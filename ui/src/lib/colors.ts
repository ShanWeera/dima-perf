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
 * Entropy color scale — Viridis-inspired, colorblind-safe.
 * Uses a perceptually uniform sequential palette (dark purple → blue → teal → green → yellow)
 * that remains distinguishable under protanopia, deuteranopia, and tritanopia.
 * This replaces the previous blue-to-red diverging scale which is problematic for
 * red-green colorblind users (most common CVD type).
 */
export const ENTROPY_COLORS = [
  '#440154', // Very low entropy (conserved) - Dark purple
  '#482878',
  '#3e4989',
  '#31688e',
  '#26828e', // Low entropy - Teal
  '#1f9e89', // Medium-low - Green-teal
  '#35b779',
  '#6ece58',
  '#b5de2b',
  '#fde725', // High entropy (diverse) - Bright yellow
];

/**
 * Motif type colors
 */
export const MOTIF_COLORS: Record<string, string> = {
  I: '#2E7D32',  // Index - Dark Green (passes 4.5:1 with white text)
  Ma: '#1565C0', // Major - Dark Blue (passes 4.5:1 with white text)
  Mi: '#E65100', // Minor - Dark Orange (passes 4.5:1 with white text)
  U: '#616161',  // Unique - Dark Gray (passes 4.5:1 with white text)
};

/**
 * Get color for a motif type
 */
export function getMotifColor(motifShort: string | null): string {
  if (!motifShort) return '#888888';
  return MOTIF_COLORS[motifShort] || '#888888';
}

/**
 * Convert a hex color (3/4/6/8-digit) to an rgba() string with the specified alpha.
 * Handles all standard hex formats safely, unlike template literal hex appending
 * which produces invalid CSS for 3-digit hex inputs (e.g., "#88820" from "#888" + "20").
 */
export function hexToRgba(hex: string, alpha: number): string {
  let r = 0, g = 0, b = 0;
  const h = hex.replace('#', '');
  if (h.length === 3 || h.length === 4) {
    r = parseInt(h[0] + h[0], 16);
    g = parseInt(h[1] + h[1], 16);
    b = parseInt(h[2] + h[2], 16);
  } else if (h.length >= 6) {
    r = parseInt(h.slice(0, 2), 16);
    g = parseInt(h.slice(2, 4), 16);
    b = parseInt(h.slice(4, 6), 16);
  }
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
