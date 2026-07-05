/**
 * DiMA Desktop - Color Utility Tests
 */

import { describe, it, expect } from 'vitest';
import { 
  getCharacterColor, 
  getMotifColor,
  RASMOL_COLORS,
  NUCLEOTIDE_COLORS,
} from '../lib/colors';

describe('getCharacterColor', () => {
  it('returns RasMol color for protein amino acids', () => {
    expect(getCharacterColor('D', 'protein')).toBe(RASMOL_COLORS['D']);
    expect(getCharacterColor('K', 'protein')).toBe(RASMOL_COLORS['K']);
    expect(getCharacterColor('A', 'protein')).toBe(RASMOL_COLORS['A']);
  });

  it('returns nucleotide color for DNA bases', () => {
    expect(getCharacterColor('A', 'nucleotide')).toBe(NUCLEOTIDE_COLORS['A']);
    expect(getCharacterColor('T', 'nucleotide')).toBe(NUCLEOTIDE_COLORS['T']);
    expect(getCharacterColor('G', 'nucleotide')).toBe(NUCLEOTIDE_COLORS['G']);
    expect(getCharacterColor('C', 'nucleotide')).toBe(NUCLEOTIDE_COLORS['C']);
  });

  it('handles lowercase input', () => {
    expect(getCharacterColor('a', 'nucleotide')).toBe(NUCLEOTIDE_COLORS['A']);
    expect(getCharacterColor('d', 'protein')).toBe(RASMOL_COLORS['D']);
  });

  it('returns default color for unknown characters', () => {
    expect(getCharacterColor('X', 'nucleotide')).toBe('#888888');
  });
});

describe('getMotifColor', () => {
  it('returns correct color for each motif type', () => {
    expect(getMotifColor('I')).toBe('#2E7D32');
    expect(getMotifColor('Ma')).toBe('#1565C0');
    expect(getMotifColor('Mi')).toBe('#E65100');
    expect(getMotifColor('U')).toBe('#616161');
  });

  it('returns default for null', () => {
    expect(getMotifColor(null)).toBe('#888888');
  });
});
