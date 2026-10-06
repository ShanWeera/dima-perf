//! Analysis parameters, their defaults and their validation.
//!
//! Every default lives here, once; the CLI, the GUI and the documentation read them from this
//! module. Parameters the user did not set are `None` and resolve to these defaults (or to a
//! value derived from the data, such as the k-mer length from the alphabet), and the provenance
//! records which happened.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{DimaError, Result};

/// Default support threshold *T* (Tharanga et al. 2025, Methods: "default is 100 sequences,
/// arbitrarily defined"; the paper's Fig. 2 shows 30, DiMA v5.0.9's default).
pub const DEFAULT_SUPPORT_THRESHOLD: u64 = 100;
/// Default k-mer length for proteins: 9, the length of peptides presented by HLA class I
/// (Falk et al. 1991; Rammensee 1993; Tharanga et al. 2025).
pub const DEFAULT_KMER_PROTEIN: usize = 9;
/// Default k-mer length for nucleotides: 27, the coding sequence of a 9-mer.
pub const DEFAULT_KMER_NUCLEOTIDE: usize = 27;
/// Default value for empty or missing header fields (DiMA v5.0.9's `--header-fillna`).
pub const DEFAULT_FILL: &str = "Unknown";
/// Default HCS threshold (per cent): the paper's default, 100% (Tharanga et al. 2025, Methods).
pub const DEFAULT_HCS_THRESHOLD: Percent = Percent::HUNDRED;
/// Lower bound (exclusive) of the HCS threshold: above 50% the Index variant is unique.
pub const HCS_THRESHOLD_MIN_EXCLUSIVE: Percent = Percent {
    mantissa: 50,
    scale: 0,
};
/// Records per input batch (upper bound).
pub const BATCH_MAX_RECORDS: usize = 10_000;
/// Bytes per input batch (upper bound, sequence and header bytes).
pub const BATCH_MAX_BYTES: usize = 64 << 20;
/// Minimum number of records examined by the first-batch probe (alphabet, header statistics),
/// when the input has that many.
pub const PROBE_MIN_RECORDS: usize = 1_000;
/// Minimum number of probed records before a field can be reported as identifier-like.
pub const ID_DIAG_MIN_RECORDS: usize = 100;
/// Maximum number of mismatched headers listed in full.
pub const MISMATCH_LOG_MAX: usize = 20;

/// Sequence alphabet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Alphabet {
    /// Amino acids.
    Protein,
    /// DNA (T; U is folded to T).
    Dna,
    /// RNA (U; T is folded to U).
    Rna,
}

impl Alphabet {
    /// `true` for DNA and RNA.
    #[must_use]
    pub const fn is_nucleotide(self) -> bool {
        matches!(self, Self::Dna | Self::Rna)
    }

    /// Default k-mer length for this alphabet.
    #[must_use]
    pub const fn default_kmer_length(self) -> usize {
        if self.is_nucleotide() {
            DEFAULT_KMER_NUCLEOTIDE
        } else {
            DEFAULT_KMER_PROTEIN
        }
    }

    /// Lower-case name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Protein => "protein",
            Self::Dna => "dna",
            Self::Rna => "rna",
        }
    }

    /// Name used in generated query names ("Unknown Protein").
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Protein => "Protein",
            Self::Dna => "DNA",
            Self::Rna => "RNA",
        }
    }
}

impl fmt::Display for Alphabet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Entropy estimator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Estimator {
    /// The paper's rarefaction-regression correction (Tharanga et al. 2025, Fig. 2), computed
    /// as its exact expectation.
    #[default]
    Dima,
    /// Chao et al. (2013).
    ChaoWangJost,
    /// Miller (1955) and Madow.
    MillerMadow,
    /// Plug-in (maximum-likelihood) entropy, uncorrected.
    Raw,
}

impl Estimator {
    /// Every estimator.
    pub const ALL: [Self; 4] = [Self::Dima, Self::ChaoWangJost, Self::MillerMadow, Self::Raw];

    /// Command-line spelling.
    #[must_use]
    pub const fn cli_name(self) -> &'static str {
        match self {
            Self::Dima => "dima",
            Self::ChaoWangJost => "chao-wang-jost",
            Self::MillerMadow => "miller-madow",
            Self::Raw => "raw",
        }
    }

    /// Serialised (snake_case) name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dima => "dima",
            Self::ChaoWangJost => "chao_wang_jost",
            Self::MillerMadow => "miller_madow",
            Self::Raw => "raw",
        }
    }
}

/// How characters outside the alphabet are handled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    /// Stop at the first unknown character, reporting record and column.
    #[default]
    Strict,
    /// Exclude k-mers containing unknown characters and report one aggregated diagnostic.
    Permissive,
    /// As permissive, and print the per-character table.
    Report,
}

impl ValidationMode {
    /// Serialised name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Permissive => "permissive",
            Self::Report => "report",
        }
    }
}

/// Where a parameter's value came from (recorded in the provenance).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// The documented default.
    Default,
    /// Set by the user.
    User,
    /// Derived from the data (for example the k-mer length from the alphabet).
    Derived,
}

/// A resolved parameter value with its origin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Param<T> {
    /// The value used.
    pub value: T,
    /// Where it came from.
    pub origin: Origin,
}

impl<T> Param<T> {
    /// A user-set value if present, else `fallback` with origin `otherwise`.
    pub fn resolve(user: Option<T>, fallback: impl FnOnce() -> T, otherwise: Origin) -> Self {
        match user {
            Some(value) => Self {
                value,
                origin: Origin::User,
            },
            None => Self {
                value: fallback(),
                origin: otherwise,
            },
        }
    }
}

/// An exact decimal percentage, used for thresholds that are compared with count ratios.
///
/// `"95.5"` is stored as 955 × 10⁻¹, and [`Percent::is_met_by`] compares
/// `count / total × 100 ≥ value` in exact integer arithmetic, so a threshold never misses its
/// boundary through floating-point rounding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Percent {
    mantissa: u64,
    scale: u32,
}

impl Percent {
    /// 100%.
    pub const HUNDRED: Self = Self {
        mantissa: 100,
        scale: 0,
    };
    /// Maximum number of decimal places accepted.
    pub const MAX_DECIMALS: u32 = 9;

    /// `true` when `count / total × 100 ≥ self`; `false` when `total` is 0.
    #[must_use]
    pub fn is_met_by(self, count: u64, total: u64) -> bool {
        if total == 0 {
            return false;
        }
        let lhs = u128::from(count) * 100 * 10u128.pow(self.scale);
        let rhs = u128::from(self.mantissa) * u128::from(total);
        lhs >= rhs
    }

    /// The value as a float (for display and serialisation).
    #[must_use]
    pub fn to_f64(self) -> f64 {
        // Formatting then parsing yields the double nearest to the exact decimal.
        self.to_string()
            .parse()
            .expect("a formatted decimal always parses")
    }

    /// Parses an HCS threshold: a decimal in (50, 100].
    ///
    /// # Errors
    /// A usage error explaining the range when the text is not a decimal in (50, 100].
    pub fn hcs_threshold(text: &str) -> Result<Self> {
        let p: Self = text.parse().map_err(|e: ParsePercentError| {
            DimaError::usage(format!("invalid HCS threshold {text:?}: {e}"))
        })?;
        p.check_hcs_range()?;
        Ok(p)
    }

    /// Checks that an HCS threshold lies in (50, 100].
    ///
    /// # Errors
    /// A usage error with the rationale when out of range.
    pub fn check_hcs_range(self) -> Result<()> {
        if self > HCS_THRESHOLD_MIN_EXCLUSIVE && self <= Self::HUNDRED {
            Ok(())
        } else {
            Err(
                DimaError::usage(format!("HCS threshold {self}% is outside (50, 100]")).with_hint(
                    "above 50% the Index (most common) k-mer is unique, so a conserved sequence \
                 is well defined; use e.g. 70, 90 or 100",
                ),
            )
        }
    }

    fn cmp_value(self, other: Self) -> std::cmp::Ordering {
        let s = self.scale.max(other.scale);
        let a = u128::from(self.mantissa) * 10u128.pow(s - self.scale);
        let b = u128::from(other.mantissa) * 10u128.pow(s - other.scale);
        a.cmp(&b)
    }
}

// Values are normalised (no trailing fractional zeros), so the derived structural equality is
// value equality and agrees with this ordering.
impl Ord for Percent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_value(*other)
    }
}

impl PartialOrd for Percent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.mantissa);
        }
        let div = 10u64.pow(self.scale);
        write!(
            f,
            "{}.{:0width$}",
            self.mantissa / div,
            self.mantissa % div,
            width = self.scale as usize
        )
    }
}

/// Why a percentage could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParsePercentError {
    /// Not of the form `digits[.digits]`.
    #[error("expected a decimal number such as 70 or 95.5")]
    Syntax,
    /// More than [`Percent::MAX_DECIMALS`] decimal places.
    #[error("at most 9 decimal places are supported")]
    TooPrecise,
    /// Above 100.
    #[error("a percentage cannot exceed 100")]
    AboveHundred,
}

impl FromStr for Percent {
    type Err = ParsePercentError;

    fn from_str(text: &str) -> std::result::Result<Self, Self::Err> {
        let t = text.trim();
        let (int, frac) = t.split_once('.').unwrap_or((t, ""));
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if int.is_empty() || !digits(int) || !digits(frac) || (t.contains('.') && frac.is_empty()) {
            return Err(ParsePercentError::Syntax);
        }
        let frac = frac.trim_end_matches('0');
        let scale = u32::try_from(frac.len()).map_err(|_| ParsePercentError::TooPrecise)?;
        if scale > Self::MAX_DECIMALS {
            return Err(ParsePercentError::TooPrecise);
        }
        let int = int.trim_start_matches('0');
        if int.len() > 3 {
            return Err(ParsePercentError::AboveHundred);
        }
        let int_value: u64 = if int.is_empty() {
            0
        } else {
            int.parse().map_err(|_| ParsePercentError::Syntax)?
        };
        let frac_value: u64 = if frac.is_empty() {
            0
        } else {
            frac.parse().map_err(|_| ParsePercentError::Syntax)?
        };
        let p = Self {
            mantissa: int_value * 10u64.pow(scale) + frac_value,
            scale,
        };
        if p > Self::HUNDRED {
            return Err(ParsePercentError::AboveHundred);
        }
        Ok(p)
    }
}

impl Serialize for Percent {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_f64(self.to_f64())
    }
}

impl<'de> Deserialize<'de> for Percent {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let v = f64::deserialize(d)?;
        if !v.is_finite() || v < 0.0 {
            return Err(serde::de::Error::custom(
                "percentage must be a finite number of at least 0",
            ));
        }
        // `Display` for f64 is the shortest representation that round-trips, so a value
        // written from an exact decimal is read back as that decimal.
        format!("{v}").parse().map_err(serde::de::Error::custom)
    }
}

/// Parameters of one analysis. `None` means "use the default" (or derive from the data).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnalysisParams {
    /// Sequence alphabet; `None` detects it from the data.
    pub alphabet: Option<Alphabet>,
    /// k-mer length; `None` uses the alphabet's default.
    pub kmer_length: Option<usize>,
    /// Support threshold *T*.
    pub support_threshold: Option<u64>,
    /// Entropy estimator.
    pub estimator: Option<Estimator>,
    /// Handling of characters outside the alphabet.
    pub validation: Option<ValidationMode>,
    /// Flag positions whose sample coverage is below this value (0–1).
    pub min_coverage: Option<f64>,
    /// Name of the analysed protein or gene; `None` derives it from the input file name.
    pub query_name: Option<String>,
    /// Names of the `|`-separated header fields.
    pub header_format: Option<Vec<String>>,
    /// Header fields to stratify variants by (default: all of `header_format`).
    pub metadata_fields: Option<Vec<String>>,
    /// Value used for empty header fields and mismatched headers.
    pub header_fill: Option<String>,
}

impl AnalysisParams {
    /// The `dima analyze` options that reproduce these parameters: long option names, unset
    /// parameters omitted, header field lists joined with `|`. Front ends use it to record and
    /// show the equivalent command (D103).
    #[must_use]
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        let mut opt = |name: &str, value: Option<String>| {
            if let Some(v) = value {
                args.push(format!("--{name}"));
                args.push(v);
            }
        };
        opt("alphabet", self.alphabet.map(|a| a.as_str().to_owned()));
        opt("kmer-length", self.kmer_length.map(|k| k.to_string()));
        opt(
            "support-threshold",
            self.support_threshold.map(|t| t.to_string()),
        );
        opt("estimator", self.estimator.map(|e| e.cli_name().to_owned()));
        opt("validation", self.validation.map(|v| v.as_str().to_owned()));
        opt("min-coverage", self.min_coverage.map(|c| c.to_string()));
        opt("query-name", self.query_name.clone());
        opt(
            "header-format",
            self.header_format.as_ref().map(|f| f.join("|")),
        );
        opt(
            "metadata-fields",
            self.metadata_fields.as_ref().map(|f| f.join("|")),
        );
        opt("header-fillna", self.header_fill.clone());
        args
    }

    /// Checks the parameters that can be checked without data. Header names are checked by
    /// the header module, `k ≤ alignment length` once the alignment length is known.
    ///
    /// # Errors
    /// A usage error naming the parameter and the accepted range.
    pub fn validate(&self) -> Result<()> {
        if self.kmer_length == Some(0) {
            return Err(DimaError::usage("k-mer length must be at least 1"));
        }
        if self.support_threshold == Some(0) {
            return Err(
                DimaError::usage("support threshold must be at least 1").with_hint(
                    "positions with support above the threshold are eligible; the paper uses 100",
                ),
            );
        }
        if let Some(c) = self.min_coverage {
            if !(0.0..=1.0).contains(&c) {
                return Err(
                    DimaError::usage(format!("minimum coverage {c} is outside [0, 1]"))
                        .with_hint("sample coverage is a probability; for example 0.95"),
                );
            }
        }
        if let Some(name) = &self.query_name {
            if name.trim().is_empty() {
                return Err(DimaError::usage("query name must not be empty"));
            }
        }
        if let Some(fill) = &self.header_fill {
            if fill.trim().is_empty() {
                return Err(DimaError::usage("header fill value must not be empty")
                    .with_hint("empty values are what the fill value replaces; use e.g. Unknown"));
            }
        }
        if self.metadata_fields.is_some() && self.header_format.is_none() {
            return Err(
                DimaError::usage("metadata fields need a header format").with_hint(
                    "name the header fields with --header-format, e.g. 'Accession|Host|Year'",
                ),
            );
        }
        Ok(())
    }

    /// The support threshold with its origin.
    #[must_use]
    pub fn support_threshold(&self) -> Param<u64> {
        Param::resolve(
            self.support_threshold,
            || DEFAULT_SUPPORT_THRESHOLD,
            Origin::Default,
        )
    }

    /// The estimator with its origin.
    #[must_use]
    pub fn estimator(&self) -> Param<Estimator> {
        Param::resolve(self.estimator, Estimator::default, Origin::Default)
    }

    /// The validation mode with its origin.
    #[must_use]
    pub fn validation(&self) -> Param<ValidationMode> {
        Param::resolve(self.validation, ValidationMode::default, Origin::Default)
    }

    /// The k-mer length with its origin, given the resolved alphabet.
    #[must_use]
    pub fn kmer_length(&self, alphabet: Alphabet) -> Param<usize> {
        Param::resolve(
            self.kmer_length,
            || alphabet.default_kmer_length(),
            Origin::Derived,
        )
    }

    /// The header fill value with its origin.
    #[must_use]
    pub fn header_fill(&self) -> Param<String> {
        Param::resolve(
            self.header_fill.clone(),
            || DEFAULT_FILL.to_owned(),
            Origin::Default,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_to_arguments() {
        assert!(AnalysisParams::default().to_args().is_empty());
        let p = AnalysisParams {
            alphabet: Some(Alphabet::Dna),
            kmer_length: Some(27),
            estimator: Some(Estimator::ChaoWangJost),
            min_coverage: Some(0.95),
            header_format: Some(vec!["Accession".into(), "Host".into()]),
            ..AnalysisParams::default()
        };
        assert_eq!(
            p.to_args(),
            [
                "--alphabet",
                "dna",
                "--kmer-length",
                "27",
                "--estimator",
                "chao-wang-jost",
                "--min-coverage",
                "0.95",
                "--header-format",
                "Accession|Host"
            ]
        );
    }

    #[test]
    fn percent_parse_and_display() {
        for (s, d) in [
            ("70", "70"),
            ("70.0", "70"),
            ("095.50", "95.5"),
            ("100", "100"),
            ("100.000", "100"),
            ("0.5", "0.5"),
            ("99.999999999", "99.999999999"),
            (" 80 ", "80"),
        ] {
            assert_eq!(s.parse::<Percent>().unwrap().to_string(), d, "{s}");
        }
        for s in [
            "",
            ".5",
            "5.",
            "1e2",
            "-5",
            "+5",
            "nan",
            "100.0000000001",
            "7 0",
            "١٠",
        ] {
            assert!(s.parse::<Percent>().is_err(), "{s:?}");
        }
        assert_eq!(
            "100.1".parse::<Percent>(),
            Err(ParsePercentError::AboveHundred)
        );
        assert_eq!(
            "1000".parse::<Percent>(),
            Err(ParsePercentError::AboveHundred)
        );
        assert_eq!(
            "1.1234567891".parse::<Percent>(),
            Err(ParsePercentError::TooPrecise)
        );
    }

    #[test]
    fn percent_exact_comparison() {
        let p: Percent = "95.5".parse().unwrap();
        // 191/200 = 95.5% exactly.
        assert!(p.is_met_by(191, 200));
        assert!(!p.is_met_by(190, 200));
        let third: Percent = "66.666666667".parse().unwrap();
        assert!(
            !third.is_met_by(2, 3),
            "2/3 = 66.6666666666… < 66.666666667"
        );
        assert!(Percent::HUNDRED.is_met_by(7, 7));
        assert!(!Percent::HUNDRED.is_met_by(6, 7));
        assert!(!Percent::HUNDRED.is_met_by(0, 0));
        assert!(Percent::HUNDRED.is_met_by(u64::MAX, u64::MAX));
        // (2^64 - 2) / (2^64 - 1) = 1 - 5.4e-20 still meets 99.999999999%; no overflow.
        assert!(
            "99.999999999"
                .parse::<Percent>()
                .unwrap()
                .is_met_by(u64::MAX - 1, u64::MAX)
        );
        assert!(
            !"99.999999999"
                .parse::<Percent>()
                .unwrap()
                .is_met_by(99_999_999_998, 100_000_000_000)
        );
    }

    #[test]
    fn hcs_threshold_range() {
        assert!(Percent::hcs_threshold("50").is_err());
        assert!(Percent::hcs_threshold("50.000000001").is_ok());
        assert!(Percent::hcs_threshold("100").is_ok());
        assert!(Percent::hcs_threshold("100.5").is_err());
        assert!(Percent::hcs_threshold("abc").is_err());
        let e = Percent::hcs_threshold("40").unwrap_err();
        assert_eq!(e.to_string(), "HCS threshold 40% is outside (50, 100]");
        assert!(e.hint().unwrap().contains("Index"));
    }

    #[test]
    fn percent_serde_round_trip() {
        for s in ["70", "95.5", "100", "66.666666667"] {
            let p: Percent = s.parse().unwrap();
            let json = serde_json::to_string(&p).unwrap();
            let back: Percent = serde_json::from_str(&json).unwrap();
            assert_eq!(back, p, "{s} -> {json}");
        }
        assert_eq!(serde_json::to_string(&Percent::HUNDRED).unwrap(), "100.0");
        assert!(serde_json::from_str::<Percent>("-1").is_err());
    }

    #[test]
    fn params_validation_and_resolution() {
        let mut p = AnalysisParams::default();
        p.validate().unwrap();
        assert_eq!(
            p.support_threshold(),
            Param {
                value: 100,
                origin: Origin::Default
            }
        );
        assert_eq!(p.kmer_length(Alphabet::Dna).value, 27);
        assert_eq!(p.kmer_length(Alphabet::Protein).origin, Origin::Derived);
        assert_eq!(p.estimator().value, Estimator::Dima);
        assert_eq!(p.validation().value, ValidationMode::Strict);
        assert_eq!(p.header_fill().value, "Unknown");
        p.support_threshold = Some(30);
        assert_eq!(
            p.support_threshold(),
            Param {
                value: 30,
                origin: Origin::User
            }
        );
        for bad in [
            AnalysisParams {
                kmer_length: Some(0),
                ..Default::default()
            },
            AnalysisParams {
                support_threshold: Some(0),
                ..Default::default()
            },
            AnalysisParams {
                min_coverage: Some(1.5),
                ..Default::default()
            },
            AnalysisParams {
                min_coverage: Some(f64::NAN),
                ..Default::default()
            },
            AnalysisParams {
                header_fill: Some(" ".into()),
                ..Default::default()
            },
            AnalysisParams {
                query_name: Some(String::new()),
                ..Default::default()
            },
            AnalysisParams {
                metadata_fields: Some(vec!["a".into()]),
                ..Default::default()
            },
        ] {
            assert_eq!(
                bad.validate().unwrap_err().class(),
                crate::error::ExitClass::Usage
            );
        }
    }

    #[test]
    fn enum_names() {
        assert_eq!(
            serde_json::to_string(&Estimator::ChaoWangJost).unwrap(),
            r#""chao_wang_jost""#
        );
        assert_eq!(Estimator::ChaoWangJost.cli_name(), "chao-wang-jost");
        assert_eq!(serde_json::to_string(&Alphabet::Dna).unwrap(), r#""dna""#);
        assert_eq!(
            serde_json::to_string(&Origin::Derived).unwrap(),
            r#""derived""#
        );
        for e in Estimator::ALL {
            assert_eq!(
                serde_json::to_string(&e).unwrap(),
                format!("\"{}\"", e.as_str())
            );
        }
    }
}
