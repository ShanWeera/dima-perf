/**
 * DiMA Desktop - Color Utility Tests
 */

import { describe, it, expect } from 'vitest';
import { 
  getCharacterColor, 
  getEntropyColor, 
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

describe('getEntropyColor', () => {
  it('returns blue for low entropy', () => {
    const color = getEntropyColor(0);
    expect(color).toBe('#313695');
  });

  it('returns red for high entropy', () => {
    const color = getEntropyColor(0.99);
    expect(color).toBe('#a50026');
  });

  it('returns intermediate color for mid entropy', () => {
    const color = getEntropyColor(0.5);
    expect(color).toBeDefined();
  });
});

describe('getMotifColor', () => {
  it('returns correct color for each motif type', () => {
    expect(getMotifColor('I')).toBe('#4CAF50');
    expect(getMotifColor('Ma')).toBe('#2196F3');
    expect(getMotifColor('Mi')).toBe('#FF9800');
    expect(getMotifColor('U')).toBe('#9E9E9E');
  });

  it('returns default for null', () => {
    expect(getMotifColor(null)).toBe('#888888');
  });
});
