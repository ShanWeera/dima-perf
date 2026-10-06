//! Sequence alphabets: detection from residue composition and per-byte symbol classes.
//!
//! **Detection** is a port of Easel's `esl_abc_GuessAlphabet()` (Eddy 2011; Easel commit
//! 07ca83ba9ef0414dba9ce0a9331d465b5eb58f2b, `esl_alphabet.c`, BSD licence — see
//! THIRD-PARTY-NOTICES), applied to the letters of the first input batch. Easel's rules were
//! tuned to make no false calls on NCBI nr/nt, Pfam and Rfam; when unsure they return
//! "unknown", and DiMA then asks the user for `--alphabet` rather than guessing.
//!
//! **Symbol classes** (case-folded first):
//!
//! | Class | Protein | DNA / RNA |
//! |---|---|---|
//! | residue | `ACDEFGHIKLMNPQRSTVWY` + `U` (selenocysteine; Böck et al. 1991) and `O` (pyrrolysine; Srinivasan et al. 2002) | `ACGT` / `ACGU` (T and U folded to the alphabet's letter) |
//! | gap | `-` `.` `_` | same |
//! | missing | `~` `?` | same |
//! | ambiguity | `X` `B` `J` `Z` | `R Y K M S W B D H V N X I` (IUPAC-IUB; Cornish-Bowden 1985) |
//! | stop | `*` `$` `^` | same (LANL "Codes and Symbols in Sequence Alignments"; Gene Cutter help) |
//! | frameshift | `#` | same |
//! | whitespace | space, TAB, CR, VT, FF (ignored) | same |
//!
//! Every class except *residue* and *whitespace* excludes the k-mers that contain it; anything
//! else is *unknown* and handled by the validation mode.

use crate::config::Alphabet;
use crate::error::{DimaError, Result};
use crate::human::plural;
use crate::model::{AlphabetMethod, ByteClass};

/// The white-space bytes of FASTA text (D37): space, TAB, CR, VT and FF (the POSIX `space`
/// class without the line feed). They are ignored inside sequence lines, and lines made only of
/// them are skipped before the first record.
pub const WHITESPACE: &[u8] = b" \t\r\x0B\x0C";

/// `true` for a [`WHITESPACE`] byte.
#[must_use]
pub const fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | 0x0B | 0x0C)
}

/// Counts of the letters A–Z, case-insensitive (Easel's `ct[0..25]`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LetterCounts([u64; 26]);

impl LetterCounts {
    /// Adds the letters of `bytes` (non-letters are ignored).
    pub fn add(&mut self, bytes: &[u8]) {
        for &b in bytes {
            let u = b.to_ascii_uppercase();
            if u.is_ascii_uppercase() {
                self.0[usize::from(u - b'A')] += 1;
            }
        }
    }

    /// The letters of a byte histogram (`h[b]` = occurrences of byte `b`), case-insensitive.
    #[must_use]
    pub fn from_histogram(h: &[u64; 256]) -> Self {
        let mut c = Self::default();
        for (i, n) in c.0.iter_mut().enumerate() {
            let upper = b'A' + u8::try_from(i).unwrap_or(0);
            *n = h[usize::from(upper)] + h[usize::from(upper.to_ascii_lowercase())];
        }
        c
    }

    /// Adds another tally.
    pub fn merge(&mut self, other: &Self) {
        for (a, b) in self.0.iter_mut().zip(other.0) {
            *a += b;
        }
    }

    /// Count of one upper-case letter.
    #[must_use]
    pub fn get(&self, letter: u8) -> u64 {
        debug_assert!(letter.is_ascii_uppercase());
        self.0[usize::from(letter - b'A')]
    }

    /// Total letters.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.0.iter().sum()
    }
}

/// Easel's `esl_abc_GuessAlphabet()`: the alphabet implied by a letter composition, or `None`
/// when the composition does not determine it.
///
/// Rules, in order (n = total letters):
/// 1. n ≤ 10 → unknown (too little evidence);
/// 2. n > 2000 and all N → DNA (genome assemblies led by N runs);
/// 3. any of `EFIJLOPQZ` → protein;
/// 4. ≥ 98% `ACGTN` and all of A, C, G, T seen → DNA;
/// 5. ≥ 98% `ACGUN` and all of A, C, G, U seen → RNA;
/// 6. ≥ 98% amino-acid letters (or N, T, X), more `DHKMRSVWY` than `ACG`, and at least 15
///    different letters seen → protein (as written in Easel this cannot fire: when rule 3 does not
///    apply, at most 14 of these letters can occur; kept for fidelity);
/// 7. otherwise unknown.
#[must_use]
#[allow(clippy::cast_precision_loss)] // Exactly as Easel: int64 counts compared in double.
pub fn guess(ct: &LetterCounts) -> Option<Alphabet> {
    let count = |set: &[u8]| -> (u64, u64) {
        set.iter().fold((0, 0), |(n, x), &l| {
            let c = ct.get(l);
            if c > 0 { (n + c, x + 1) } else { (n, x) }
        })
    };
    let n = ct.total();
    let (n1, x1) = count(b"EFIJLOPQZ");
    let (n2, x2) = count(b"ACG");
    let (n3, x3) = count(b"DHKMRSVWY");
    let nt = ct.get(b'T');
    let nu = ct.get(b'U');
    let nx = ct.get(b'X');
    let nn = ct.get(b'N');
    let (xt, xu, xn) = (u64::from(nt > 0), u64::from(nu > 0), u64::from(nn > 0));
    let nearly_all = |k: u64| (n - k) as f64 <= 0.02 * n as f64;

    if n <= 10 {
        None
    } else if n > 2000 && nn == n {
        Some(Alphabet::Dna)
    } else if n1 > 0 {
        Some(Alphabet::Protein)
    } else if nearly_all(n2 + nt + nn) && x2 + xt == 4 {
        Some(Alphabet::Dna)
    } else if nearly_all(n2 + nu + nn) && x2 + xu == 4 {
        Some(Alphabet::Rna)
    } else if nearly_all(n1 + n2 + n3 + nn + nt + nx) && n3 > n2 && x1 + x2 + x3 + xn + xt >= 15 {
        Some(Alphabet::Protein)
    } else {
        None
    }
}

/// The alphabet decision for a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// The alphabet used.
    pub alphabet: Alphabet,
    /// Whether it was detected or given.
    pub method: AlphabetMethod,
    /// Letters examined.
    pub residues_examined: u64,
    /// Why the data contradict an explicit choice, if they do.
    pub contradiction: Option<String>,
}

/// Resolves the alphabet from the user's choice and the probe's letter counts.
///
/// # Errors
/// [`DimaError::InputData`] when no alphabet was given and the composition does not determine
/// one, with a hint to pass `--alphabet`.
pub fn resolve(
    choice: Option<Alphabet>,
    counts: &LetterCounts,
    records_examined: u64,
) -> Result<Resolution> {
    let guessed = guess(counts);
    let residues_examined = counts.total();
    if let Some(alphabet) = choice {
        let contradiction = guessed.filter(|g| *g != alphabet).map(|g| {
            format!(
                "--alphabet {alphabet} was given, but the residue composition of {} looks \
                 like {g}",
                first_records(records_examined)
            )
        });
        return Ok(Resolution {
            alphabet,
            method: AlphabetMethod::User,
            residues_examined,
            contradiction,
        });
    }
    let Some(alphabet) = guessed else {
        let (t, u) = (counts.get(b'T'), counts.get(b'U'));
        let n = residues_examined;
        let message = if n <= 10 {
            format!(
                "only {} in {}; that is too few to tell protein from nucleotide sequences",
                plural(n, "residue letter", "residue letters"),
                first_records(records_examined)
            )
        } else if t > 0 && u > 0 {
            format!(
                "the sequences contain both T ({t}) and U ({u}), so DNA and RNA cannot be told \
                 apart"
            )
        } else {
            format!(
                "the residue composition of {} ({}) does not identify the alphabet",
                first_records(records_examined),
                plural(n, "letter", "letters")
            )
        };
        return Err(DimaError::input_data(None, message)
            .with_hint("pass --alphabet protein, --alphabet dna or --alphabet rna"));
    };
    Ok(Resolution {
        alphabet,
        method: AlphabetMethod::Detected,
        residues_examined,
        contradiction: None,
    })
}

/// `the first record` or `the first N records`.
fn first_records(n: u64) -> String {
    if n == 1 {
        "the first record".to_owned()
    } else {
        format!("the first {n} records")
    }
}

/// Per-byte classification and canonical spelling for one alphabet.
///
/// `canonical(b)` is the upper-case residue (with T/U folded) for residues and 0 for every
/// other byte, so a k-mer is valid exactly when none of its canonical bytes is 0.
#[derive(Clone, Debug)]
pub struct ByteClassTable {
    alphabet: Alphabet,
    class: [ByteClass; 256],
    canonical: [u8; 256],
}

impl ByteClassTable {
    /// Builds the table for `alphabet`.
    #[must_use]
    pub fn new(alphabet: Alphabet) -> Self {
        let mut class = [ByteClass::Unknown; 256];
        let mut canonical = [0u8; 256];
        let mut set = |bytes: &[u8], c: ByteClass| {
            for &b in bytes {
                for v in [b, b.to_ascii_lowercase()] {
                    class[usize::from(v)] = c;
                }
            }
        };
        set(b"-._", ByteClass::Gap);
        set(b"~?", ByteClass::Missing);
        set(b"*$^", ByteClass::Stop);
        set(b"#", ByteClass::Frameshift);
        set(WHITESPACE, ByteClass::Whitespace);
        let (residues, ambiguity): (&[u8], &[u8]) = match alphabet {
            Alphabet::Protein => (b"ACDEFGHIKLMNPQRSTVWYUO", b"XBJZ"),
            Alphabet::Dna => (b"ACGTU", b"RYKMSWBDHVNXI"),
            Alphabet::Rna => (b"ACGUT", b"RYKMSWBDHVNXI"),
        };
        set(ambiguity, ByteClass::Ambiguity);
        set(residues, ByteClass::Residue);
        for &r in residues {
            let canon = match (alphabet, r) {
                (Alphabet::Dna, b'U') => b'T',
                (Alphabet::Rna, b'T') => b'U',
                _ => r,
            };
            canonical[usize::from(r)] = canon;
            canonical[usize::from(r.to_ascii_lowercase())] = canon;
        }
        Self {
            alphabet,
            class,
            canonical,
        }
    }

    /// The alphabet of the table.
    #[must_use]
    pub const fn alphabet(&self) -> Alphabet {
        self.alphabet
    }

    /// Class of a byte.
    #[must_use]
    pub const fn class(&self, b: u8) -> ByteClass {
        self.class[b as usize]
    }

    /// Canonical residue for a residue byte, 0 for any other byte.
    #[must_use]
    pub const fn canonical(&self, b: u8) -> u8 {
        self.canonical[b as usize]
    }

    /// `true` for the letter that is folded into the alphabet's spelling (U in DNA, T in RNA),
    /// in either case.
    #[must_use]
    pub const fn is_folded_t_u(&self, b: u8) -> bool {
        matches!(
            (self.alphabet, b),
            (Alphabet::Dna, b'U' | b'u') | (Alphabet::Rna, b'T' | b't')
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_counts_letters_like_add() {
        let text = b"ACGTacgtNnxX-.*~ \n\t\xff\x00Zz";
        let mut h = [0u64; 256];
        for &b in text {
            h[usize::from(b)] += 1;
        }
        let mut added = LetterCounts::default();
        added.add(text);
        assert_eq!(LetterCounts::from_histogram(&h), added);
        assert_eq!(added.total(), 14);
    }

    fn counts(s: &str) -> LetterCounts {
        let mut c = LetterCounts::default();
        c.add(s.as_bytes());
        c
    }

    #[test]
    fn guess_follows_easel_rules() {
        assert_eq!(guess(&counts("ACGTACGTAC")), None, "n ≤ 10");
        assert_eq!(guess(&counts("ACGTACGTACG")), Some(Alphabet::Dna));
        assert_eq!(guess(&counts("acguacguacgu")), Some(Alphabet::Rna));
        assert_eq!(guess(&counts(&"N".repeat(2001))), Some(Alphabet::Dna));
        assert_eq!(guess(&counts(&"N".repeat(2000))), None);
        assert_eq!(
            guess(&counts("ACGTACGTACGE")),
            Some(Alphabet::Protein),
            "giveaway E"
        );
        // Missing T: not all four seen.
        assert_eq!(guess(&counts("ACGACGACGACGA")), None);
        // 2% tolerance: 1 foreign letter (R) in 50 is allowed, 2 in 50 is not.
        let mut s = "ACGT".repeat(12);
        s.push_str("AR");
        assert_eq!(guess(&counts(&s)), Some(Alphabet::Dna));
        let mut s = "ACGT".repeat(12);
        s.push_str("RR");
        assert_eq!(guess(&counts(&s)), None);
        // Both T and U substantially: neither DNA nor RNA.
        assert_eq!(guess(&counts(&"ACGTU".repeat(20))), None);
        // Rule 6 needs 15 distinct letters, but without the giveaway letters only 14 (ACG,
        // DHKMRSVWY, N, T) can occur, so in the current Easel it never fires: a protein with none
        // of EFIJLOPQZ stays unknown (and DiMA asks for --alphabet). The port keeps this.
        assert_eq!(guess(&counts("MKVDHRSWYACGTNMKVDHRSWYNTKD")), None);
        // Digits and punctuation are ignored.
        assert_eq!(guess(&counts("ACGT-ACGT.ACGT 123")), Some(Alphabet::Dna));
    }

    #[test]
    fn guess_on_mers_spike_protein() {
        let fasta = include_str!("../../../samples/mers_spike_aa.fasta");
        let mut c = LetterCounts::default();
        for line in fasta.lines().filter(|l| !l.starts_with('>')) {
            c.add(line.as_bytes());
        }
        assert_eq!(guess(&c), Some(Alphabet::Protein));
    }

    #[test]
    fn resolve_reports_contradictions_and_failures() {
        let dna = counts(&"ACGT".repeat(10));
        let r = resolve(None, &dna, 3).unwrap();
        assert_eq!(
            (r.alphabet, r.method),
            (Alphabet::Dna, AlphabetMethod::Detected)
        );
        let r = resolve(Some(Alphabet::Protein), &dna, 3).unwrap();
        assert_eq!(r.method, AlphabetMethod::User);
        assert!(r.contradiction.unwrap().contains("looks like dna"));
        let e = resolve(None, &counts("ACG"), 1).unwrap_err();
        assert!(e.to_string().contains("only 3 residue letters"));
        assert!(e.hint().unwrap().contains("--alphabet"));
        let e = resolve(None, &counts(&"ACGTU".repeat(20)), 4).unwrap_err();
        assert!(e.to_string().contains("both T (20) and U (20)"));
        assert_eq!(e.class(), crate::error::ExitClass::InvalidInput);
        // An explicit choice works even without evidence.
        assert!(
            resolve(Some(Alphabet::Rna), &counts(""), 0)
                .unwrap()
                .contradiction
                .is_none()
        );
    }

    /// Every byte value × every alphabet has exactly the documented class.
    #[test]
    fn byte_tables_are_exhaustive() {
        for alphabet in [Alphabet::Protein, Alphabet::Dna, Alphabet::Rna] {
            let t = ByteClassTable::new(alphabet);
            for b in 0u8..=255 {
                let u = b.to_ascii_uppercase();
                let expected = match u {
                    b'-' | b'.' | b'_' => ByteClass::Gap,
                    b'~' | b'?' => ByteClass::Missing,
                    b'*' | b'$' | b'^' => ByteClass::Stop,
                    b'#' => ByteClass::Frameshift,
                    b' ' | b'\t' | b'\r' | 0x0B | 0x0C => ByteClass::Whitespace,
                    _ if u.is_ascii_uppercase() => {
                        let (res, amb): (&[u8], &[u8]) = match alphabet {
                            Alphabet::Protein => (b"ACDEFGHIKLMNPQRSTVWYUO", b"XBJZ"),
                            _ => (b"ACGTU", b"RYKMSWBDHVNXI"),
                        };
                        if res.contains(&u) {
                            ByteClass::Residue
                        } else if amb.contains(&u) {
                            ByteClass::Ambiguity
                        } else {
                            ByteClass::Unknown
                        }
                    }
                    _ => ByteClass::Unknown,
                };
                assert_eq!(t.class(b), expected, "{alphabet} byte {b:#04x}");
                let canon = t.canonical(b);
                if expected == ByteClass::Residue {
                    let want = match (alphabet, u) {
                        (Alphabet::Dna, b'U') => b'T',
                        (Alphabet::Rna, b'T') => b'U',
                        _ => u,
                    };
                    assert_eq!(canon, want, "{alphabet} byte {b:#04x}");
                } else {
                    assert_eq!(canon, 0, "{alphabet} byte {b:#04x}");
                }
            }
            assert_eq!(t.class(0), ByteClass::Unknown);
            assert_eq!(
                t.class(b'\n'),
                ByteClass::Unknown,
                "line feeds never reach the table"
            );
        }
        assert!(ByteClassTable::new(Alphabet::Dna).is_folded_t_u(b'u'));
        assert!(!ByteClassTable::new(Alphabet::Dna).is_folded_t_u(b't'));
        assert!(ByteClassTable::new(Alphabet::Rna).is_folded_t_u(b'T'));
        assert!(!ByteClassTable::new(Alphabet::Protein).is_folded_t_u(b'U'));
    }
}
