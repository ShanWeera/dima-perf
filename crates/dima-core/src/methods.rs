//! A methods paragraph and the reference list of a run, for `dima cite --from` and the GUI's
//! "Export methods & references" (D89): built from the run's recorded parameters and its
//! `run.references` — and, when given, from the conserved sequences computed from it and their
//! `references` — so it states exactly what was done and cites exactly what was used.

use std::fmt::Write as _;

use crate::citations::{self, Citation, Cite};
use crate::config::{Alphabet, Estimator};
use crate::diagnostics::plural;
use crate::model::{HcsEligibility, HcsResults, Reference, Results};
use crate::provenance::SOFTWARE_REFERENCE_KEY;

/// Output formats of a reference list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CitationFormat {
    /// Numbered plain-text references.
    Text,
    /// BibTeX entries.
    Bibtex,
    /// RIS records.
    Ris,
    /// A complete `CITATION.cff` with the references.
    Cff,
}

/// What a methods text and reference list describe: an analysis, the conserved sequences
/// computed from one, both, or neither (DiMA itself).
#[derive(Clone, Copy, Debug, Default)]
pub struct Cited<'a> {
    /// The results of an analysis.
    pub results: Option<&'a Results>,
    /// Conserved sequences.
    pub hcs: Option<&'a HcsResults>,
}

impl<'a> Cited<'a> {
    /// An analysis.
    #[must_use]
    pub const fn results(results: &'a Results) -> Self {
        Self {
            results: Some(results),
            hcs: None,
        }
    }

    /// The DiMA version that wrote what is cited, or this one.
    fn version(&self) -> &'a str {
        self.results
            .map(|r| r.run.tool.version.as_str())
            .or_else(|| self.hcs.map(|h| h.tool.version.as_str()))
            .unwrap_or(citations::SOFTWARE_VERSION)
    }
}

/// A parenthetical citation of several works: `(Rao 1945; Blackwell 1947)`.
fn cited(keys: &[&'static str]) -> String {
    let labels: Vec<String> = keys.iter().map(|&k| Cite::new(k).short()).collect();
    format!("({})", labels.join("; "))
}

fn estimator_text(e: Estimator) -> String {
    match e {
        Estimator::Dima => format!(
            "the rarefaction-regression correction of {}, computed as its exact expectation {}",
            Cite::new(citations::DIMA_PAPER).at("Fig. 2").narrative(),
            cited(&["rao1945", "blackwell1947"])
        ),
        Estimator::ChaoWangJost => {
            format!("the estimator of {}", Cite::new("chao2013").narrative())
        }
        Estimator::MillerMadow => {
            format!("the Miller–Madow correction {}", cited(&["miller1955"]))
        }
        Estimator::Raw => "the plug-in (observed) estimator".to_owned(),
    }
}

fn alphabet_text(a: Alphabet) -> &'static str {
    match a {
        Alphabet::Protein => "protein",
        Alphabet::Dna => "DNA",
        Alphabet::Rna => "RNA",
    }
}

/// What eligibility (support above T) meant for a run: only the paper's estimator corrects
/// eligible positions alone; the others estimate every position alike.
fn eligible_for(e: Estimator) -> &'static str {
    match e {
        Estimator::Dima => "the corrected entropy and the summary statistics",
        Estimator::ChaoWangJost | Estimator::MillerMadow | Estimator::Raw => {
            "the summary statistics"
        }
    }
}

/// A methods paragraph describing the run.
#[must_use]
pub fn sentence(r: &Results) -> String {
    let p = &r.run.parameters;
    let mut s = format!(
        "Sequence diversity was analysed with DiMA {} using dima {}. In the {} \
         alignment of {} ({} columns), the overlapping {}-mers at each of the {} were counted; \
         positions supported by more than {} were eligible for {}. Shannon entropy {} was \
         estimated with {}, and sample coverage after {}. Variants were classified as Index, \
         Major, Minor and Unique motifs {}.",
        cited(&[citations::DIMA_PAPER]),
        r.run.tool.version,
        alphabet_text(p.alphabet.value),
        plural(r.sequence_count, "sequence", "sequences"),
        r.run.alignment_length,
        r.kmer_length,
        plural(r.position_count, "position", "positions"),
        plural(r.support_threshold, "sequence", "sequences"),
        eligible_for(p.estimator.value),
        cited(&["shannon1948"]),
        estimator_text(p.estimator.value),
        Cite::new("chao2012").narrative(),
        cited(&["hu2013"]),
    );
    let unknown = r.run.validation.unknown_total;
    if unknown > 0 {
        s.push_str(&format!(
            " K-mers containing characters that are not symbols of the {} alphabet ({}) \
             were excluded.",
            alphabet_text(p.alphabet.value),
            plural(unknown, "occurrence", "occurrences"),
        ));
    }
    if let Some(fields) = &p.metadata_fields.value {
        s.push_str(&format!(
            " Variant counts were stratified by the header fields {}.",
            fields.join(", ")
        ));
    }
    s
}

/// One entry of a reference list.
enum Entry<'a> {
    /// This software, at the given version.
    Software(&'a str),
    /// A reference this version knows.
    Known(&'static Citation),
    /// A reference recorded by a run that this version does not know (for example from a newer
    /// dima): rendered from the text stored in the results.
    Recorded(&'a Reference),
}

impl Entry<'_> {
    fn text(&self) -> String {
        match self {
            Self::Software(v) => citations::software_text(v),
            Self::Known(c) => c.text(),
            Self::Recorded(r) => r.citation.clone(),
        }
    }

    fn bibtex(&self) -> String {
        match self {
            Self::Software(v) => citations::software_bibtex(v),
            Self::Known(c) => c.bibtex(),
            Self::Recorded(r) => {
                let mut s = format!(
                    "@misc{{{},\n  note      = {{{}}},\n",
                    bibtex_key(&r.key),
                    citations::bibtex_escape(&r.citation)
                );
                if let Some(d) = &r.doi {
                    let _ = writeln!(s, "  doi       = {{{d}}},");
                }
                if let Some(u) = &r.url {
                    let _ = writeln!(s, "  url       = {{{u}}},");
                }
                s.push('}');
                s
            }
        }
    }

    fn ris(&self) -> String {
        match self {
            Self::Software(v) => citations::software_ris(v),
            Self::Known(c) => c.ris(),
            Self::Recorded(r) => {
                let mut s = String::from("TY  - GEN\n");
                let _ = writeln!(s, "N1  - {}", one_line(&r.citation));
                if let Some(d) = &r.doi {
                    let _ = writeln!(s, "DO  - {d}");
                }
                if let Some(u) = &r.url {
                    let _ = writeln!(s, "UR  - {u}");
                }
                s.push_str("ER  - \n");
                s
            }
        }
    }
}

/// A BibTeX key made of the characters BibTeX accepts in keys.
fn bibtex_key(key: &str) -> String {
    let k: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'))
        .collect();
    if k.is_empty() {
        "reference".to_owned()
    } else {
        k
    }
}

/// Line breaks replaced by spaces (RIS and YAML comments are line-based).
fn one_line(s: &str) -> String {
    s.split(['\r', '\n'])
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The references of a run, in the order recorded. The software is cited at the version that
/// produced the results.
/// The reference list, in order and without repeats: the analysis's references (or the paper
/// and this software), then those of the conserved sequences not already listed.
fn entries<'a>(c: Cited<'a>) -> Vec<Entry<'a>> {
    let version = c.version();
    let mut out: Vec<Entry<'a>> = Vec::new();
    let mut seen: Vec<&'a str> = Vec::new();
    let mut add = |key: &'a str, recorded: Option<&'a Reference>| {
        if seen.contains(&key) {
            return;
        }
        seen.push(key);
        let entry = if key == SOFTWARE_REFERENCE_KEY {
            Some(Entry::Software(version))
        } else {
            citations::by_key(key)
                .map(Entry::Known)
                .or(recorded.map(Entry::Recorded))
        };
        out.extend(entry);
    };
    match c.results {
        Some(r) => r.run.references.iter().for_each(|x| add(&x.key, Some(x))),
        None => {
            add(citations::DIMA_PAPER, None);
            add(SOFTWARE_REFERENCE_KEY, None);
        }
    }
    if let Some(h) = c.hcs {
        h.references.iter().for_each(|x| add(&x.key, Some(x)));
    }
    out
}

/// A methods sentence for conserved sequences.
#[must_use]
pub fn hcs_sentence(h: &HcsResults) -> String {
    let p = &h.parameters;
    let t = plural(h.source.support_threshold, "sequence", "sequences");
    let mut s = format!(
        "Historically conserved sequences were identified as runs of {}-mer positions whose \
         Index k-mer had an incidence of at least {}% {}, joining k-mers that overlap or are \
         adjacent and agree on their shared columns {}; each was given the exact 95% \
         Clopper–Pearson lower bound of its conservation {}.",
        h.source.kmer_length,
        p.threshold_percent,
        cited(&[citations::DIMA_PAPER]),
        cited(&["raman2020"]),
        cited(&["clopper1934"]),
    );
    match p.eligibility {
        HcsEligibility::AllPositions => {
            let _ = write!(
                s,
                " Regions that include positions supported by at most {t} were flagged."
            );
        }
        HcsEligibility::SupportGtThreshold => {
            let _ = write!(
                s,
                " Only positions supported by more than {t} were used; positions with less \
                 support ended a region {}.",
                cited(&["castresana2000"])
            );
        }
    }
    s
}

/// The methods text and its numbered references, as plain text: what `dima cite --from`
/// prints and the desktop app's "Methods & references" export writes. `None` when nothing is
/// cited (no results and no conserved sequences).
#[must_use]
pub fn methods_text(c: Cited<'_>) -> Option<String> {
    let paragraph = match (c.results, c.hcs) {
        (Some(r), Some(h)) => format!("{} {}", sentence(r), hcs_sentence(h)),
        (Some(r), None) => sentence(r),
        (None, Some(h)) => hcs_sentence(h),
        (None, None) => return None,
    };
    Some(format!(
        "Methods:\n\n{paragraph}\n\nReferences:\n\n{}\n",
        references(c, CitationFormat::Text)
    ))
}

/// The reference list of what is cited (DiMA itself when nothing is).
#[must_use]
pub fn references(c: Cited<'_>, format: CitationFormat) -> String {
    let list = entries(c);
    match format {
        CitationFormat::Text => list
            .iter()
            .enumerate()
            .map(|(i, e)| format!("{}. {}", i + 1, e.text()))
            .collect::<Vec<_>>()
            .join("\n"),
        CitationFormat::Bibtex => list
            .iter()
            .map(Entry::bibtex)
            .collect::<Vec<_>>()
            .join("\n\n"),
        CitationFormat::Ris => list.iter().map(Entry::ris).collect::<Vec<_>>().join("\n"),
        CitationFormat::Cff => {
            let version = c.version();
            let extra: Vec<&Citation> = list
                .iter()
                .filter_map(|e| match e {
                    Entry::Known(c) if c.key != citations::DIMA_PAPER => Some(*c),
                    _ => None,
                })
                .collect();
            let mut doc = citations::cff_document(version, &extra);
            // CFF references need structured authors and titles, which a recorded reference
            // does not have; they are kept as comments so nothing the run cited is lost.
            let recorded: Vec<&Reference> = list
                .iter()
                .filter_map(|e| match e {
                    Entry::Recorded(r) => Some(*r),
                    _ => None,
                })
                .collect();
            if !recorded.is_empty() {
                doc.push_str("# Also cited by this run (unknown to this dima version):\n");
                for r in recorded {
                    let _ = writeln!(doc, "# - {}", one_line(&r.citation));
                }
            }
            doc
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dima_only() {
        let t = references(Cited::default(), CitationFormat::Text);
        assert!(t.starts_with("1. Tharanga S, Ünlü ES"));
        assert!(t.contains("\n2. Tharanga S. dima: sequence diversity"));
        assert!(
            references(Cited::default(), CitationFormat::Bibtex).contains("@software{dima_rs_")
        );
        assert!(references(Cited::default(), CitationFormat::Ris).contains("TY  - COMP"));
        // A user's CFF does not carry the repository file's "do not edit" comment.
        assert!(
            references(Cited::default(), CitationFormat::Cff).starts_with("cff-version: 1.2.0")
        );
    }

    #[test]
    fn a_run_is_cited_as_recorded() {
        let mut r: Results = serde_json::from_slice(
            &std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/tiny/results.json"
            ))
            .unwrap(),
        )
        .unwrap();
        // Results written by another version cite that version.
        r.run.tool.version = "0.9.1".to_owned();
        r.run.references.push(Reference {
            key: "future2030".to_owned(),
            citation: "Doe J. A method from the future. J Future. 2030;1:1. doi:10.0/x".to_owned(),
            doi: Some("10.0/x".to_owned()),
            url: None,
        });
        let n = r.run.references.len();
        let t = references(Cited::results(&r), CitationFormat::Text);
        assert_eq!(t.lines().count(), n);
        assert_eq!(t.matches("[software]").count(), 1);
        assert!(t.contains(", version 0.9.1 [software]. "));
        assert!(t.ends_with(&format!(
            "{n}. Doe J. A method from the future. J Future. 2030;1:1. doi:10.0/x"
        )));
        let b = references(Cited::results(&r), CitationFormat::Bibtex);
        assert_eq!(b.matches("\n@").count() + 1, n);
        assert!(b.contains("@software{dima_rs_0_9_1,"));
        assert!(b.ends_with(
            "@misc{future2030,\n  note      = {Doe J. A method from the future. J Future. \
             2030;1:1. doi:10.0/x},\n  doi       = {10.0/x},\n}"
        ));
        let ris = references(Cited::results(&r), CitationFormat::Ris);
        assert_eq!(ris.matches("TY  - ").count(), n);
        assert!(ris.contains("ET  - 0.9.1\n"));
        assert!(ris.ends_with("TY  - GEN\nN1  - Doe J. A method from the future. J Future. 2030;1:1. doi:10.0/x\nDO  - 10.0/x\nER  - \n"));
        let cff = references(Cited::results(&r), CitationFormat::Cff);
        assert!(cff.contains("version: \"0.9.1\"\n"));
        assert!(cff.ends_with(
            "# Also cited by this run (unknown to this dima version):\n# - Doe J. A method from \
             the future. J Future. 2030;1:1. doi:10.0/x\n"
        ));
    }

    #[test]
    fn the_paragraph_cites_with_the_registry_labels() {
        let r: Results = serde_json::from_slice(
            &std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/tiny/results.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let s = sentence(&r);
        for cite in [
            "DiMA (Tharanga et al. 2025)",
            "Shannon entropy (Shannon 1948)",
            "sample coverage after Chao & Jost (2012)",
            "motifs (Hu et al. 2013).",
        ] {
            assert!(s.contains(cite), "{cite:?} not in {s:?}");
        }
        assert_eq!(
            estimator_text(Estimator::Dima),
            "the rarefaction-regression correction of Tharanga et al. (2025, Fig. 2), computed \
             as its exact expectation (Rao 1945; Blackwell 1947)"
        );
        assert_eq!(
            estimator_text(Estimator::ChaoWangJost),
            "the estimator of Chao et al. (2013)"
        );
        assert_eq!(
            estimator_text(Estimator::MillerMadow),
            "the Miller–Madow correction (Miller 1955)"
        );
        assert!(s.contains("were eligible for the corrected entropy and the summary statistics"));
        // Other estimators estimate every position alike: eligibility is only for summaries.
        let mut raw = r.clone();
        raw.run.parameters.estimator.value = Estimator::Raw;
        raw.run.validation.unknown_total = 3;
        let s = sentence(&raw);
        assert!(
            s.contains("were eligible for the summary statistics. "),
            "{s}"
        );
        assert!(s.contains("the plug-in (observed) estimator"), "{s}");
        assert!(
            s.contains("not symbols of the protein alphabet (3 occurrences) were excluded."),
            "{s}"
        );
    }

    #[test]
    fn conserved_sequences_add_their_sentence_and_references() {
        let r: Results = serde_json::from_slice(
            &std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/tiny/results.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let params = crate::hcs::HcsParams {
            threshold: "75".parse().unwrap(),
            eligible_only: true,
        };
        let h = crate::hcs::compute(&r, &params).unwrap();
        let both = Cited {
            results: Some(&r),
            hcs: Some(&h),
        };
        let text = methods_text(both).unwrap();
        assert!(
            text.contains("an incidence of at least 75% (Tharanga et al. 2025)"),
            "{text}"
        );
        assert!(text.contains("positions with less support ended a region (Castresana 2000)."));
        // Every reference once, the run's first, then the conserved sequences' new ones.
        let list = references(both, CitationFormat::Text);
        let run_only = references(Cited::results(&r), CitationFormat::Text);
        assert!(list.starts_with(&run_only), "{list}");
        for key in ["raman2020", "clopper1934", "castresana2000"] {
            let c = citations::by_key(key).unwrap();
            assert_eq!(list.matches(&c.text()).count(), 1, "{key}");
        }
        assert_eq!(list.matches("Tharanga S, Ünlü ES").count(), 1);
        // Conserved sequences alone: the paper and the software head the list.
        let alone = Cited {
            results: None,
            hcs: Some(&h),
        };
        let list = references(alone, CitationFormat::Text);
        assert!(list.starts_with("1. Tharanga S, Ünlü ES"), "{list}");
        assert!(
            list.contains("\n2. Tharanga S. dima: sequence diversity"),
            "{list}"
        );
        assert!(
            methods_text(alone)
                .unwrap()
                .starts_with("Methods:\n\nHistorically conserved")
        );
        assert_eq!(methods_text(Cited::default()), None);
    }
}
