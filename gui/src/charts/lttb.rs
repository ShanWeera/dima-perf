//! LTTB (Largest Triangle Three Buckets) downsampling with x-range bucketization.
//!
//! Ported from `ui/src/lib/lttb.ts`. Unlike ECharts' built-in LTTB which uses
//! equal-count buckets by array index, this implementation divides the position
//! RANGE [min_x, max_x] into equal-width intervals. Points are selected per
//! bucket by largest triangle area using actual x-values, preserving the shape
//! of non-uniformly spaced data.
//!
//! Scientific motivation: When positions are filtered (e.g., only high-entropy
//! positions selected), the remaining data is non-uniformly spaced. Index-based
//! bucketing would distort the x-axis geometry.
//!
//! Based on: Steinarsson 2013, "Downsampling Time Series for Visual Representation"
//! Performance: O(n) linear scan, sub-millisecond for typical DiMA datasets.

/// A 2D point for downsampling.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Triangle area using the shoelace formula for three points.
/// Returns the ABSOLUTE area (always positive) — higher area means the point
/// contributes more visual significance to the line shape.
fn triangle_area(a: Point, b: Point, c: Point) -> f64 {
    ((a.x - c.x) * (b.y - a.y) - (a.x - b.x) * (c.y - a.y)).abs() * 0.5
}

/// Downsample a 2D dataset using LTTB with equal x-range buckets.
///
/// # Arguments
/// * `data` - Slice of [Point], MUST be sorted by x ascending (precondition)
/// * `threshold` - Target number of output points (minimum 3)
///
/// # Returns
/// Downsampled Vec preserving first/last points and visually significant peaks.
/// Output length may be less than threshold if some buckets are empty (sparse data).
///
/// # Edge cases
/// - `threshold < 3` or `data.len() <= threshold`: returns data unchanged
/// - `x_range == 0` (all points at same x): returns data unchanged
/// - Empty buckets: skipped (no output point), may produce fewer than threshold points
pub fn lttb_downsample_by_range(data: &[Point], threshold: usize) -> Vec<Point> {
    if data.len() <= threshold || threshold < 3 {
        return data.to_vec();
    }

    let x_min = data[0].x;
    let x_max = data[data.len() - 1].x;
    let x_range = x_max - x_min;

    if x_range == 0.0 {
        return data.to_vec();
    }

    let mut result: Vec<Point> = Vec::with_capacity(threshold);

    // Always include first point
    result.push(data[0]);

    // Number of interior buckets (first and last points are fixed)
    let bucket_count = threshold - 2;
    let bucket_width = x_range / bucket_count as f64;

    // Pre-assign data points to their respective buckets.
    // Each bucket covers [x_min + i*bucket_width, x_min + (i+1)*bucket_width).
    // Single linear scan since data is sorted by x.
    let mut buckets: Vec<Vec<Point>> = vec![Vec::new(); bucket_count];
    let mut bucket_idx: usize = 0;

    for point in &data[1..data.len() - 1] {
        // Advance bucket index to find correct bucket for this x
        while bucket_idx < bucket_count - 1
            && point.x >= x_min + (bucket_idx + 1) as f64 * bucket_width
        {
            bucket_idx += 1;
        }
        buckets[bucket_idx].push(*point);
    }

    // For each non-empty bucket, select the point that forms the largest triangle
    // with the previously selected point and the centroid of the next non-empty bucket.
    let mut prev_selected = data[0];

    for i in 0..bucket_count {
        let bucket = &buckets[i];
        if bucket.is_empty() {
            continue;
        }

        // Compute the average point of the next non-empty bucket (or use last point)
        let (avg_x, avg_y) = find_next_bucket_centroid(&buckets, i, data);

        // Find point in this bucket with largest triangle area
        let mut max_area: f64 = -1.0;
        let mut best_point = bucket[0];

        for &point in bucket {
            let area = triangle_area(prev_selected, point, Point { x: avg_x, y: avg_y });
            if area > max_area {
                max_area = area;
                best_point = point;
            }
        }

        result.push(best_point);
        prev_selected = best_point;
    }

    // Always include last point
    result.push(data[data.len() - 1]);

    result
}

/// Find the centroid of the next non-empty bucket after index `current_bucket`.
/// Falls back to the last data point if no non-empty bucket follows.
fn find_next_bucket_centroid(
    buckets: &[Vec<Point>],
    current_bucket: usize,
    data: &[Point],
) -> (f64, f64) {
    for bucket in buckets.iter().skip(current_bucket + 1) {
        if !bucket.is_empty() {
            let sum_x: f64 = bucket.iter().map(|p| p.x).sum();
            let sum_y: f64 = bucket.iter().map(|p| p.y).sum();
            let count = bucket.len() as f64;
            return (sum_x / count, sum_y / count);
        }
    }
    // No non-empty bucket found — use last data point
    let last = data[data.len() - 1];
    (last.x, last.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn test_threshold_less_than_3_returns_unchanged() {
        let data = vec![pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)];
        let result = lttb_downsample_by_range(&data, 2);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_data_shorter_than_threshold_returns_unchanged() {
        let data = vec![pt(0.0, 0.0), pt(1.0, 1.0)];
        let result = lttb_downsample_by_range(&data, 5);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_x_range_zero_returns_unchanged() {
        let data = vec![pt(5.0, 0.0), pt(5.0, 1.0), pt(5.0, 2.0), pt(5.0, 3.0)];
        let result = lttb_downsample_by_range(&data, 3);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_preserves_first_and_last() {
        let data: Vec<Point> = (0..100).map(|i| pt(i as f64, (i as f64).sin())).collect();
        let result = lttb_downsample_by_range(&data, 10);
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result.last().unwrap().x, 99.0);
    }

    #[test]
    fn test_output_length_at_most_threshold() {
        let data: Vec<Point> = (0..1000).map(|i| pt(i as f64, (i as f64).sin())).collect();
        let result = lttb_downsample_by_range(&data, 50);
        assert!(result.len() <= 50);
    }

    #[test]
    fn test_sparse_data_fewer_than_threshold() {
        // Data clustered in two regions with a huge gap — some buckets will be empty
        let mut data = Vec::new();
        for i in 0..10 {
            data.push(pt(i as f64, 1.0));
        }
        for i in 990..1000 {
            data.push(pt(i as f64, 2.0));
        }
        let result = lttb_downsample_by_range(&data, 50);
        // Output should be shorter than 50 since most interior buckets are empty
        assert!(result.len() < 50);
        assert!(result.len() >= 3); // At least first, last, and some interior
    }

    #[test]
    fn test_empty_input() {
        let result = lttb_downsample_by_range(&[], 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_point() {
        let data = vec![pt(1.0, 2.0)];
        let result = lttb_downsample_by_range(&data, 10);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_triangle_area_calculation() {
        let area = triangle_area(pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 0.0));
        assert!((area - 1.0).abs() < 1e-10);
    }
}
