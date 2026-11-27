//! What the entropy chart draws (D87) for the metric it plots (entropy, or similarity-weighted
//! entropy when the run computed it), derived once per dataset, filter and metric change.
//!
//! * **Eligible** positions (support > T): solid Okabe–Ito blue; with the paper's estimator
//!   their entropy is corrected where possible (support of at least 1.25 T gives the five
//!   regression points DiMA v5.0.9 requires; below that it is the observed entropy).
//! * **LS/ELS** positions (0 < support ≤ T): dashed vermillion; with the paper's estimator they
//!   carry the *uncorrected* (observed) entropy and are labelled "uncorrected, support ≤ T"
//!   (other estimators estimate every position with at least two sequences, so only the
//!   support differs; no similarity-weighted value is corrected, and the label says that they
//!   are not in its summary). ELS is grouped with LS (G43).
//! * **NS** positions (support 0) have no value of either metric (D34/D56): they become hatched
//!   full-height bands labelled "no usable support", never a value of 0 (Kent 2010; Kerpedjiev
//!   2018: missing values are not zeros). The band means support 0 and nothing else: a
//!   supported position without a value (only in an edited file) just breaks the line.
//!
//! The line is one polyline per run of consecutive shown positions with a value; it breaks at
//! positions without one and wherever filtering removed positions in between (G45), so it never
//! bridges a gap.

use dima_core::config::Estimator;
use dima_core::model::{EntropyMethod, PositionResult, Results, SupportLabel};

use super::m4::m4_indices;
use super::metric::ChartMetric;

/// Line style of a point (and of the segments touching it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// Support > T: solid (corrected where possible with the paper's estimator).
    Eligible,
    /// Support ≤ T (LS or ELS): dashed (observed entropy with the paper's estimator).
    LowSupport,
}

/// Class of a position for display (D87, G43).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    /// Support > T.
    Eligible,
    /// 0 < support < T.
    Low,
    /// Support = T.
    EqualToThreshold,
    /// Support 0.
    NoSupport,
}

impl Class {
    /// The class of a position.
    #[must_use]
    pub fn of(p: &PositionResult) -> Self {
        match p.low_support {
            None => Self::Eligible,
            Some(SupportLabel::Low) => Self::Low,
            Some(SupportLabel::EqualToThreshold) => Self::EqualToThreshold,
            Some(SupportLabel::NoSupport) => Self::NoSupport,
        }
    }

    /// Short label: `eligible`, `LS`, `ELS`, `NS`.
    #[must_use]
    pub const fn short(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::Low => "LS",
            Self::EqualToThreshold => "ELS",
            Self::NoSupport => "NS",
        }
    }

    /// What the class means for the shown entropy, with the threshold, the run's estimator and
    /// the method that gave the position's entropy.
    #[must_use]
    pub fn describe(self, t: u64, estimator: Estimator, method: EntropyMethod) -> String {
        match self {
            Self::Eligible => format!(
                "eligible (support > {t}): {}",
                entropy_kind(estimator, method, true)
            ),
            Self::Low => format!(
                "LS, low support (support < {t}): {}",
                entropy_kind(estimator, method, false)
            ),
            Self::EqualToThreshold => format!(
                "ELS, support = {t}: {}",
                entropy_kind(estimator, method, false)
            ),
            Self::NoSupport => "NS, no usable support: no entropy".to_owned(),
        }
    }

    /// The line style, `None` for NS (drawn as a band).
    #[must_use]
    pub const fn style(self) -> Option<Style> {
        match self {
            Self::Eligible => Some(Style::Eligible),
            Self::Low | Self::EqualToThreshold => Some(Style::LowSupport),
            Self::NoSupport => None,
        }
    }
}

/// The entropy a position shows, from the method that gave it (`eligible`: support > T). The
/// paper's estimator corrects eligible positions where possible and reports the observed
/// entropy at the others (Tharanga et al. 2025, Fig. 2); Chao–Wang–Jost and Miller–Madow
/// estimate every position with at least 2 sequences and keep the observed entropy (0 bits) at
/// a position with one (`single_observation`); raw is the observed entropy everywhere.
const fn entropy_kind(estimator: Estimator, method: EntropyMethod, eligible: bool) -> &'static str {
    match (method, estimator) {
        (EntropyMethod::Dima, _) => "corrected entropy",
        (EntropyMethod::ChaoWangJost, _) => "Chao\u{2013}Wang\u{2013}Jost entropy",
        (EntropyMethod::MillerMadow, _) => "Miller\u{2013}Madow entropy",
        (EntropyMethod::NotEstimable, _) => "no entropy",
        // Too few subsample sizes (support below 1.25 T) leave an eligible position
        // uncorrected.
        (EntropyMethod::PlugIn, Estimator::Dima) if eligible => {
            "observed entropy (the correction did not apply)"
        }
        (EntropyMethod::PlugIn, Estimator::Dima) => "uncorrected entropy",
        (EntropyMethod::PlugIn, Estimator::Raw) => "observed entropy",
        (EntropyMethod::PlugIn, Estimator::ChaoWangJost | Estimator::MillerMadow) => {
            "observed entropy (a single sequence)"
        }
    }
}

/// Legend text of the solid and the dashed line (D87), shared by the screen and the exports.
/// Similarity-weighted entropy is never corrected, so its legend names only the support and
/// that low-support positions are left out of its summary.
#[must_use]
pub fn legend(t: u64, estimator: Estimator, metric: ChartMetric) -> [String; 2] {
    match (metric, estimator) {
        (ChartMetric::SimilarityWeighted, _) => [
            format!("support > {t}"),
            format!("support \u{2264} {t}, not in the summary"),
        ],
        (ChartMetric::Entropy, Estimator::Dima) => [
            format!("support > {t}: corrected where possible"),
            format!("support \u{2264} {t}: uncorrected"),
        ],
        (
            ChartMetric::Entropy,
            Estimator::ChaoWangJost | Estimator::MillerMadow | Estimator::Raw,
        ) => [
            format!("support > {t}: eligible"),
            format!("support \u{2264} {t}: low support"),
        ],
    }
}

/// A plotted point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Position.
    pub x: f64,
    /// The plotted value, in bits.
    pub y: f64,
    /// Line style.
    pub style: Style,
    /// Index into `Results::results`.
    pub index: usize,
}

/// Consecutive shown positions with a value (no position without one and no filtered-out gap
/// inside).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// Points in position order.
    pub points: Vec<Point>,
}

/// A run of consecutive shown NS positions, `first..=last`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NsSpan {
    /// First position.
    pub first: u64,
    /// Last position.
    pub last: u64,
}

impl NsSpan {
    /// `true` when the band (its positions ± half a column) overlaps the view `lo..=hi`.
    #[must_use]
    pub fn in_view(self, lo: f64, hi: f64) -> bool {
        #[allow(clippy::cast_precision_loss)]
        let (a, b) = (self.first as f64 - 0.5, self.last as f64 + 0.5);
        b >= lo && a <= hi
    }
}

/// Everything the chart shows for one dataset, filter state and metric.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Series {
    /// Line paths.
    pub paths: Vec<Path>,
    /// No-support bands.
    pub ns_spans: Vec<NsSpan>,
    /// Shown positions whose sample coverage is below the run's `--min-coverage`.
    pub low_coverage: Vec<u64>,
    /// Largest shown value of the metric.
    pub y_max: Option<f64>,
    /// First and last shown position.
    pub extent: Option<(u64, u64)>,
    /// Eligible points, counted once by [`Series::build`] (the legends and the chart's
    /// description read the counts every frame).
    eligible_points: usize,
    /// Low-support points, counted alike.
    low_support_points: usize,
}

impl Series {
    /// Builds the series of `metric` from the shown positions (`shown` indexes
    /// `results.results`, in position order).
    #[must_use]
    pub fn build(results: &Results, shown: &[usize], metric: ChartMetric) -> Self {
        let mut s = Self::default();
        let mut path = Path::default();
        let mut previous: Option<u64> = None;
        for &i in shown {
            let Some(p) = results.results.get(i) else {
                continue;
            };
            let contiguous = previous.and_then(|q| q.checked_add(1)) == Some(p.position);
            previous = Some(p.position);
            s.extent = Some(match s.extent {
                None => (p.position, p.position),
                Some((a, _)) => (a, p.position),
            });
            if p.below_min_coverage == Some(true) {
                s.low_coverage.push(p.position);
            }
            let Some(style) = Class::of(p).style() else {
                // NS: a band, and a break.
                match s.ns_spans.last_mut() {
                    Some(span) if contiguous && span.last.checked_add(1) == Some(p.position) => {
                        span.last = p.position;
                    }
                    _ => s.ns_spans.push(NsSpan {
                        first: p.position,
                        last: p.position,
                    }),
                }
                if !path.points.is_empty() {
                    s.paths.push(std::mem::take(&mut path));
                }
                continue;
            };
            let Some(y) = metric.value(p) else {
                // Supported but without a value (an edited file): a break, not a band, which
                // would say "no usable support".
                if !path.points.is_empty() {
                    s.paths.push(std::mem::take(&mut path));
                }
                continue;
            };
            if !contiguous && !path.points.is_empty() {
                s.paths.push(std::mem::take(&mut path));
            }
            s.y_max = Some(s.y_max.map_or(y, |m: f64| m.max(y)));
            match style {
                Style::Eligible => s.eligible_points += 1,
                Style::LowSupport => s.low_support_points += 1,
            }
            #[allow(clippy::cast_precision_loss)]
            path.points.push(Point {
                x: p.position as f64,
                y,
                style,
                index: i,
            });
        }
        if !path.points.is_empty() {
            s.paths.push(path);
        }
        s
    }

    /// `true` when there is nothing to draw.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.ns_spans.is_empty()
    }

    /// Number of shown points of a style.
    #[must_use]
    pub const fn count(&self, style: Style) -> usize {
        match style {
            Style::Eligible => self.eligible_points,
            Style::LowSupport => self.low_support_points,
        }
    }

    /// `true` when some shown point has the style (its legend entry applies).
    #[must_use]
    pub const fn has(&self, style: Style) -> bool {
        self.count(style) > 0
    }
}

/// A polyline in one style.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// Style of every segment.
    pub style: Style,
    /// Points in data space.
    pub points: Vec<(f64, f64)>,
}

/// What to draw for the view `[lo, hi]` at `columns` pixels: the strokes (M4-reduced unless
/// `columns` is `None`) and the isolated single points (G29: drawn as markers).
///
/// A segment is solid only when both its ends are eligible; any segment touching a low-support
/// value is dashed. Strokes of different styles share their boundary point, so the line is
/// continuous and the class boundaries survive downsampling.
#[must_use]
pub fn strokes(
    series: &Series,
    lo: f64,
    hi: f64,
    columns: Option<usize>,
) -> (Vec<Stroke>, Vec<Point>) {
    let mut strokes = Vec::new();
    let mut markers = Vec::new();
    for path in &series.paths {
        let pts = &path.points;
        // Visible slice plus one neighbour on each side, so the line reaches the edges.
        let start = pts.partition_point(|p| p.x < lo).saturating_sub(1);
        let end = (pts.partition_point(|p| p.x <= hi) + 1).min(pts.len());
        if start >= end {
            continue;
        }
        let visible = &pts[start..end];
        if pts.len() == 1 {
            if (lo..=hi).contains(&pts[0].x) {
                markers.push(pts[0]);
            }
            continue;
        }
        let mut i = 0;
        while i + 1 < visible.len() {
            let style = segment_style(visible[i], visible[i + 1]);
            let mut j = i + 1;
            while j + 1 < visible.len() && segment_style(visible[j], visible[j + 1]) == style {
                j += 1;
            }
            let run: Vec<(f64, f64)> = visible[i..=j].iter().map(|p| (p.x, p.y)).collect();
            let points = match columns {
                Some(c) => m4_indices(&run, lo, hi, c)
                    .into_iter()
                    .map(|k| run[k])
                    .collect(),
                None => run,
            };
            strokes.push(Stroke { style, points });
            i = j;
        }
    }
    (strokes, markers)
}

fn segment_style(a: Point, b: Point) -> Style {
    if a.style == Style::Eligible && b.style == Style::Eligible {
        Style::Eligible
    } else {
        Style::LowSupport
    }
}

/// Identifies the lines the chart draws: what [`strokes`] returns, and the GPU vertices built
/// from it. Compared exactly (no hash), so lines are never reused for other data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChartKey {
    session: u64,
    version: u64,
    view: (u64, u64),
    columns: usize,
}

/// The key of the lines of a session's series: the session id (unique per load, G23; a new
/// load restarts the version, so the version alone cannot tell two results apart), the
/// session's version (bumped whenever its series is rebuilt: by a filter, and by a metric switch
/// even when the same positions are shown, so the lines of one metric are never drawn for the
/// other), the view `(lo, hi)` and the pixel columns the lines are reduced to. The value range
/// is not part of it: strokes and vertices are in data units, and the axis maximum is only a
/// uniform of the line shader.
#[must_use]
pub fn chart_cache_key(session: u64, version: u64, view: (f64, f64), columns: usize) -> ChartKey {
    ChartKey {
        session,
        version,
        view: (view.0.to_bits(), view.1.to_bits()),
        columns,
    }
}

/// The point nearest to data x among the shown points (ties: the lower position).
#[must_use]
pub fn nearest_point(series: &Series, x: f64) -> Option<Point> {
    let mut best: Option<Point> = None;
    for path in &series.paths {
        let pts = &path.points;
        let k = pts.partition_point(|p| p.x < x);
        for cand in [k.checked_sub(1), Some(k)].into_iter().flatten() {
            if let Some(&p) = pts.get(cand) {
                let better = best.is_none_or(|b| {
                    let (d, db) = ((p.x - x).abs(), (b.x - x).abs());
                    d < db || (d == db && p.x < b.x)
                });
                if better {
                    best = Some(p);
                }
            }
        }
    }
    best
}

/// The NS span containing data x (within its columns `[first − ½, last + ½]`).
#[must_use]
pub fn ns_span_at(series: &Series, x: f64) -> Option<NsSpan> {
    let k = series.ns_spans.partition_point(|s| {
        #[allow(clippy::cast_precision_loss)]
        let end = s.last as f64 + 0.5;
        end < x
    });
    let s = *series.ns_spans.get(k)?;
    #[allow(clippy::cast_precision_loss)]
    let start = s.first as f64 - 0.5;
    (x >= start).then_some(s)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use dima_core::model::{Motif, Variant};

    /// A position for chart tests: `None` entropy means NS.
    pub(crate) fn position(pos: u64, entropy: Option<f64>, support: u64, t: u64) -> PositionResult {
        let low_support = if support == 0 {
            Some(SupportLabel::NoSupport)
        } else if support < t {
            Some(SupportLabel::Low)
        } else if support == t {
            Some(SupportLabel::EqualToThreshold)
        } else {
            None
        };
        PositionResult {
            position: pos,
            low_support,
            support,
            distinct_variants_count: 0,
            entropy,
            observed_entropy: entropy,
            entropy_method: if support == 0 {
                EntropyMethod::NotEstimable
            } else if support > t {
                EntropyMethod::Dima
            } else {
                EntropyMethod::PlugIn
            },
            entropy_fallback: None,
            distinct_variants_incidence: None,
            total_variants_incidence: if support == 0 { None } else { Some(0.0) },
            sample_coverage: None,
            below_min_coverage: None,
            similarity_weighted_entropy: None,
            normalised_observed_entropy: None,
            normalised_similarity_weighted_entropy: None,
            diversity_motifs: if support == 0 {
                Vec::new()
            } else {
                vec![Variant {
                    sequence: "AAA".into(),
                    count: support,
                    incidence: 100.0,
                    motif: if support > 1 {
                        Motif::Index
                    } else {
                        Motif::Unique
                    },
                    metadata: None,
                }]
            },
        }
    }

    pub(crate) fn results_of(positions: Vec<PositionResult>) -> Results {
        let mut r = crate::tests::fixtures::results(3);
        r.results = positions;
        r
    }

    #[test]
    fn descriptions_follow_the_method_used() {
        let eligible = |m| Class::Eligible.describe(100, Estimator::Dima, m);
        assert!(eligible(EntropyMethod::Dima).ends_with(": corrected entropy"));
        assert!(eligible(EntropyMethod::PlugIn).contains("the correction did not apply"));
        let ls = Class::Low.describe(100, Estimator::Dima, EntropyMethod::PlugIn);
        assert!(ls.ends_with("uncorrected entropy"), "{ls}");
        let mm = Class::Eligible.describe(100, Estimator::MillerMadow, EntropyMethod::MillerMadow);
        assert!(mm.contains("Miller"), "{mm}");
        // Chao–Wang–Jost and Miller–Madow estimate low-support positions too, but a position
        // with a single sequence keeps its observed entropy (single_observation): it is never
        // called an estimate.
        for e in [Estimator::ChaoWangJost, Estimator::MillerMadow] {
            for class in [Class::Low, Class::EqualToThreshold] {
                let single = class.describe(1, e, EntropyMethod::PlugIn);
                assert!(
                    single.ends_with(": observed entropy (a single sequence)"),
                    "{single}"
                );
            }
        }
        assert_eq!(
            Class::Low.describe(100, Estimator::ChaoWangJost, EntropyMethod::ChaoWangJost),
            "LS, low support (support < 100): Chao\u{2013}Wang\u{2013}Jost entropy"
        );
        assert_eq!(
            Class::EqualToThreshold.describe(1, Estimator::MillerMadow, EntropyMethod::PlugIn),
            "ELS, support = 1: observed entropy (a single sequence)"
        );
        assert_eq!(
            Class::Low.describe(100, Estimator::Raw, EntropyMethod::PlugIn),
            "LS, low support (support < 100): observed entropy"
        );
        assert_eq!(
            Class::NoSupport.describe(100, Estimator::Dima, EntropyMethod::NotEstimable),
            "NS, no usable support: no entropy"
        );
    }

    /// As the library writes them: with Miller–Madow (or Chao–Wang–Jost) a position supported
    /// by two sequences is estimated, one supported by a single sequence keeps its observed
    /// entropy (`plug_in`, `single_observation`), and the class line says which.
    #[test]
    fn a_single_sequence_with_miller_madow_is_observed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("three.fa");
        // Position 1 (ACD): two valid 3-mers; position 3 (DEF): one.
        std::fs::write(&path, ">a\nACDEF\n>b\nACDE-\n>c\nAC---\n").unwrap();
        for estimator in [Estimator::MillerMadow, Estimator::ChaoWangJost] {
            let params = dima_core::config::AnalysisParams {
                alphabet: Some(dima_core::config::Alphabet::Protein),
                kmer_length: Some(3),
                estimator: Some(estimator),
                ..dima_core::config::AnalysisParams::default()
            };
            let r = dima_core::analyze(
                &dima_core::input::Source::Path(path.clone()),
                &params,
                &dima_core::control::RunControl::new(),
            )
            .unwrap()
            .results;
            let describe = |position: u64| {
                let p = &r.results[usize::try_from(position).unwrap() - 1];
                Class::of(p).describe(r.support_threshold, estimator, p.entropy_method)
            };
            let single = &r.results[2];
            assert_eq!(single.support, 1);
            assert_eq!(single.entropy_method, EntropyMethod::PlugIn);
            assert_eq!(
                single.entropy_fallback,
                Some(dima_core::model::EntropyFallback::SingleObservation)
            );
            assert_eq!(
                describe(3),
                "LS, low support (support < 100): observed entropy (a single sequence)"
            );
            assert_eq!(r.results[0].support, 2);
            let name = match estimator {
                Estimator::MillerMadow => "Miller\u{2013}Madow",
                _ => "Chao\u{2013}Wang\u{2013}Jost",
            };
            assert_eq!(
                describe(1),
                format!("LS, low support (support < 100): {name} entropy")
            );
        }
    }

    #[test]
    fn spans_in_view_include_their_half_columns() {
        let span = NsSpan {
            first: 10,
            last: 12,
        };
        assert!(span.in_view(0.0, 9.5), "touches the view's right edge");
        assert!(!span.in_view(0.0, 9.4));
        assert!(span.in_view(12.5, 40.0), "touches the view's left edge");
        assert!(!span.in_view(12.6, 40.0));
        assert!(span.in_view(11.0, 11.0));
    }

    #[test]
    fn classes_styles_and_breaks() {
        // 1-2 eligible, 3 LS, 4 NS, 5-6 eligible, (7 filtered out), 8 ELS.
        let r = results_of(vec![
            position(1, Some(0.1), 200, 100),
            position(2, Some(0.2), 200, 100),
            position(3, Some(0.3), 50, 100),
            position(4, None, 0, 100),
            position(5, Some(0.5), 200, 100),
            position(6, Some(0.6), 200, 100),
            position(7, Some(0.7), 200, 100),
            position(8, Some(0.8), 100, 100),
        ]);
        let s = Series::build(&r, &[0, 1, 2, 3, 4, 5, 7], ChartMetric::Entropy);
        assert_eq!(s.ns_spans, vec![NsSpan { first: 4, last: 4 }]);
        assert_eq!(s.paths.len(), 3, "breaks at NS and at the filtered gap");
        assert_eq!(s.paths[0].points.len(), 3);
        assert_eq!(
            s.paths[2].points[0].style,
            Style::LowSupport,
            "ELS is low support"
        );
        assert_eq!(s.extent, Some((1, 8)));
        assert_eq!(s.count(Style::LowSupport), 2);
        assert!((s.y_max.unwrap() - 0.8).abs() < 1e-12);

        let (strokes, markers) = strokes(&s, 0.5, 8.5, None);
        // Path 1: solid 1→2, dashed 2→3; path 2: solid 5→6; path 3: a single ELS marker.
        assert_eq!(strokes.len(), 3);
        assert_eq!(strokes[0].style, Style::Eligible);
        assert_eq!(strokes[0].points, vec![(1.0, 0.1), (2.0, 0.2)]);
        assert_eq!(strokes[1].style, Style::LowSupport);
        assert_eq!(
            strokes[1].points[0],
            (2.0, 0.2),
            "styles share the boundary point"
        );
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].x, 8.0);
    }

    proptest::proptest! {
        /// The per-style counts that `build` stores equal a scan of every shown point.
        #[test]
        fn stored_counts_equal_a_full_scan(
            supports in proptest::collection::vec(0u64..4, 0..120),
            hidden in proptest::collection::vec(proptest::bool::weighted(0.2), 120),
        ) {
            // Support 0 is NS, 1 LS, 2 ELS (T = 2) and 3 eligible.
            let r = results_of(
                supports
                    .iter()
                    .zip(1u64..)
                    .map(|(&s, p)| position(p, (s > 0).then_some(0.5), s, 2))
                    .collect(),
            );
            let shown: Vec<usize> = (0..supports.len()).filter(|&i| !hidden[i]).collect();
            let s = Series::build(&r, &shown, ChartMetric::Entropy);
            for style in [Style::Eligible, Style::LowSupport] {
                let scan = s.paths.iter().flat_map(|p| &p.points).filter(|p| p.style == style).count();
                proptest::prop_assert_eq!(s.count(style), scan);
                proptest::prop_assert_eq!(s.has(style), scan > 0);
            }
        }
    }

    /// The band means support 0, for either metric: a supported position without a value
    /// (an edited file's `null`) breaks the line but is no band, and a position without
    /// support is a band whatever its values say.
    #[test]
    fn bands_mark_no_support_and_missing_values_only_break_the_line() {
        let swe = |p: PositionResult, v: Option<f64>| {
            let mut p = p;
            p.similarity_weighted_entropy = v;
            p
        };
        let r = results_of(vec![
            swe(position(1, Some(0.4), 200, 100), Some(0.2)),
            // Supported, but its values were removed.
            swe(position(2, None, 200, 100), None),
            swe(position(3, Some(0.6), 40, 100), Some(0.3)),
            swe(position(4, None, 0, 100), None),
            swe(position(5, Some(0.8), 200, 100), Some(0.1)),
        ]);
        for metric in ChartMetric::ALL {
            let s = Series::build(&r, &[0, 1, 2, 3, 4], metric);
            assert_eq!(s.ns_spans, vec![NsSpan { first: 4, last: 4 }], "{metric:?}");
            let paths: Vec<Vec<f64>> = s
                .paths
                .iter()
                .map(|p| p.points.iter().map(|q| q.x).collect())
                .collect();
            assert_eq!(paths, [vec![1.0], vec![3.0], vec![5.0]], "{metric:?}");
            assert_eq!(s.extent, Some((1, 5)));
            assert_eq!(
                (s.count(Style::Eligible), s.count(Style::LowSupport)),
                (2, 1)
            );
        }
        // Each metric plots its own values, and its own maximum.
        let e = Series::build(&r, &[0, 2, 4], ChartMetric::Entropy);
        let w = Series::build(&r, &[0, 2, 4], ChartMetric::SimilarityWeighted);
        assert_eq!(e.paths[0].points[0].y, 0.4);
        assert_eq!(w.paths[0].points[0].y, 0.2);
        assert_eq!((e.y_max, w.y_max), (Some(0.8), Some(0.3)));
        // Not computed: no line at all, but the band stays.
        let off = results_of(vec![
            position(1, Some(0.4), 200, 100),
            position(2, None, 0, 100),
        ]);
        let s = Series::build(&off, &[0, 1], ChartMetric::SimilarityWeighted);
        assert!(s.paths.is_empty() && s.y_max.is_none());
        assert_eq!(s.ns_spans.len(), 1);
    }

    #[test]
    fn legends_follow_the_estimator_and_the_metric() {
        use ChartMetric::{Entropy, SimilarityWeighted};
        assert_eq!(
            legend(100, Estimator::Dima, Entropy),
            [
                "support > 100: corrected where possible",
                "support \u{2264} 100: uncorrected"
            ]
        );
        assert_eq!(
            legend(100, Estimator::Raw, Entropy),
            [
                "support > 100: eligible",
                "support \u{2264} 100: low support"
            ]
        );
        for e in Estimator::ALL {
            assert_eq!(
                legend(30, e, SimilarityWeighted),
                ["support > 30", "support \u{2264} 30, not in the summary"],
                "never corrected, whatever the estimator"
            );
        }
    }

    #[test]
    fn ns_is_never_zero_and_spans_merge() {
        let r = results_of(vec![
            position(1, None, 0, 100),
            position(2, None, 0, 100),
            position(3, Some(0.0), 300, 100),
            position(4, None, 0, 100),
        ]);
        let s = Series::build(&r, &[0, 1, 2, 3], ChartMetric::Entropy);
        assert_eq!(
            s.ns_spans,
            vec![NsSpan { first: 1, last: 2 }, NsSpan { first: 4, last: 4 }]
        );
        assert!(
            s.paths.iter().flat_map(|p| &p.points).all(|p| p.x == 3.0),
            "NS positions are not points"
        );
        assert_eq!(ns_span_at(&s, 1.7), Some(NsSpan { first: 1, last: 2 }));
        assert_eq!(ns_span_at(&s, 3.0), None);
        assert_eq!(ns_span_at(&s, 4.4), Some(NsSpan { first: 4, last: 4 }));
        // NS separated by a filtered-out position are separate bands.
        let s = Series::build(&r, &[0, 3], ChartMetric::Entropy);
        assert_eq!(s.ns_spans.len(), 2);
    }

    #[test]
    fn nearest_and_view_clipping() {
        let r = results_of(
            (1..=100)
                .map(|p| position(p, Some(p as f64), 500, 100))
                .collect(),
        );
        let all: Vec<usize> = (0..100).collect();
        let s = Series::build(&r, &all, ChartMetric::Entropy);
        assert_eq!(nearest_point(&s, 41.4).unwrap().x, 41.0);
        assert_eq!(
            nearest_point(&s, 41.5).unwrap().x,
            41.0,
            "ties go to the lower position"
        );
        assert_eq!(nearest_point(&s, -5.0).unwrap().x, 1.0);
        let (window, _) = strokes(&s, 20.0, 30.0, None);
        let xs: Vec<f64> = window[0].points.iter().map(|p| p.0).collect();
        assert_eq!(
            xs.first(),
            Some(&19.0),
            "one neighbour beyond the left edge"
        );
        assert_eq!(xs.last(), Some(&31.0));
        let (reduced, _) = strokes(&s, 0.5, 100.5, Some(10));
        assert!(reduced[0].points.len() <= 40);
        assert_eq!(reduced[0].points.first(), Some(&(1.0, 1.0)));
        assert_eq!(reduced[0].points.last(), Some(&(100.0, 100.0)));
    }
}
