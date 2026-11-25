//! Data ↔ screen mappings shared by the entropy chart, the conserved-region strip below it and
//! the exported figures, so all three always line up (G18).
//!
//! Positions are whole numbers but each occupies the column `[p − ½, p + ½]`, so a view of a
//! single position still has width (G29) and region bars cover their columns exactly.
//! Coordinates stay `f64` until the final conversion to screen space (G58).

/// The horizontal extent of positions `first..=last`: `[first − ½, last + ½]`.
#[must_use]
pub fn position_extent(first: u64, last: u64) -> (f64, f64) {
    // Positions are below 2^53: exact.
    #[allow(clippy::cast_precision_loss)]
    let (a, b) = (first as f64, last as f64);
    (a - 0.5, b + 0.5)
}

/// Maps data x (positions) to screen x and back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XMap {
    /// Data value at the left edge.
    pub lo: f64,
    /// Data value at the right edge.
    pub hi: f64,
    /// Screen x of the left edge.
    pub left: f32,
    /// Screen x of the right edge.
    pub right: f32,
}

impl XMap {
    /// Width in screen units.
    #[must_use]
    pub fn width(&self) -> f32 {
        (self.right - self.left).max(0.0)
    }

    /// Data span.
    #[must_use]
    pub fn span(&self) -> f64 {
        (self.hi - self.lo).max(f64::MIN_POSITIVE)
    }

    /// Screen x of data x (not clamped).
    #[must_use]
    pub fn to_screen(self, x: f64) -> f32 {
        let t = (x - self.lo) / self.span();
        // Screen coordinates are f32 by nature; the fraction is computed in f64 first.
        #[allow(clippy::cast_possible_truncation)]
        let t = t as f32;
        self.left + t * self.width()
    }

    /// Data x of screen x, clamped to the mapped range.
    #[must_use]
    pub fn to_data(self, sx: f32) -> f64 {
        let w = self.width();
        if w <= 0.0 {
            return self.lo;
        }
        let t = f64::from(((sx - self.left) / w).clamp(0.0, 1.0));
        self.lo + t * self.span()
    }

    /// Data units per screen unit.
    #[must_use]
    pub fn data_per_pixel(&self) -> f64 {
        let w = f64::from(self.width());
        if w > 0.0 {
            self.span() / w
        } else {
            self.span()
        }
    }
}

/// Maps entropy (bits) to screen y: 0 at the bottom, `max` at the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YMap {
    /// Value at the top edge (> 0).
    pub max: f64,
    /// Screen y of the top edge.
    pub top: f32,
    /// Screen y of the bottom edge.
    pub bottom: f32,
}

impl YMap {
    /// Screen y of a value.
    #[must_use]
    pub fn to_screen(self, y: f64) -> f32 {
        let t = if self.max > 0.0 { y / self.max } else { 0.0 };
        #[allow(clippy::cast_possible_truncation)]
        let t = t as f32;
        self.bottom - t * (self.bottom - self.top)
    }
}

/// The shortest value axis, in bits: its ticks are then at least 0.001 apart, so three
/// decimals always tell them apart and every label fits in five characters.
pub const MIN_Y_AXIS_MAX: f64 = 0.005;

/// A y-axis maximum for data whose largest value is `max`: 5% above it and at least
/// [`MIN_Y_AXIS_MAX`], or 1 bit when every value is 0 (a fully conserved stretch is drawn as a
/// flat line, not hidden).
#[must_use]
pub fn y_axis_max(max: Option<f64>) -> f64 {
    match max {
        Some(m) if m.is_finite() && m > 0.0 => (m * 1.05).max(MIN_Y_AXIS_MAX),
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_extent() {
        let (lo, hi) = position_extent(1, 100);
        assert_eq!((lo, hi), (0.5, 100.5));
        let m = XMap {
            lo,
            hi,
            left: 50.0,
            right: 1050.0,
        };
        assert!((m.to_screen(0.5) - 50.0).abs() < 1e-4);
        assert!((m.to_screen(100.5) - 1050.0).abs() < 1e-3);
        assert!((m.to_data(550.0) - 50.5).abs() < 1e-9);
        assert_eq!(m.to_data(-10.0), 0.5, "clamped");
        assert!((m.data_per_pixel() - 0.1).abs() < 1e-12);
        // A single position still has a width.
        let (a, b) = position_extent(7, 7);
        assert_eq!(b - a, 1.0);
    }

    #[test]
    fn large_positions_keep_precision() {
        // f32 cannot represent 16_777_217; the mapping is computed in f64 first (G58).
        let m = XMap {
            lo: 16_777_216.5,
            hi: 16_777_226.5,
            left: 0.0,
            right: 1000.0,
        };
        let a = m.to_screen(16_777_217.0);
        let b = m.to_screen(16_777_218.0);
        assert!((b - a - 100.0).abs() < 0.01, "{a} {b}");
    }

    #[test]
    fn y_mapping() {
        let y = YMap {
            max: 2.0,
            top: 10.0,
            bottom: 110.0,
        };
        assert!((y.to_screen(0.0) - 110.0).abs() < 1e-6);
        assert!((y.to_screen(2.0) - 10.0).abs() < 1e-6);
        assert_eq!(y_axis_max(None), 1.0);
        assert_eq!(
            y_axis_max(Some(0.0)),
            1.0,
            "all-zero data keeps a 1-bit axis"
        );
        assert!((y_axis_max(Some(2.0)) - 2.1).abs() < 1e-12);
        assert_eq!(y_axis_max(Some(1e-6)), MIN_Y_AXIS_MAX);
        assert_eq!(
            y_axis_max(Some(f64::from_bits(1))),
            MIN_Y_AXIS_MAX,
            "subnormal"
        );
        assert!((y_axis_max(Some(0.005)) - 0.00525).abs() < 1e-15);
        let flat = YMap {
            max: 0.0,
            top: 0.0,
            bottom: 10.0,
        };
        assert!((flat.to_screen(3.0) - 10.0).abs() < 1e-6);
    }
}
