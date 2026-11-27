//! The deterministic provenance of a run (`run` in the results) and its diagnostics.
//!
//! Everything recorded here can change the results or is needed to interpret them: the tool
//! version, every parameter with its origin, the alphabet decision, the input's identity, what
//! validation found, and the references of the methods that ran. Nothing here depends on the
//! machine, the time or the thread count, so identical inputs give identical bytes.

use crate::alphabet::{ByteClassTable, Resolution};
use crate::citations;
use crate::config::{Alphabet, Estimator, ValidationMode};
use crate::diagnostics::{Code, Diagnostic, Tally, agree, plural};
use crate::entropy;
use crate::header::{self, HeaderSpec, HeaderStats};
use crate::model::{
    AlphabetInfo, CharacterCount, EntropyFallback, EntropyInfo, MetadataInfo, Mismatches,
    PositionResult, Rarefaction, Reference, ValidationInfo, WhitespaceCounts,
};
use crate::normalise::SequenceStats;

/// Key of the reference to this software in `run.references`.
pub const SOFTWARE_REFERENCE_KEY: &str = "dima_software";

/// The methods a run used, which decide its references.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MethodsUsed {
    /// The entropy estimator.
    pub estimator: Estimator,
    /// The alphabet.
    pub alphabet: Alphabet,
    /// The alphabet was detected from the data, not given.
    pub alphabet_detected: bool,
    /// Similarity-weighted entropy was computed.
    pub similarity_weighted_entropy: bool,
}

/// The references of a run: the DiMA paper, this software, Shannon entropy, the estimator, the
/// coverage estimate, the motif definitions and, where used, similarity-weighted entropy (with
/// the amino-acid identity measure for proteins), alphabet detection and nucleotide ambiguity
/// codes (D89).
#[must_use]
pub fn references(m: MethodsUsed) -> Vec<Reference> {
    let MethodsUsed {
        estimator,
        alphabet,
        alphabet_detected: detected,
        similarity_weighted_entropy,
    } = m;
    let mut keys: Vec<&str> = vec!["tharanga2025"];
    let software = Reference {
        key: SOFTWARE_REFERENCE_KEY.to_owned(),
        citation: citations::software_text(citations::SOFTWARE_VERSION),
        doi: None,
        url: Some(citations::SOFTWARE_REPOSITORY.to_owned()),
    };
    keys.push("shannon1948");
    keys.extend_from_slice(estimator_references(estimator));
    keys.push("chao2012");
    keys.push("hu2013");
    if similarity_weighted_entropy {
        keys.push("wu2025");
        if !alphabet.is_nucleotide() {
            keys.push("grishin1995");
        }
    }
    if detected {
        keys.push("eddy2011");
    }
    if alphabet.is_nucleotide() {
        keys.push("cornishbowden1985");
    }
    let mut out: Vec<Reference> = Vec::with_capacity(keys.len() + 1);
    out.push(Reference::of(keys[0]));
    out.push(software);
    out.extend(keys[1..].iter().map(|k| Reference::of(k)));
    out
}

/// Citation keys of an estimator.
#[must_use]
pub const fn estimator_references(estimator: Estimator) -> &'static [&'static str] {
    match estimator {
        Estimator::Dima => &["rao1945", "blackwell1947"],
        Estimator::ChaoWangJost => &["chao2013"],
        Estimator::MillerMadow => &["miller1955"],
        Estimator::Raw => &[],
    }
}

/// Entropy settings of the run.
#[must_use]
pub fn entropy_info(estimator: Estimator) -> EntropyInfo {
    EntropyInfo {
        estimator,
        log_base: 2,
        rarefaction: (estimator == Estimator::Dima).then(|| Rarefaction {
            grid: "paper_fig2".to_owned(),
            size_rounding: "ceil".to_owned(),
            start_rounding: "ceil_to_5".to_owned(),
            sampling: "expected_with_replacement".to_owned(),
            min_points: crate::entropy::RAREFACTION_MIN_POINTS as u64,
        }),
    }
}

/// Alphabet provenance.
#[must_use]
pub fn alphabet_info(
    resolution: &Resolution,
    records_examined: u64,
    t_u_folded: u64,
) -> AlphabetInfo {
    AlphabetInfo {
        resolved: resolution.alphabet,
        method: resolution.method,
        records_examined,
        residues_examined: resolution.residues_examined,
        t_u_folded,
        contradiction: resolution.contradiction.clone(),
    }
}

/// Validation provenance.
#[must_use]
pub fn validation_info(
    mode: ValidationMode,
    records: u64,
    table: &ByteClassTable,
    seq: &SequenceStats,
    headers: Option<&HeaderStats>,
) -> ValidationInfo {
    let characters = seq
        .characters
        .iter()
        .enumerate()
        .filter(|&(_, &n)| n > 0)
        .map(|(b, &count)| {
            #[allow(clippy::cast_possible_truncation)] // b < 256.
            let byte = b as u8;
            CharacterCount {
                character: byte
                    .is_ascii_graphic()
                    .then(|| char::from(byte).to_string()),
                byte,
                class: table.class(byte),
                count,
            }
        })
        .collect();
    ValidationInfo {
        mode,
        records,
        lowercase_folded: seq.lowercase_folded,
        whitespace: WhitespaceCounts {
            trailing: seq.trailing_whitespace_lines,
            interior: seq.interior_whitespace.count,
        },
        characters,
        unknown_total: seq.unknown.count,
        header_utf8_replacements: headers.map_or(0, |h| h.utf8_replacements),
        header_cleaning: headers.map(|h| header::cleaning_record(&h.cleaning)),
    }
}

/// Header-metadata provenance.
#[must_use]
pub fn metadata_info(spec: &HeaderSpec, stats: &HeaderStats, probe_records: u64) -> MetadataInfo {
    MetadataInfo {
        fields: spec.fields().to_vec(),
        stratified: spec.stratified_names(),
        probe_records,
        mismatches: Mismatches {
            count: stats.mismatch_count,
            first: stats.mismatches.clone(),
        },
        fill_uses: stats.cleaning.fill_uses,
    }
}

/// Inputs of [`diagnostics`].
#[derive(Debug)]
pub struct DiagnosticInputs<'a> {
    /// Resolved alphabet decision.
    pub resolution: &'a Resolution,
    /// Sequence statistics.
    pub sequences: &'a SequenceStats,
    /// Header statistics and format, with metadata.
    pub headers: Option<(&'a HeaderSpec, &'a HeaderStats)>,
    /// Header-value diagnostics computed from the value dictionary.
    pub value_diagnostics: Vec<Diagnostic>,
    /// Identifier-like field diagnostic from the probe.
    pub identifier: Option<Diagnostic>,
    /// Number of sequences.
    pub sequence_count: u64,
    /// Support threshold.
    pub threshold: u64,
    /// The entropy estimator.
    pub estimator: Estimator,
    /// Eligible positions.
    pub eligible_positions: u64,
    /// Every position's result, in order.
    pub positions: &'a [PositionResult],
    /// `--min-coverage` and the positions below it.
    pub below_min_coverage: Option<(f64, Tally)>,
    /// A header fill value was given without a header format.
    pub fill_without_format: bool,
}

/// The diagnostics of a run, in report order.
#[must_use]
pub fn diagnostics(d: DiagnosticInputs<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let alphabet = d.resolution.alphabet;
    let s = d.sequences;
    if s.lowercase_folded > 0 {
        let t = Tally::count_only(s.lowercase_folded);
        out.push(Diagnostic::new(
            Code::LowercaseFolded,
            format!(
                "{} {} read as upper case",
                plural(
                    s.lowercase_folded,
                    "lower-case letter",
                    "lower-case letters"
                ),
                agree(s.lowercase_folded, "was", "were")
            ),
            t,
        ));
    }
    if s.t_u_folded > 0 {
        let (from, to) = if alphabet == Alphabet::Dna {
            ("U", "T")
        } else {
            ("T", "U")
        };
        let t = Tally::count_only(s.t_u_folded);
        out.push(Diagnostic::new(
            Code::TUFolded,
            format!(
                "{} {from} {} read as {to} ({alphabet} alphabet)",
                s.t_u_folded,
                agree(s.t_u_folded, "letter was", "letters were")
            ),
            t,
        ));
    }
    if let Some(c) = &d.resolution.contradiction {
        // The message says everything; an example would repeat it.
        out.push(
            Diagnostic::new(Code::AlphabetContradiction, c.clone(), Tally::count_only(1))
                .with_hint("check --alphabet; characters outside it are excluded or rejected"),
        );
    }
    if !s.interior_whitespace.is_empty() {
        out.push(
            Diagnostic::new(
                Code::InteriorWhitespace,
                format!(
                    "{} whitespace between residues; it was ignored",
                    plural(
                        s.interior_whitespace.count,
                        "sequence line contains",
                        "sequence lines contain"
                    )
                ),
                s.interior_whitespace.clone(),
            )
            .with_hint(
                "check that the alignment file was not reformatted (e.g. by a word processor)",
            ),
        );
    }
    if !s.unknown.is_empty() {
        out.push(
            Diagnostic::new(
                Code::UnknownCharactersExcluded,
                format!(
                    "{} not valid in a {alphabet} alignment; the k-mers that contain {} were \
                     excluded",
                    plural(s.unknown.count, "character is", "characters are"),
                    agree(s.unknown.count, "it", "them")
                ),
                s.unknown.clone(),
            )
            .with_hint("run.validation.characters lists every character; check --alphabet"),
        );
    }
    if let Some((spec, h)) = d.headers {
        if h.mismatch_count > 0 {
            let mut t = Tally::default();
            for m in &h.mismatches {
                t.add(1, || {
                    format!(
                        "record {} (line {}): {}",
                        m.record,
                        m.line,
                        plural(m.found, "field", "fields")
                    )
                });
            }
            t.count = h.mismatch_count;
            out.push(
                Diagnostic::new(
                    Code::HeaderFieldCountMismatch,
                    format!(
                        "{} not have the {} of the header format; {} metadata were set to {:?}",
                        plural(h.mismatch_count, "header does", "headers do"),
                        plural(spec.fields().len() as u64, "field", "fields"),
                        agree(h.mismatch_count, "its", "their"),
                        spec.fill()
                    ),
                    t,
                )
                .with_hint(header::mismatch_diagnostic_hint(spec, h)),
            );
        }
        if h.utf8_replacements > 0 {
            let t = Tally::count_only(h.utf8_replacements);
            out.push(
                Diagnostic::new(
                    Code::HeaderUtf8Replaced,
                    format!(
                        "{} in headers {} replaced by U+FFFD",
                        plural(
                            h.utf8_replacements,
                            "invalid UTF-8 sequence",
                            "invalid UTF-8 sequences"
                        ),
                        agree(h.utf8_replacements, "was", "were")
                    ),
                    t,
                )
                .with_hint("convert the file to UTF-8, e.g. with iconv"),
            );
        }
        if let Some(c) = header::cleaning_diagnostic(&h.cleaning) {
            out.push(c);
        }
    }
    out.extend(d.value_diagnostics);
    out.extend(d.identifier);
    if d.sequence_count <= d.threshold {
        // The message says everything; an example would repeat it.
        out.push(threshold_exceeds_sequence_count(
            d.threshold,
            d.sequence_count,
            d.estimator,
        ));
    } else if d.eligible_positions == 0 {
        let t = Tally::count_only(1);
        out.push(
            Diagnostic::new(
                Code::NoEligiblePositions,
                format!("no position has support above the threshold ({})", d.threshold),
                t,
            )
            .with_hint("gappy or ambiguous columns reduce support; consider a lower --support-threshold or a shorter k"),
        );
    }
    if d.estimator == Estimator::Dima {
        out.extend(uncorrected_eligible_positions(
            d.positions,
            d.threshold,
            d.eligible_positions,
        ));
    }
    if let Some((min, t)) = d.below_min_coverage {
        if !t.is_empty() {
            out.push(
                Diagnostic::new(
                    Code::BelowMinCoverage,
                    format!(
                        "{} sample coverage below {min}",
                        plural(t.count, "position has", "positions have")
                    ),
                    t,
                )
                .with_hint("their k-mer diversity is incompletely sampled; see sample_coverage"),
            );
        }
    }
    if d.fill_without_format {
        let mut t = Tally::default();
        t.add(1, || "--header-fillna".to_owned());
        out.push(
            Diagnostic::new(
                Code::FlagHasNoEffect,
                "a header fill value was given without a header format; it has no effect",
                t,
            )
            .with_hint("name the header fields with --header-format to use metadata"),
        );
    }
    crate::diagnostics::sort(&mut out);
    out
}

/// `threshold_exceeds_sequence_count`: no position can be eligible, so the summaries are empty.
/// What the entropies are then depends on the estimator; the hint gives the largest threshold
/// that helps.
fn threshold_exceeds_sequence_count(threshold: u64, n: u64, estimator: Estimator) -> Diagnostic {
    let consequence = match estimator {
        Estimator::Dima | Estimator::Raw => {
            "the summaries are empty and every entropy is the observed value".to_owned()
        }
        Estimator::ChaoWangJost | Estimator::MillerMadow => format!(
            "the summaries are empty, but the {} estimate still applies to every position with \
             at least 2 sequences",
            estimator.cli_name()
        ),
    };
    let largest = match estimator {
        Estimator::Dima => {
            entropy::max_correcting_threshold(n).map(|t| (t, "eligible and corrected"))
        }
        _ => n
            .checked_sub(1)
            .filter(|&t| t >= 1)
            .map(|t| (t, "eligible")),
    };
    let hint = largest.map_or_else(
        || "add sequences: support above the smallest threshold, 1, needs at least 2".to_owned(),
        |(t, what)| {
            format!(
                "lower --support-threshold to {t} or less, so that positions supported by all \
                 {n} sequences are {what}; or add sequences"
            )
        },
    );
    Diagnostic::new(
        Code::ThresholdExceedsSequenceCount,
        format!(
            "the support threshold ({threshold}) is not below the number of sequences ({n}), so \
             no position can have support above it: {consequence}"
        ),
        Tally::count_only(1),
    )
    .with_hint(hint)
}

// The messages below spell the minimum out.
const _: () = assert!(entropy::RAREFACTION_MIN_POINTS == 5);

/// `uncorrected_eligible_positions` (estimator `dima` only): eligible positions with more than
/// one k-mer that kept their observed entropy because the paper's correction could not be
/// computed (`too_few_points`: support below ⌈1.25·T⌉; `degenerate_regression`: T = 2,
/// support 3). Eligible positions with one k-mer, whose entropy is 0 bits either way, are named
/// in the message but not counted. `None` when no position is counted.
fn uncorrected_eligible_positions(
    positions: &[PositionResult],
    threshold: u64,
    eligible: u64,
) -> Option<Diagnostic> {
    let mut tally = Tally::default();
    let (mut single, mut few, mut degenerate) = (0u64, false, false);
    let mut lowest_support = u64::MAX;
    for p in positions.iter().filter(|p| p.low_support.is_none()) {
        match p.entropy_fallback {
            Some(EntropyFallback::TooFewPoints) => few = true,
            Some(EntropyFallback::DegenerateRegression) => degenerate = true,
            _ => continue,
        }
        let kmers = p.diversity_motifs.len() as u64;
        if kmers < 2 {
            single += 1;
            continue;
        }
        lowest_support = lowest_support.min(p.support);
        tally.add(1, || {
            format!(
                "position {} (support {}, {kmers} distinct k-mers)",
                p.position, p.support
            )
        });
    }
    if tally.is_empty() {
        return None;
    }
    let mut reasons = Vec::new();
    if few {
        reasons.push(format!(
            "the paper's correction needs at least five regression points (a minimum taken from \
             DiMA v5.0.9), which support below {} (1.25 × T) does not give",
            entropy::min_corrected_support(threshold)
        ));
    }
    if degenerate {
        reasons.push(
            "all subsamples of the paper's correction have the same size there, so no \
             regression line exists"
                .to_owned(),
        );
    }
    let reason = reasons.join("; ");
    let n = tally.count;
    let mut message = if n + single == eligible {
        if eligible == 1 {
            format!(
                "the only eligible position was not corrected: {reason}; it kept its observed \
                 entropy and is included in the summaries"
            )
        } else {
            format!(
                "none of the {eligible} eligible positions was corrected: {reason}; they kept \
                 their observed entropy and are included in the summaries"
            )
        }
    } else {
        format!(
            "{n} of the {eligible} eligible positions kept {} observed entropy: {reason}; {} \
             included in the summaries",
            agree(n, "its", "their"),
            agree(n, "it is", "they are")
        )
    };
    if single > 0 {
        message.push_str(&format!(
            "; {single} {} {} one k-mer, where the correction would also give 0 bits",
            if n + single == eligible {
                "of them"
            } else {
                "more"
            },
            agree(single, "has", "have")
        ));
    }
    let lowest = entropy::max_correcting_threshold(lowest_support).unwrap_or(1);
    Some(
        Diagnostic::new(Code::UncorrectedEligiblePositions, message, tally).with_hint(format!(
            "to have them corrected, raise their support to at least {} (more sequences with a \
             valid k-mer there) or lower --support-threshold to at most {lowest}, and report \
             the threshold you used; entropy_fallback marks them",
            entropy::min_corrected_support(threshold)
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_follow_the_methods() {
        let keys = |estimator, alphabet, alphabet_detected| {
            references(MethodsUsed {
                estimator,
                alphabet,
                alphabet_detected,
                similarity_weighted_entropy: false,
            })
            .into_iter()
            .map(|r| r.key)
            .collect::<Vec<_>>()
        };
        assert_eq!(
            keys(Estimator::Dima, Alphabet::Protein, true),
            [
                "tharanga2025",
                "dima_software",
                "shannon1948",
                "rao1945",
                "blackwell1947",
                "chao2012",
                "hu2013",
                "eddy2011"
            ]
        );
        assert_eq!(
            keys(Estimator::ChaoWangJost, Alphabet::Dna, false),
            [
                "tharanga2025",
                "dima_software",
                "shannon1948",
                "chao2013",
                "chao2012",
                "hu2013",
                "cornishbowden1985"
            ]
        );
        assert!(
            keys(Estimator::MillerMadow, Alphabet::Rna, false).contains(&"miller1955".to_owned())
        );
        let r = references(MethodsUsed {
            estimator: Estimator::Raw,
            alphabet: Alphabet::Protein,
            alphabet_detected: false,
            similarity_weighted_entropy: false,
        });
        assert_eq!(r[0].doi.as_deref(), Some("10.1093/bib/bbae607"));
        assert!(r[1].citation.contains("dima"));
        // Similarity-weighted entropy adds Wu 2025, and for proteins the identity measure.
        let with = |alphabet| {
            references(MethodsUsed {
                estimator: Estimator::Raw,
                alphabet,
                alphabet_detected: false,
                similarity_weighted_entropy: true,
            })
            .into_iter()
            .map(|r| r.key)
            .collect::<Vec<_>>()
        };
        assert_eq!(
            with(Alphabet::Protein)[4..],
            ["hu2013", "wu2025", "grishin1995"]
        );
        assert_eq!(
            with(Alphabet::Dna)[4..],
            ["hu2013", "wu2025", "cornishbowden1985"]
        );
    }

    #[test]
    fn entropy_settings() {
        assert!(entropy_info(Estimator::Dima).rarefaction.is_some());
        assert!(entropy_info(Estimator::Raw).rarefaction.is_none());
        assert_eq!(
            entropy_info(Estimator::Dima)
                .rarefaction
                .unwrap()
                .min_points,
            5
        );
    }

    use crate::model::{EntropyMethod, Motif, SupportLabel, Variant};

    /// A position with `kmers` distinct k-mers (the first carries the rest of the support) and
    /// the given label and fallback.
    fn position(
        position: u64,
        support: u64,
        kmers: u64,
        low_support: Option<SupportLabel>,
        fallback: Option<EntropyFallback>,
    ) -> PositionResult {
        let variant = |i: u64, count: u64| Variant {
            sequence: format!("K{i}"),
            count,
            incidence: 100.0 * count as f64 / support as f64,
            motif: if i == 0 { Motif::Index } else { Motif::Unique },
            metadata: None,
        };
        PositionResult {
            position,
            low_support,
            support,
            distinct_variants_count: kmers.saturating_sub(1),
            entropy: Some(0.5),
            observed_entropy: Some(0.5),
            entropy_method: if fallback.is_some() {
                EntropyMethod::PlugIn
            } else {
                EntropyMethod::Dima
            },
            entropy_fallback: fallback,
            distinct_variants_incidence: None,
            total_variants_incidence: None,
            sample_coverage: None,
            below_min_coverage: None,
            similarity_weighted_entropy: None,
            normalised_observed_entropy: None,
            normalised_similarity_weighted_entropy: None,
            diversity_motifs: (0..kmers)
                .map(|i| variant(i, if i == 0 { support - kmers + 1 } else { 1 }))
                .collect(),
        }
    }

    const FEW: Option<EntropyFallback> = Some(EntropyFallback::TooFewPoints);

    #[test]
    fn uncorrected_eligible_positions_are_counted_with_their_reason() {
        // T = 5: support 6 is below ⌈1.25·5⌉ = 7.
        let ps = [
            position(1, 7, 2, None, None),
            position(2, 6, 2, None, FEW),
            position(3, 6, 1, None, FEW),
            position(4, 7, 1, None, None),
            position(
                5,
                3,
                2,
                Some(SupportLabel::Low),
                Some(EntropyFallback::SupportAtOrBelowThreshold),
            ),
        ];
        let d = uncorrected_eligible_positions(&ps, 5, 4).unwrap();
        assert_eq!(d.code, "uncorrected_eligible_positions");
        assert_eq!(d.severity, crate::diagnostics::Severity::Warning);
        assert_eq!(
            d.message,
            "1 of the 4 eligible positions kept its observed entropy: the paper's correction \
             needs at least five regression points (a minimum taken from DiMA v5.0.9), which \
             support below 7 (1.25 × T) does not give; it is included in the summaries; 1 more \
             has one k-mer, where the correction would also give 0 bits"
        );
        assert_eq!(d.count, 1, "single-k-mer positions are not counted");
        assert_eq!(d.examples, ["position 2 (support 6, 2 distinct k-mers)"]);
        assert_eq!(
            d.hint.as_deref(),
            Some(
                "to have them corrected, raise their support to at least 7 (more sequences with \
                 a valid k-mer there) or lower --support-threshold to at most 4, and report the \
                 threshold you used; entropy_fallback marks them"
            )
        );
        // Every eligible position uncorrected.
        let all = [position(1, 6, 3, None, FEW), position(2, 6, 1, None, FEW)];
        let d = uncorrected_eligible_positions(&all, 5, 2).unwrap();
        assert!(
            d.message
                .starts_with("none of the 2 eligible positions was corrected: "),
            "{}",
            d.message
        );
        assert!(
            d.message.ends_with(
                "; they kept their observed entropy and are included in the summaries; 1 of \
                 them has one k-mer, where the correction would also give 0 bits"
            ),
            "{}",
            d.message
        );
        let one = [position(1, 6, 3, None, FEW)];
        assert!(
            uncorrected_eligible_positions(&one, 5, 1)
                .unwrap()
                .message
                .starts_with("the only eligible position was not corrected: ")
        );
        // Only single-k-mer positions (0 bits either way), or none: no warning.
        assert_eq!(
            uncorrected_eligible_positions(&[position(1, 6, 1, None, FEW)], 5, 1),
            None
        );
        assert_eq!(uncorrected_eligible_positions(&ps[..1], 5, 1), None);
        // T = 2, support 3: equal subsample sizes, corrected from support 4, or at T = 1.
        let degenerate = [position(
            1,
            3,
            2,
            None,
            Some(EntropyFallback::DegenerateRegression),
        )];
        let d = uncorrected_eligible_positions(&degenerate, 2, 1).unwrap();
        assert!(
            d.message.contains(
                ": all subsamples of the paper's correction have the same size there, so no \
                 regression line exists; "
            ),
            "{}",
            d.message
        );
        assert!(d.hint.as_deref().unwrap().contains(
            "raise their support to at least 4 (more sequences with a valid k-mer there) or \
                 lower --support-threshold to at most 1,"
        ));
    }

    #[test]
    fn the_new_warning_is_raised_with_the_papers_estimator_only() {
        let resolution = crate::alphabet::resolve(
            Some(Alphabet::Dna),
            &crate::alphabet::LetterCounts::default(),
            0,
        )
        .unwrap();
        let stats = SequenceStats::default();
        let ps = [position(1, 6, 2, None, FEW)];
        let codes = |estimator| {
            diagnostics(DiagnosticInputs {
                resolution: &resolution,
                sequences: &stats,
                headers: None,
                value_diagnostics: Vec::new(),
                identifier: None,
                sequence_count: 6,
                threshold: 5,
                estimator,
                eligible_positions: 1,
                positions: &ps,
                below_min_coverage: None,
                fill_without_format: false,
            })
            .into_iter()
            .map(|d| d.code)
            .collect::<Vec<_>>()
        };
        assert_eq!(codes(Estimator::Dima), ["uncorrected_eligible_positions"]);
        for other in [
            Estimator::Raw,
            Estimator::ChaoWangJost,
            Estimator::MillerMadow,
        ] {
            assert!(codes(other).is_empty(), "{other:?}");
        }
    }

    #[test]
    fn threshold_advice_depends_on_the_estimator() {
        let d = threshold_exceeds_sequence_count(100, 26, Estimator::Dima);
        assert_eq!(
            d.message,
            "the support threshold (100) is not below the number of sequences (26), so no \
             position can have support above it: the summaries are empty and every entropy is \
             the observed value"
        );
        assert_eq!(
            d.hint.as_deref(),
            Some(
                "lower --support-threshold to 20 or less, so that positions supported by all 26 \
                 sequences are eligible and corrected; or add sequences"
            )
        );
        assert!(d.examples.is_empty(), "the message says it all");
        assert_eq!(d.count, 1);
        let d = threshold_exceeds_sequence_count(100, 26, Estimator::ChaoWangJost);
        assert!(
            d.message.ends_with(
                "so no position can have support above it: the summaries are empty, but the \
                 chao-wang-jost estimate still applies to every position with at least 2 \
                 sequences"
            ),
            "{}",
            d.message
        );
        assert_eq!(
            d.hint.as_deref(),
            Some(
                "lower --support-threshold to 25 or less, so that positions supported by all 26 \
                 sequences are eligible; or add sequences"
            )
        );
        assert!(
            threshold_exceeds_sequence_count(5, 3, Estimator::Dima)
                .hint
                .unwrap()
                .starts_with("lower --support-threshold to 1 or less,")
        );
        for estimator in Estimator::ALL {
            assert_eq!(
                threshold_exceeds_sequence_count(1, 1, estimator)
                    .hint
                    .as_deref(),
                Some("add sequences: support above the smallest threshold, 1, needs at least 2")
            );
        }
    }
}
