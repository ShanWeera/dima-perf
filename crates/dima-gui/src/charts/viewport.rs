//! Zoom and pan of a one-dimensional position axis, as pure functions of the current view and
//! the full extent (both `(lo, hi)` in data units). `None` means "show everything".
//!
//! Gestures follow genome browsers: scroll zooms about the cursor, drag pans, Shift-drag zooms
//! to a region, double-click resets. A view never gets narrower than [`MIN_SPAN`] positions and
//! snaps back to the full extent once it covers 99% of it, so it cannot get stuck almost
//! zoomed out.

/// The narrowest view, in positions.
pub const MIN_SPAN: f64 = 5.0;
/// Fraction of the full extent beyond which a view snaps to "everything".
const SNAP: f64 = 0.99;

/// A view of a full extent.
pub type Range = (f64, f64);

/// Clamps a view into the full extent, keeping its span; `None` when it covers almost all.
#[must_use]
pub fn normalise(view: Range, full: Range) -> Option<Range> {
    let full_span = full.1 - full.0;
    let positive = full_span.partial_cmp(&0.0) == Some(std::cmp::Ordering::Greater);
    if !positive || !view.0.is_finite() || !view.1.is_finite() {
        return None;
    }
    let span = (view.1 - view.0).clamp(MIN_SPAN.min(full_span), full_span);
    if span >= full_span * SNAP {
        return None;
    }
    let lo = view.0.clamp(full.0, full.1 - span);
    Some((lo, lo + span))
}

/// Zooms by `factor` (< 1 zooms in) keeping the data point at `anchor` (0 = left edge,
/// 1 = right edge of the view) fixed on screen.
#[must_use]
pub fn zoom(view: Option<Range>, full: Range, factor: f64, anchor: f64) -> Option<Range> {
    let (lo, hi) = view.unwrap_or(full);
    let anchor = anchor.clamp(0.0, 1.0);
    let x = lo + anchor * (hi - lo);
    let span = (hi - lo) * factor;
    normalise((x - anchor * span, x + (1.0 - anchor) * span), full)
}

/// Pans by `delta` data units (positive moves the view right).
#[must_use]
pub fn pan(view: Option<Range>, full: Range, delta: f64) -> Option<Range> {
    let (lo, hi) = view?;
    normalise((lo + delta, hi + delta), full)
}

/// Zooms to the region between `a` and `b`, unless it is narrower than [`MIN_SPAN`] (an
/// accidental micro-drag), in which case the view is unchanged.
#[must_use]
pub fn region(view: Option<Range>, full: Range, a: f64, b: f64) -> Option<Range> {
    let (lo, hi) = (a.min(b), a.max(b));
    if hi - lo < MIN_SPAN {
        return view;
    }
    normalise((lo, hi), full)
}

/// Re-centres a zoomed view on `x`, keeping its span; a full view stays full.
#[must_use]
pub fn center_on(view: Option<Range>, full: Range, x: f64) -> Option<Range> {
    let (lo, hi) = view?;
    let half = (hi - lo) / 2.0;
    normalise((x - half, x + half), full)
}

/// `true` when `x` is inside the shown range.
#[must_use]
pub fn shows(view: Option<Range>, full: Range, x: f64) -> bool {
    let (lo, hi) = view.unwrap_or(full);
    (lo..=hi).contains(&x)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: Range = (0.5, 100.5);

    #[test]
    fn zoom_keeps_the_anchor_fixed() {
        let v = zoom(None, FULL, 0.5, 0.25).expect("zoomed");
        let x_before = 0.5 + 0.25 * 100.0;
        let x_after = v.0 + 0.25 * (v.1 - v.0);
        assert!((x_before - x_after).abs() < 1e-9);
    }

    #[test]
    fn zooming_out_snaps_back_to_everything() {
        let mut v = Some((40.0, 60.0));
        for _ in 0..20 {
            v = zoom(v, FULL, 1.25, 0.5);
            if v.is_none() {
                return;
            }
        }
        panic!("never returned to the full range");
    }

    #[test]
    fn zoom_never_collapses_below_the_minimum() {
        let mut v = None;
        for _ in 0..60 {
            v = zoom(v, FULL, 0.5, 0.5);
            let (lo, hi) = v.expect("zoomed");
            assert!(hi - lo >= MIN_SPAN - 1e-9);
        }
    }

    #[test]
    fn views_stay_inside_the_extent() {
        let (lo, hi) = zoom(None, FULL, 0.5, 1.0).unwrap();
        assert!(lo >= FULL.0 && hi <= FULL.1 + 1e-9);
        let (lo, hi) = pan(Some((10.0, 20.0)), FULL, -1000.0).unwrap();
        assert_eq!((lo, hi), (0.5, 10.5));
        let (lo, hi) = pan(Some((10.0, 20.0)), FULL, 1000.0).unwrap();
        assert!((hi - 100.5).abs() < 1e-9 && (hi - lo - 10.0).abs() < 1e-9);
        assert_eq!(pan(None, FULL, 5.0), None, "nothing to pan at full view");
    }

    #[test]
    fn region_ignores_micro_drags() {
        assert_eq!(region(None, FULL, 10.0, 12.0), None);
        assert_eq!(
            region(Some((1.0, 50.0)), FULL, 10.0, 12.0),
            Some((1.0, 50.0))
        );
        assert_eq!(region(None, FULL, 30.0, 10.0), Some((10.0, 30.0)));
    }

    #[test]
    fn degenerate_extents() {
        // A single position: nothing to zoom into.
        assert_eq!(zoom(None, (6.5, 7.5), 0.5, 0.5), None);
        assert_eq!(normalise((f64::NAN, 3.0), FULL), None);
        assert_eq!(center_on(None, FULL, 50.0), None);
        let (lo, hi) = center_on(Some((0.5, 10.5)), FULL, 50.0).unwrap();
        assert!(((lo + hi) / 2.0 - 50.0).abs() < 1e-9);
        assert!(shows(None, FULL, 3.0));
        assert!(!shows(Some((10.0, 20.0)), FULL, 3.0));
    }
}
