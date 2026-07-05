/**
 * LTTB (Largest Triangle Three Buckets) downsampling with x-range bucketization.
 *
 * Unlike ECharts' built-in LTTB which uses equal-count buckets by array index,
 * this implementation divides the position RANGE [min_x, max_x] into equal-width
 * intervals. Points are selected per bucket by largest triangle area using actual
 * x-values, preserving the shape of non-uniformly spaced data.
 *
 * Scientific motivation: When positions are filtered (e.g., only high-entropy
 * positions selected), the remaining data is non-uniformly spaced. Index-based
 * bucketing would distort the x-axis geometry, giving equal weight to dense and
 * sparse regions — misrepresenting the entropy landscape.
 *
 * Based on: Steinarsson 2013, "Downsampling Time Series for Visual Representation"
 * Range-bucket variant: minmaxlttb (Rust), ggalmazor/downsampling (Java)
 *
 * Performance: O(n) linear scan, sub-millisecond for typical DiMA datasets.
 */

/**
 * Triangle area using the shoelace formula for three points.
 * Returns the ABSOLUTE area (always positive) — higher area means the point
 * contributes more visual significance to the line shape.
 */
function triangleArea(
  ax: number, ay: number,
  bx: number, by: number,
  cx: number, cy: number,
): number {
  return Math.abs((ax - cx) * (by - ay) - (ax - bx) * (cy - ay)) * 0.5;
}

/**
 * Downsample a 2D dataset using LTTB with equal x-range buckets.
 *
 * @param data - Array of [x, y] tuples, MUST be sorted by x ascending
 * @param threshold - Target number of output points (minimum 3)
 * @returns Downsampled array preserving first/last points and visually
 *          significant peaks. Output length may be less than threshold
 *          if some buckets are empty (sparse data).
 */
export function lttbDownsampleByRange(
  data: [number, number][],
  threshold: number,
): [number, number][] {
  if (data.length <= threshold || threshold < 3) return data;

  const result: [number, number][] = [];

  // Always include first and last points
  result.push(data[0]);

  const xMin = data[0][0];
  const xMax = data[data.length - 1][0];
  const xRange = xMax - xMin;

  if (xRange === 0) return data;

  // Number of interior buckets (first and last points are fixed)
  const bucketCount = threshold - 2;
  const bucketWidth = xRange / bucketCount;

  // Pre-assign data points to their respective buckets.
  // Each bucket covers [xMin + i*bucketWidth, xMin + (i+1)*bucketWidth).
  // Using a single linear scan since data is sorted by x.
  const buckets: [number, number][][] = Array.from({ length: bucketCount }, () => []);
  let bucketIdx = 0;

  for (let i = 1; i < data.length - 1; i++) {
    // Advance bucket index to find correct bucket for this x
    while (
      bucketIdx < bucketCount - 1 &&
      data[i][0] >= xMin + (bucketIdx + 1) * bucketWidth
    ) {
      bucketIdx++;
    }
    buckets[bucketIdx].push(data[i]);
  }

  // For each non-empty bucket, select the point that forms the largest triangle
  // with the previously selected point and the average of the next bucket.
  let prevSelected = data[0];

  for (let i = 0; i < bucketCount; i++) {
    const bucket = buckets[i];
    if (bucket.length === 0) continue;

    // Compute the average point of the next non-empty bucket (or use last point)
    let avgX = data[data.length - 1][0];
    let avgY = data[data.length - 1][1];

    let foundNext = false;
    for (let j = i + 1; j < bucketCount; j++) {
      if (buckets[j].length > 0) {
        let sumX = 0, sumY = 0;
        for (const pt of buckets[j]) {
          sumX += pt[0];
          sumY += pt[1];
        }
        avgX = sumX / buckets[j].length;
        avgY = sumY / buckets[j].length;
        foundNext = true;
        break;
      }
    }
    if (!foundNext) {
      avgX = data[data.length - 1][0];
      avgY = data[data.length - 1][1];
    }

    // Find point in this bucket with largest triangle area
    let maxArea = -1;
    let bestPoint = bucket[0];
    for (const point of bucket) {
      const area = triangleArea(
        prevSelected[0], prevSelected[1],
        point[0], point[1],
        avgX, avgY,
      );
      if (area > maxArea) {
        maxArea = area;
        bestPoint = point;
      }
    }

    result.push(bestPoint);
    prevSelected = bestPoint;
  }

  result.push(data[data.length - 1]);
  return result;
}
