//! Typed analysis results and their JSON representation (`schemas/results.schema.json`).
//!
//! Key names follow DiMA (Tharanga et al. 2025) so existing knowledge of the tool carries over;
//! enumerations serialise to DiMA's strings. Quantities that are undefined, or not computed
//! because their option is off, are `null`, never a sentinel: entropy at positions without
//! support, ratios with a zero denominator, extrema of an empty set, similarity-weighted entropy
//! of a run without `--similarity-weighted-entropy`. The output is deterministic: no timestamps,
//! no absolute paths, a fixed key order.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::config::{Alphabet, Estimator, Origin, Param, ValidationMode};
use crate::diagnostics::Diagnostic;

/// Version of the results JSON schema (SemVer: a reader accepts the same major). 1.1.0 added
/// similarity-weighted entropy (Wu 2025): three per-position values, a summary and a parameter.
pub const SCHEMA_VERSION: &str = "1.1.0";

/// Version of the conserved-sequences (HCS) JSON schema; it changes only when that schema does.
pub const HCS_SCHEMA_VERSION: &str = "1.0.0";

// ---------------------------------------------------------------------------------------------
// Per-position vocabulary
// ---------------------------------------------------------------------------------------------

/// Low-support class of a position with support *N* and threshold *T* (Tharanga et al. 2025):
/// no support (N = 0), low support (0 < N < T), equal to the threshold (N = T). Positions with
/// N > T are *eligible* and carry no label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportLabel {
    /// No support: no sequence has a valid k-mer here.
    #[serde(rename = "NS")]
    NoSupport,
    /// Low support: fewer sequences than the threshold.
    #[serde(rename = "LS")]
    Low,
    /// Equal to the threshold.
    #[serde(rename = "ELS")]
    EqualToThreshold,
}

impl SupportLabel {
    /// The paper's abbreviation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoSupport => "NS",
            Self::Low => "LS",
            Self::EqualToThreshold => "ELS",
        }
    }
}

/// The method that produced a position's `entropy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntropyMethod {
    /// The paper's rarefaction-regression correction (exact expectation).
    Dima,
    /// Chao et al. (2013).
    ChaoWangJost,
    /// Miller–Madow.
    MillerMadow,
    /// Plug-in (observed) entropy.
    PlugIn,
    /// No support: entropy is undefined.
    NotEstimable,
}

impl EntropyMethod {
    /// Serialised name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dima => "dima",
            Self::ChaoWangJost => "chao_wang_jost",
            Self::MillerMadow => "miller_madow",
            Self::PlugIn => "plug_in",
            Self::NotEstimable => "not_estimable",
        }
    }
}

/// Why the requested estimator fell back to the plug-in entropy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntropyFallback {
    /// `dima`: support N ≤ T, so the paper reports the observed entropy.
    SupportAtOrBelowThreshold,
    /// `dima`: fewer than five rarefaction points (DiMA v5.0.9's minimum).
    TooFewPoints,
    /// `dima`: all subsample sizes are equal, so no regression line exists.
    DegenerateRegression,
    /// Miller–Madow / Chao–Wang–Jost: a single observation (N = 1), entropy 0.
    SingleObservation,
}

impl EntropyFallback {
    /// Serialised name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SupportAtOrBelowThreshold => "support_at_or_below_threshold",
            Self::TooFewPoints => "too_few_points",
            Self::DegenerateRegression => "degenerate_regression",
            Self::SingleObservation => "single_observation",
        }
    }
}

/// Diversity motif of a k-mer variant (Hu et al. 2013; Tharanga et al. 2025).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Motif {
    /// The most common variant (count > 1; ties are all Index).
    Index,
    /// The most common of the remaining non-unique variants (ties are all Major).
    Major,
    /// Any other variant seen more than once.
    Minor,
    /// A variant seen exactly once.
    Unique,
}

impl Motif {
    /// Short form used by DiMA: `I`, `Ma`, `Mi`, `U`.
    #[must_use]
    pub const fn short(self) -> &'static str {
        match self {
            Self::Index => "I",
            Self::Major => "Ma",
            Self::Minor => "Mi",
            Self::Unique => "U",
        }
    }

    /// Long form: `Index`, `Major`, `Minor`, `Unique`.
    #[must_use]
    pub const fn long(self) -> &'static str {
        match self {
            Self::Index => "Index",
            Self::Major => "Major",
            Self::Minor => "Minor",
            Self::Unique => "Unique",
        }
    }
}

/// Counts of each value of each stratified header field among the sequences carrying a
/// variant, in `--header-format` order with values sorted by their UTF-8 bytes.
///
/// Serialised as an object of objects: `{"Host": {"Camel": 3, "Human": 2}}`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata(pub Vec<FieldCounts>);

/// Value counts of one header field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldCounts {
    /// Field name.
    pub field: String,
    /// `(value, count)` pairs.
    pub values: Vec<(String, u64)>,
}

struct Pairs<'a>(&'a [(String, u64)]);

impl Serialize for Pairs<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

impl Serialize for Metadata {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for f in &self.0 {
            m.serialize_entry(&f.field, &Pairs(&f.values))?;
        }
        m.end()
    }
}

/// Deserialises a JSON object into ordered pairs, preserving document order.
struct OrderedMap<V>(Vec<(String, V)>);

impl<'de, V: Deserialize<'de>> Deserialize<'de> for OrderedMap<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V2<V>(std::marker::PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for V2<V> {
            type Value = OrderedMap<V>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut v = Vec::with_capacity(a.size_hint().unwrap_or(0));
                while let Some(entry) = a.next_entry::<String, V>()? {
                    v.push(entry);
                }
                Ok(OrderedMap(v))
            }
        }
        d.deserialize_map(V2(std::marker::PhantomData))
    }
}

impl<'de> Deserialize<'de> for Metadata {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let fields = OrderedMap::<OrderedMap<u64>>::deserialize(d)?;
        Ok(Self(
            fields
                .0
                .into_iter()
                .map(|(field, values)| FieldCounts {
                    field,
                    values: values.0,
                })
                .collect(),
        ))
    }
}

/// One distinct k-mer at a position.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    /// The k-mer, upper case, in the resolved alphabet.
    pub sequence: String,
    /// Number of sequences carrying it.
    pub count: u64,
    /// `count / support × 100`.
    pub incidence: f64,
    /// Diversity motif.
    pub motif: Motif,
    /// Header-field value counts; `null` when no header format was given.
    pub metadata: Option<Metadata>,
}

impl Serialize for Variant {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(6))?;
        m.serialize_entry("sequence", &self.sequence)?;
        m.serialize_entry("count", &self.count)?;
        m.serialize_entry("incidence", &self.incidence)?;
        m.serialize_entry("motif_short", self.motif.short())?;
        m.serialize_entry("motif_long", &self.motif)?;
        m.serialize_entry("metadata", &self.metadata)?;
        m.end()
    }
}

impl<'de> Deserialize<'de> for Variant {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            sequence: String,
            count: u64,
            incidence: f64,
            motif_short: String,
            motif_long: Motif,
            metadata: Option<Metadata>,
        }
        let r = Raw::deserialize(d)?;
        if r.motif_short != r.motif_long.short() {
            return Err(de::Error::custom(format!(
                "motif_short {:?} does not match motif_long {:?}",
                r.motif_short,
                r.motif_long.long()
            )));
        }
        Ok(Self {
            sequence: r.sequence,
            count: r.count,
            incidence: r.incidence,
            motif: r.motif_long,
            metadata: r.metadata,
        })
    }
}

/// Results for one k-mer position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PositionResult {
    /// 1-based alignment column of the k-mer's first residue.
    pub position: u64,
    /// Low-support class; `null` for eligible positions (support > T).
    pub low_support: Option<SupportLabel>,
    /// Number of sequences with a valid k-mer here (*N*).
    pub support: u64,
    /// Number of distinct non-Index variants (DiMA's definition).
    pub distinct_variants_count: u64,
    /// Entropy in bits from `entropy_method`; `null` without support.
    pub entropy: Option<f64>,
    /// Observed (plug-in) entropy in bits; `null` without support.
    pub observed_entropy: Option<f64>,
    /// Method that produced `entropy`.
    pub entropy_method: EntropyMethod,
    /// Why the requested estimator was not used, if it was not.
    pub entropy_fallback: Option<EntropyFallback>,
    /// Non-Index variant types per 100 non-Index sequences; `null` when there are none.
    pub distinct_variants_incidence: Option<f64>,
    /// Non-Index sequences per 100 supporting sequences; `null` without support.
    pub total_variants_incidence: Option<f64>,
    /// Estimated sample coverage (Chao & Jost 2012); `null` when support < 2.
    pub sample_coverage: Option<f64>,
    /// Whether coverage is below `--min-coverage`; `null` when not requested or not estimable.
    pub below_min_coverage: Option<bool>,
    /// Similarity-weighted entropy H_sim in bits (Wu 2025); `null` when not requested or
    /// without support.
    pub similarity_weighted_entropy: Option<f64>,
    /// Observed entropy divided by log₂ of the number of distinct k-mers, Index included (H_n,
    /// Wu 2025); `null` when not requested or with fewer than two distinct k-mers.
    pub normalised_observed_entropy: Option<f64>,
    /// Similarity-weighted entropy divided by log₂ of the number of distinct k-mers (H_nsim,
    /// Wu 2025); `null` when not requested or with fewer than two distinct k-mers.
    pub normalised_similarity_weighted_entropy: Option<f64>,
    /// Variants ordered by count (descending), then sequence.
    pub diversity_motifs: Vec<Variant>,
}

// ---------------------------------------------------------------------------------------------
// Summaries
// ---------------------------------------------------------------------------------------------

/// An entropy extremum: the first (lowest) position attaining it and how many positions do.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extremum {
    /// 1-based position of the first position with this value.
    pub position: u64,
    /// The value, in bits.
    pub entropy: f64,
    /// Number of positions with exactly this value (≥ 1).
    pub tied_position_count: u64,
}

/// Number of positions in each support class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportLabelCounts {
    /// N = 0.
    #[serde(rename = "NS")]
    pub no_support: u64,
    /// 0 < N < T.
    #[serde(rename = "LS")]
    pub low: u64,
    /// N = T.
    #[serde(rename = "ELS")]
    pub equal_to_threshold: u64,
    /// N > T.
    pub eligible: u64,
}

/// The summary over all positions with DiMA v5.0.9's rule (positions without support count as
/// 0 bits; low-support positions contribute their observed entropy).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllPositionsSummary {
    /// The averaging rule, in words.
    pub rule: String,
    /// Number of positions averaged.
    pub position_count: u64,
    /// Mean entropy; `null` when there are no positions.
    pub average_entropy: Option<f64>,
    /// Lowest entropy (first position, tie count).
    pub lowest_entropy: Option<Extremum>,
    /// Highest entropy (first position, tie count).
    pub highest_entropy: Option<Extremum>,
}

/// Similarity-weighted entropy summary over eligible positions (support > T), with the mean
/// observed entropy of the same positions for a like-for-like comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimilarityWeightedSummary {
    /// Number of eligible positions.
    pub position_count: u64,
    /// Mean similarity-weighted entropy; `null` when no position is eligible.
    pub average_entropy: Option<f64>,
    /// Lowest similarity-weighted entropy (first position, tie count).
    pub lowest_entropy: Option<Extremum>,
    /// Highest similarity-weighted entropy (first position, tie count).
    pub highest_entropy: Option<Extremum>,
    /// Mean observed (plug-in) entropy of the same positions; `null` when none is eligible.
    pub average_observed_entropy: Option<f64>,
}

/// Sample-coverage summary (Chao & Jost 2012).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoverageSummary {
    /// Positions with support ≥ 2, where coverage is defined.
    pub evaluated: u64,
    /// Lowest coverage.
    pub min: Option<f64>,
    /// Mean coverage.
    pub mean: Option<f64>,
    /// The `--min-coverage` threshold, if given.
    pub min_coverage: Option<f64>,
    /// Positions below the threshold, if one was given.
    pub below_min_count: Option<u64>,
}

// ---------------------------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------------------------

/// Name and version of the producing tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInfo {
    /// `dima`.
    pub name: String,
    /// SemVer version.
    pub version: String,
}

/// Every parameter with its value and origin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    /// Sequence alphabet.
    pub alphabet: Param<Alphabet>,
    /// k-mer length.
    pub kmer_length: Param<u64>,
    /// Support threshold *T*.
    pub support_threshold: Param<u64>,
    /// Entropy estimator.
    pub estimator: Param<Estimator>,
    /// Whether similarity-weighted entropy was computed (absent from schema 1.0.0 files, which
    /// are read as runs without it).
    #[serde(default = "not_requested")]
    pub similarity_weighted_entropy: Param<bool>,
    /// Validation mode.
    pub validation: Param<ValidationMode>,
    /// Minimum sample coverage, if any.
    pub min_coverage: Param<Option<f64>>,
    /// Query (protein or gene) name.
    pub query_name: Param<String>,
    /// Header field names, if any.
    pub header_format: Param<Option<Vec<String>>>,
    /// Stratified header fields, if any.
    pub metadata_fields: Param<Option<Vec<String>>>,
    /// Fill value for empty or mismatched header fields.
    pub header_fill: Param<String>,
}

/// An option a file written before it existed did not have: off, by default.
const fn not_requested() -> Param<bool> {
    Param {
        value: false,
        origin: Origin::Default,
    }
}

/// How the alphabet was determined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphabetMethod {
    /// Detected from the data (Easel's rules; Eddy 2011).
    Detected,
    /// Given by the user.
    User,
}

/// Alphabet provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlphabetInfo {
    /// The alphabet used.
    pub resolved: Alphabet,
    /// How it was chosen.
    pub method: AlphabetMethod,
    /// Records examined by the detector.
    pub records_examined: u64,
    /// Residue letters examined by the detector.
    pub residues_examined: u64,
    /// T (in RNA) or U (in DNA) letters folded to the alphabet's spelling.
    pub t_u_folded: u64,
    /// Why the data contradict an explicit alphabet, if they do.
    pub contradiction: Option<String>,
}

/// Compression of the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compression {
    /// Plain text.
    None,
    /// gzip or BGZF.
    Gzip,
    /// bzip2.
    Bzip2,
    /// Zstandard.
    Zstd,
}

/// A content checksum.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    /// Algorithm (`sha-256`).
    #[serde(rename = "type")]
    pub kind: String,
    /// Lower-case hexadecimal digest.
    pub value: String,
}

/// Identity of the analysed input (D47): name, size and checksum of the bytes as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputInfo {
    /// File name without directories, or `-` for standard input.
    pub name: String,
    /// Size of the stored (possibly compressed) bytes.
    pub size_bytes: u64,
    /// Checksum of the stored bytes.
    pub checksum: Checksum,
    /// Compression detected from the content.
    pub compression: Compression,
}

/// Class of a sequence byte (see the documentation's symbol tables).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteClass {
    /// A residue of the alphabet.
    Residue,
    /// Alignment gap (`-`, `.`, `_`).
    Gap,
    /// Missing data (`~`, `?`).
    Missing,
    /// Ambiguity code (IUPAC).
    Ambiguity,
    /// Stop codon (`*`, `$`, `^`).
    Stop,
    /// Frameshift (`#`).
    Frameshift,
    /// Whitespace inside a sequence line (ignored).
    Whitespace,
    /// Not valid for the alphabet.
    Unknown,
}

impl ByteClass {
    /// Serialised name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Residue => "residue",
            Self::Gap => "gap",
            Self::Missing => "missing",
            Self::Ambiguity => "ambiguity",
            Self::Stop => "stop",
            Self::Frameshift => "frameshift",
            Self::Whitespace => "whitespace",
            Self::Unknown => "unknown",
        }
    }
}

/// Occurrences of one byte value in sequence lines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterCount {
    /// The character, when the byte is a visible ASCII character (`!` to `~`); `None` for the
    /// space, control bytes and bytes above 127.
    #[serde(rename = "char")]
    pub character: Option<String>,
    /// The byte value.
    pub byte: u8,
    /// Its class under the resolved alphabet (before case folding).
    pub class: ByteClass,
    /// Occurrences.
    pub count: u64,
}

/// Whitespace found inside sequence lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhitespaceCounts {
    /// Lines with trailing whitespace.
    pub trailing: u64,
    /// Lines with whitespace between residues.
    pub interior: u64,
}

/// Code points removed from header values, by code point.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodePointCount {
    /// `U+XXXX`.
    pub code_point: String,
    /// Occurrences.
    pub count: u64,
}

/// Header-value cleaning statistics (D59).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderCleaning {
    /// Unicode version of the character data used.
    pub unicode_version: String,
    /// Values with leading or trailing white space removed.
    pub trimmed: u64,
    /// Invisible (format and control) code points removed.
    pub invisible_removed: Vec<CodePointCount>,
    /// Values changed by NFC normalisation.
    pub nfc_changed: u64,
    /// Stratified values replaced by the fill value: empty values, and every stratified field of
    /// a header with the wrong number of fields (the same count as `run.metadata.fill_uses`).
    pub fill_uses: u64,
}

/// Input validation statistics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidationInfo {
    /// Validation mode.
    pub mode: ValidationMode,
    /// Records read.
    pub records: u64,
    /// Lower-case residue letters folded to upper case.
    pub lowercase_folded: u64,
    /// Whitespace inside sequence lines.
    pub whitespace: WhitespaceCounts,
    /// Every byte value seen in sequence lines, by byte.
    pub characters: Vec<CharacterCount>,
    /// Occurrences of characters of class `unknown`.
    pub unknown_total: u64,
    /// Header bytes replaced because they were not valid UTF-8.
    pub header_utf8_replacements: u64,
    /// Header-value cleaning; `null` when headers were not parsed.
    pub header_cleaning: Option<HeaderCleaning>,
}

/// A header whose field count differs from the header format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderMismatch {
    /// 1-based record number.
    pub record: u64,
    /// 1-based line number.
    pub line: u64,
    /// The header line (without `>`).
    pub header: String,
    /// Expected number of fields.
    pub expected: u64,
    /// Fields found.
    pub found: u64,
}

/// Header mismatches: the total and the first ones in full.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mismatches {
    /// Total mismatched headers.
    pub count: u64,
    /// The first mismatches (at most 20).
    pub first: Vec<HeaderMismatch>,
}

/// Header-metadata provenance (present when a header format was given).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataInfo {
    /// Header field names.
    pub fields: Vec<String>,
    /// Fields variants are stratified by.
    pub stratified: Vec<String>,
    /// Records examined for the field statistics behind the metadata diagnostics.
    pub probe_records: u64,
    /// Mismatched headers.
    pub mismatches: Mismatches,
    /// Stratified values replaced by the fill value: empty values, and every stratified field of
    /// a header with the wrong number of fields.
    pub fill_uses: u64,
}

/// Settings of the paper's rarefaction-regression correction as implemented (D51, D55).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rarefaction {
    /// Subsample grid: `paper_fig2`, the grid of the paper's Fig. 2 with the step and start
    /// rounding of DiMA v5.0.9 (R % from max(50, ⌈100·T/N⌉ rounded up to a multiple of 5) to 95
    /// in steps of 5, plus 100%).
    pub grid: String,
    /// Subsample size: `ceil` (⌈R·N/100⌉).
    pub size_rounding: String,
    /// Grid start rounding: `ceil_to_5`.
    pub start_rounding: String,
    /// `expected_with_replacement`: the exact expectation of with-replacement resampling.
    pub sampling: String,
    /// Minimum number of points for the regression (5).
    pub min_points: u64,
}

/// Entropy settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntropyInfo {
    /// Estimator requested.
    pub estimator: Estimator,
    /// Logarithm base (2: bits).
    pub log_base: u64,
    /// Rarefaction settings (estimator `dima` only).
    pub rarefaction: Option<Rarefaction>,
}

/// A reference cited by the methods used in the run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// Citation key.
    pub key: String,
    /// Full reference text.
    pub citation: String,
    /// DOI, if any.
    pub doi: Option<String>,
    /// URL, for references without a DOI.
    pub url: Option<String>,
}

impl From<&crate::citations::Citation> for Reference {
    fn from(c: &crate::citations::Citation) -> Self {
        Self {
            key: c.key.to_owned(),
            citation: c.text(),
            doi: c.doi.map(str::to_owned),
            url: c.url.map(str::to_owned),
        }
    }
}

impl Reference {
    /// The reference for a citation key.
    ///
    /// # Panics
    /// If the key is unknown (keys used by the library are checked by tests).
    #[must_use]
    pub fn of(key: &str) -> Self {
        crate::citations::by_key(key)
            .unwrap_or_else(|| panic!("unknown citation key {key:?}"))
            .into()
    }
}

/// Provenance of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunInfo {
    /// Producing tool.
    pub tool: ToolInfo,
    /// Parameters.
    pub parameters: Parameters,
    /// Alphabet determination.
    pub alphabet: AlphabetInfo,
    /// Alignment length (columns).
    pub alignment_length: u64,
    /// Input identity.
    pub input: InputInfo,
    /// Input validation.
    pub validation: ValidationInfo,
    /// Header metadata; `null` without a header format.
    pub metadata: Option<MetadataInfo>,
    /// Entropy settings.
    pub entropy: EntropyInfo,
    /// Conditions worth knowing about.
    pub diagnostics: Vec<Diagnostic>,
    /// The publication, this software and the references of the methods used.
    pub references: Vec<Reference>,
}

/// The complete results of an analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Results {
    /// Schema version ([`SCHEMA_VERSION`]).
    pub schema_version: String,
    /// Query (protein or gene) name.
    pub query_name: String,
    /// Number of sequences.
    pub sequence_count: u64,
    /// Support threshold *T*.
    pub support_threshold: u64,
    /// k-mer length.
    pub kmer_length: u64,
    /// Positions with support ≤ T (NS + LS + ELS), as in DiMA.
    pub low_support_count: u64,
    /// Mean entropy over eligible positions (support > T); `null` when there are none.
    pub average_entropy: Option<f64>,
    /// Lowest entropy over eligible positions.
    pub lowest_entropy: Option<Extremum>,
    /// Highest entropy over eligible positions.
    pub highest_entropy: Option<Extremum>,
    /// Number of k-mer positions (alignment length − k + 1).
    pub position_count: u64,
    /// Positions with support > T.
    pub eligible_position_count: u64,
    /// Positions per support class.
    pub support_label_counts: SupportLabelCounts,
    /// Summary over all positions with DiMA v5.0.9's rule.
    pub all_positions_summary: AllPositionsSummary,
    /// Sample-coverage summary.
    pub coverage_summary: CoverageSummary,
    /// Similarity-weighted entropy summary; `null` when it was not requested.
    pub similarity_weighted_entropy_summary: Option<SimilarityWeightedSummary>,
    /// Provenance.
    pub run: RunInfo,
    /// Per-position results, in position order.
    pub results: Vec<PositionResult>,
}

// ---------------------------------------------------------------------------------------------
// Historically conserved sequences
// ---------------------------------------------------------------------------------------------

/// Which positions may contribute to conserved regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HcsEligibility {
    /// Any position whose Index variant meets the threshold (the paper's rule; low-support
    /// positions are flagged).
    AllPositions,
    /// Only positions with support above the threshold; NS, LS and ELS positions break regions.
    SupportGtThreshold,
}

/// The results an HCS computation was derived from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HcsSource {
    /// Query name.
    pub query_name: String,
    /// k-mer length.
    pub kmer_length: u64,
    /// Support threshold *T*.
    pub support_threshold: u64,
    /// Number of sequences.
    pub sequence_count: u64,
    /// Checksum of the analysed input.
    pub input_checksum: Checksum,
    /// Version of the tool that produced the results.
    pub tool_version: String,
    /// Schema version of the results.
    pub results_schema_version: String,
}

/// Confidence-bound settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HcsConfidence {
    /// `clopper_pearson` (exact, two-sided).
    pub method: String,
    /// Confidence level (0.95).
    pub level: f64,
}

/// Parameters of an HCS computation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HcsParameters {
    /// Minimum Index incidence, per cent, in (50, 100].
    pub threshold_percent: crate::config::Percent,
    /// `overlap_or_adjacent`: consecutive qualifying k-mers join when they overlap or abut and
    /// agree on the overlap.
    pub merge_rule: String,
    /// Which positions may contribute.
    pub eligibility: HcsEligibility,
    /// Confidence bound on conservation.
    pub confidence: HcsConfidence,
}

/// A position contributing to a conserved region.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HcsMember {
    /// 1-based position.
    pub position: u64,
    /// The Index k-mer.
    pub kmer: String,
    /// Sequences carrying it.
    pub count: u64,
    /// Support of the position.
    pub support: u64,
    /// Index incidence, per cent.
    pub incidence: f64,
    /// Exact 95% Clopper–Pearson lower bound of the Index proportion (0–1).
    pub conservation_lower_bound_95: f64,
}

/// One historically conserved sequence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HcsRegion {
    /// 1-based region number.
    pub id: u64,
    /// First alignment column.
    pub start_column: u64,
    /// Last alignment column.
    pub end_column: u64,
    /// Columns spanned.
    pub length: u64,
    /// The conserved sequence.
    pub sequence: String,
    /// Lowest Index incidence among members, per cent.
    pub min_incidence: f64,
    /// Lowest support among members.
    pub min_support: u64,
    /// Lowest Clopper–Pearson lower bound among members (0–1).
    pub conservation_lower_bound_95: f64,
    /// Positions from the first to the last member with support ≤ T (members or not).
    pub low_support_positions: Vec<u64>,
    /// `true` when `low_support_positions` is not empty.
    pub flagged: bool,
    /// `true` when the previous region ended because its overlap disagreed with this one.
    pub split_from_previous: bool,
    /// Contributing positions.
    pub members: Vec<HcsMember>,
}

/// Historically conserved sequences of a results file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HcsResults {
    /// Schema version ([`HCS_SCHEMA_VERSION`]).
    pub schema_version: String,
    /// The tool that computed the regions.
    pub tool: ToolInfo,
    /// The results they were computed from.
    pub source: HcsSource,
    /// Parameters.
    pub parameters: HcsParameters,
    /// Number of regions.
    pub region_count: u64,
    /// Regions with low-support positions.
    pub flagged_region_count: u64,
    /// Regions in column order.
    pub regions: Vec<HcsRegion>,
    /// Conditions worth knowing about.
    pub diagnostics: Vec<Diagnostic>,
    /// References of the method.
    pub references: Vec<Reference>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(seq: &str, count: u64, motif: Motif, metadata: Option<Metadata>) -> Variant {
        Variant {
            sequence: seq.into(),
            count,
            incidence: 50.0,
            motif,
            metadata,
        }
    }

    #[test]
    fn variant_json_shape_and_round_trip() {
        let md = Metadata(vec![
            FieldCounts {
                field: "Year".into(),
                values: vec![("2015".into(), 2), ("2012".into(), 1)],
            },
            FieldCounts {
                field: "Host".into(),
                values: vec![("Human".into(), 3)],
            },
        ]);
        let v = variant("SVKSACIEV", 3, Motif::Index, Some(md));
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(
            json,
            r#"{"sequence":"SVKSACIEV","count":3,"incidence":50.0,"motif_short":"I","motif_long":"Index","metadata":{"Year":{"2015":2,"2012":1},"Host":{"Human":3}}}"#
        );
        let back: Variant = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v, "document order is preserved");
        let bad = json.replace(r#""motif_short":"I""#, r#""motif_short":"U""#);
        assert!(serde_json::from_str::<Variant>(&bad).is_err());
        let plain = variant("A", 1, Motif::Unique, None);
        assert!(
            serde_json::to_string(&plain)
                .unwrap()
                .ends_with(r#""motif_short":"U","motif_long":"Unique","metadata":null}"#)
        );
    }

    #[test]
    fn enum_spellings() {
        let s = |v: &dyn erased::Ser| v.json();
        assert_eq!(s(&SupportLabel::NoSupport), r#""NS""#);
        assert_eq!(s(&SupportLabel::Low), r#""LS""#);
        assert_eq!(s(&SupportLabel::EqualToThreshold), r#""ELS""#);
        assert_eq!(s(&EntropyMethod::NotEstimable), r#""not_estimable""#);
        assert_eq!(s(&EntropyMethod::PlugIn), r#""plug_in""#);
        assert_eq!(
            s(&EntropyFallback::SupportAtOrBelowThreshold),
            r#""support_at_or_below_threshold""#
        );
        assert_eq!(s(&Compression::Zstd), r#""zstd""#);
        assert_eq!(s(&ByteClass::Frameshift), r#""frameshift""#);
        assert_eq!(s(&AlphabetMethod::Detected), r#""detected""#);
        for m in [
            EntropyMethod::Dima,
            EntropyMethod::ChaoWangJost,
            EntropyMethod::MillerMadow,
            EntropyMethod::PlugIn,
            EntropyMethod::NotEstimable,
        ] {
            assert_eq!(s(&m), format!("\"{}\"", m.as_str()));
        }
        for f in [
            EntropyFallback::SupportAtOrBelowThreshold,
            EntropyFallback::TooFewPoints,
            EntropyFallback::DegenerateRegression,
            EntropyFallback::SingleObservation,
        ] {
            assert_eq!(s(&f), format!("\"{}\"", f.as_str()));
        }
        let counts = SupportLabelCounts {
            no_support: 1,
            low: 2,
            equal_to_threshold: 3,
            eligible: 4,
        };
        assert_eq!(
            serde_json::to_string(&counts).unwrap(),
            r#"{"NS":1,"LS":2,"ELS":3,"eligible":4}"#
        );
    }

    /// Minimal object-safe serialisation helper for the table-driven test above.
    mod erased {
        pub trait Ser {
            fn json(&self) -> String;
        }
        impl<T: serde::Serialize> Ser for T {
            fn json(&self) -> String {
                serde_json::to_string(self).unwrap()
            }
        }
    }
}
