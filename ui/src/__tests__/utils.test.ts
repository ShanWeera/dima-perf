/**
 * DiMA Desktop - Utility Function Tests
 */

import { describe, it, expect } from 'vitest';
import { formatNumber, formatBytes, truncate } from '../lib/utils';

describe('formatNumber', () => {
  it('formats number with default precision', () => {
    expect(formatNumber(3.14159265)).toBe('3.1416');
  });

  it('formats number with custom precision', () => {
    expect(formatNumber(3.14159265, 2)).toBe('3.14');
  });

  it('handles zero', () => {
    expect(formatNumber(0)).toBe('0.0000');
  });
});

describe('formatBytes', () => {
  it('formats bytes', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
  });

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
  });

  it('formats megabytes', () => {
    expect(formatBytes(1048576)).toBe('1 MB');
  });

  it('formats gigabytes', () => {
    expect(formatBytes(1073741824)).toBe('1 GB');
  });
});

describe('truncate', () => {
  it('returns string unchanged if shorter than limit', () => {
    expect(truncate('hello', 10)).toBe('hello');
  });

  it('truncates with ellipsis', () => {
    expect(truncate('hello world', 8)).toBe('hello...');
  });

  it('handles exact length', () => {
    expect(truncate('hello', 5)).toBe('hello');
  });
});
