import { describe, it, expect } from 'vitest';
import { lttbDownsampleByRange } from '../lib/lttb';

describe('lttbDownsampleByRange', () => {
  it('returns input unchanged when below threshold', () => {
    const data: [number, number][] = [[1, 0.5], [2, 0.6], [3, 0.4]];
    expect(lttbDownsampleByRange(data, 5)).toBe(data);
  });

  it('returns input unchanged when threshold < 3', () => {
    const data: [number, number][] = [[1, 0.5], [2, 0.6], [3, 0.4], [4, 0.7]];
    expect(lttbDownsampleByRange(data, 2)).toBe(data);
  });

  it('always preserves first and last points', () => {
    const data: [number, number][] = [
      [1, 0.1], [5, 0.5], [10, 0.3], [15, 0.8],
      [20, 0.2], [25, 0.6], [30, 0.9], [35, 0.4],
      [40, 0.7], [45, 0.1], [50, 0.5],
    ];
    const result = lttbDownsampleByRange(data, 5);
    expect(result[0]).toEqual([1, 0.1]);
    expect(result[result.length - 1]).toEqual([50, 0.5]);
  });

  it('reduces data size to approximately the threshold', () => {
    const data: [number, number][] = Array.from({ length: 1000 }, (_, i) => [
      i + 1,
      Math.sin(i * 0.1) * 0.5 + 0.5,
    ]);
    const result = lttbDownsampleByRange(data, 100);
    // Output may be less than threshold if some buckets are empty, but never more
    expect(result.length).toBeLessThanOrEqual(100);
    expect(result.length).toBeGreaterThan(50);
  });

  it('preserves peaks in non-uniform data', () => {
    // Data with a clear spike at position 50 (big gap between 20 and 50)
    const data: [number, number][] = [
      [1, 0.1], [2, 0.1], [3, 0.1], [4, 0.1], [5, 0.1],
      [10, 0.2], [15, 0.2], [20, 0.2],
      [50, 0.9],  // Spike — should be preserved despite sparse region
      [80, 0.2], [85, 0.2], [90, 0.2],
      [95, 0.1], [96, 0.1], [97, 0.1], [98, 0.1], [100, 0.1],
    ];
    const result = lttbDownsampleByRange(data, 5);
    // The spike at 50 should be preserved because it creates maximum triangle area
    const hasSpike = result.some(([x, y]) => x === 50 && y === 0.9);
    expect(hasSpike).toBe(true);
  });

  it('handles single-point x-range (all same x) gracefully', () => {
    const data: [number, number][] = [[5, 0.1], [5, 0.2], [5, 0.3], [5, 0.4]];
    const result = lttbDownsampleByRange(data, 3);
    // When x-range is 0, returns full data (no meaningful bucketing possible)
    expect(result).toBe(data);
  });

  it('output is sorted by x', () => {
    const data: [number, number][] = Array.from({ length: 200 }, (_, i) => [
      i * 3 + 1,
      Math.random(),
    ]);
    const result = lttbDownsampleByRange(data, 20);
    for (let i = 1; i < result.length; i++) {
      expect(result[i][0]).toBeGreaterThanOrEqual(result[i - 1][0]);
    }
  });
});
