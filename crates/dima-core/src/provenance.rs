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
use crate::header::{self, HeaderSpec, HeaderStats};
use crate::model::{
    AlphabetInfo, CharacterCount, EntropyInfo, MetadataInfo, Mismatches, Rarefaction, Reference,
    ValidationInfo, WhitespaceCounts,
};
use crate::normalise::SequenceStats;

/// Key of the reference to this software in `run.references`.
pub const SOFTWARE_REFERENCE_KEY: &str = "dima_software";

/// The references of a run: the DiMA paper, this software, Shannon entropy, the estimator, the
/// coverage estimate, the motif definitions and, where used, alphabet detection and nucleotide
/// ambiguity codes (D89).
#[must_use]
pub fn references(estimator: Estimator, alphabet: Alphabet, detected: bool) -> Vec<Reference> {
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
    /// Eligible positions.
    pub eligible_positions: u64,
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
        let mut t = Tally::default();
        t.add(1, || c.clone());
        out.push(
            Diagnostic::new(Code::AlphabetContradiction, c.clone(), t)
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
                .with_hint("run.metadata.mismatches lists the first headers in full"),
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
        let mut t = Tally::default();
        t.add(1, || {
            format!(
                "{}, threshold {}",
                plural(d.sequence_count, "sequence", "sequences"),
                d.threshold
            )
        });
        out.push(
            Diagnostic::new(
                Code::ThresholdExceedsSequenceCount,
                format!(
                    "the support threshold ({}) is not below the number of sequences ({}), so no \
                     position can have support above it; every entropy is the observed value",
                    d.threshold, d.sequence_count
                ),
                t,
            )
            .with_hint("lower --support-threshold, or add sequences"),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_follow_the_methods() {
        let keys = |e, a, d| {
            references(e, a, d)
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
        let r = references(Estimator::Raw, Alphabet::Protein, false);
        assert_eq!(r[0].doi.as_deref(), Some("10.1093/bib/bbae607"));
        assert!(r[1].citation.contains("dima"));
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
}
