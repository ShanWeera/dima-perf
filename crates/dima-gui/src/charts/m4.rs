//! M4 downsampling for line charts (Jugel et al. 2014 [general], the primary source of the
//! algorithm): of the points falling into each pixel column, keep the first, the last, the
//! lowest and the highest. A line through the kept points rasterises to the same pixels as the
//! line through all points, so peaks and troughs are never lost, while at most four points per
//! pixel column are drawn. Used on screen and in PNG exports; SVG exports keep every point.

/// Indices (ascending) of the points of `points` (sorted by x) that M4 keeps when the x range
/// `[x0, x1]` is drawn `columns` pixels wide. Points outside the range are clamped into the
/// first or last column.
#[must_use]
pub fn m4_indices(points: &[(f64, f64)], x0: f64, x1: f64, columns: usize) -> Vec<usize> {
    let increasing = x1.partial_cmp(&x0) == Some(std::cmp::Ordering::Greater);
    if points.len() <= 4 || columns == 0 || !increasing {
        return (0..points.len()).collect();
    }
    let scale = columns as f64 / (x1 - x0);
    let column_of = |x: f64| {
        let c = ((x - x0) * scale).floor();
        // Clamped into 0..columns before the conversion.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let c = c.clamp(0.0, (columns - 1) as f64) as usize;
        c
    };
    let mut out = Vec::with_capacity(points.len().min(columns * 4));
    let mut current = column_of(points[0].0);
    let (mut first, mut last, mut lowest, mut highest) = (0, 0, 0, 0);
    let flush = |out: &mut Vec<usize>, first, lowest, highest, last| {
        let mut kept = [first, lowest, highest, last];
        kept.sort_unstable();
        for i in kept {
            if out.last() != Some(&i) {
                out.push(i);
            }
        }
    };
    for (i, &(x, y)) in points.iter().enumerate().skip(1) {
        let c = column_of(x);
        if c != current {
            flush(&mut out, first, lowest, highest, last);
            current = c;
            (first, last, lowest, highest) = (i, i, i, i);
            continue;
        }
        last = i;
        if y < points[lowest].1 {
            lowest = i;
        }
        if y > points[highest].1 {
            highest = i;
        }
    }
    flush(&mut out, first, lowest, highest, last);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn small_inputs_are_kept() {
        let p = [(1.0, 0.0), (2.0, 5.0), (3.0, 1.0)];
        assert_eq!(m4_indices(&p, 0.0, 4.0, 1), vec![0, 1, 2]);
        assert_eq!(m4_indices(&p, 4.0, 4.0, 10), vec![0, 1, 2]);
    }

    #[test]
    fn one_column_keeps_four_points() {
        let p: Vec<(f64, f64)> = (0..100)
            .map(|i| (f64::from(i), f64::from((i * 37) % 11)))
            .collect();
        let kept = m4_indices(&p, 0.0, 100.0, 1);
        assert!(kept.len() <= 4);
        assert_eq!(kept[0], 0);
        assert_eq!(*kept.last().unwrap(), 99);
    }

    proptest! {
        /// In every pixel column the extreme values, the first and the last point survive, and
        /// the output never exceeds four points per column.
        #[test]
        fn m4_keeps_extrema(
            ys in prop::collection::vec(-1000.0f64..1000.0, 1..600),
            columns in 1usize..200,
            spread in 0.1f64..5.0,
        ) {
            let points: Vec<(f64, f64)> =
                ys.iter().enumerate().map(|(i, &y)| (i as f64 * spread, y)).collect();
            let x0 = 0.0;
            let x1 = points.last().unwrap().0 + spread;
            let kept = m4_indices(&points, x0, x1, columns);
            prop_assert!(kept.windows(2).all(|w| w[0] < w[1]), "ascending, unique");
            prop_assert_eq!(kept[0], 0);
            prop_assert_eq!(*kept.last().unwrap(), points.len() - 1);
            if points.len() > 4 {
                prop_assert!(kept.len() <= 4 * columns);
                let scale = columns as f64 / (x1 - x0);
                let col = |x: f64| (((x - x0) * scale).floor()).clamp(0.0, (columns - 1) as f64) as usize;
                for c in 0..columns {
                    let in_col: Vec<usize> =
                        (0..points.len()).filter(|&i| col(points[i].0) == c).collect();
                    if in_col.is_empty() {
                        continue;
                    }
                    let max = in_col.iter().map(|&i| points[i].1).fold(f64::NEG_INFINITY, f64::max);
                    let min = in_col.iter().map(|&i| points[i].1).fold(f64::INFINITY, f64::min);
                    let kept_col: Vec<usize> = kept.iter().copied().filter(|&i| col(points[i].0) == c).collect();
                    prop_assert!(kept_col.iter().any(|&i| points[i].1 == max), "max of column {}", c);
                    prop_assert!(kept_col.iter().any(|&i| points[i].1 == min), "min of column {}", c);
                    prop_assert!(kept_col.contains(&in_col[0]), "first of column {}", c);
                    prop_assert!(kept_col.contains(in_col.last().unwrap()), "last of column {}", c);
                }
            }
        }
    }
}
