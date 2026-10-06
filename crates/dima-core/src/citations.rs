//! Bibliographic references: the single source for every citation surface.
//!
//! CLI help, the JSON `run.references`, `dima cite`, the GUI's *Cite DiMA* dialog and tooltips,
//! the documentation bibliography and `CITATION.cff` are all produced from this module, so a
//! reference can never disagree between surfaces.
//!
//! References are ranked by the repository's citation policy (see `AGENTS.md`): domain venues
//! first ([`Tier::Domain`]), then general statistics, mathematics (including numerical analysis),
//! HCI or computer science only where they are the primary source of a number, algorithm or
//! result ([`Tier::General`]), then standards and specifications, cited for what they define
//! ([`Tier::Standard`]).
//!
//! # In-text labels
//!
//! In-text labels ([`Citation::short`]) are computed once over the whole registry, so a work has
//! the same label on every surface (D114):
//!
//! 1. Family names and year: `Shannon 1948`, `Chao & Jost 2012`, `List et al. 2017`.
//! 2. Works that share that label show as many names as needed to tell them apart, the same
//!    number for each work of the group: `Grüning, Dale et al. 2018` and
//!    `Grüning, Chilton et al. 2018`. "et al." always stands for two or more names, so a list at
//!    most one name longer than shown is written in full (APA 7 §8.18; CSL 1.0.2
//!    `disambiguate-add-names`; biblatex `uniquelist=minyear`).
//! 3. Works whose names are still identical get year letters `a`, `b`, …, `z`, `aa`, … in title
//!    order, ignoring a leading A, An or The (APA 7 §8.19 and §9.47; CSL 1.0.2
//!    `disambiguate-add-year-suffix`).
//!
//! Letters appear in labels and in dima's own name-year reference lists
//! ([`Citation::listed_text`]: the documentation pages and the Bibliography). They never appear
//! in exported records ([`Citation::text`]: `run.references`, `dima cite`, BibTeX, RIS, CFF),
//! because a citing document assigns its own.
//!
//! A standard whose publisher gives it a short name is labelled by that designation instead,
//! without a year (D115): `RFC 8259` (RFC 7322, Section 3.5), `UAX #15` (the Unicode
//! Consortium's abbreviated citations), `WCAG 2.2` (the W3C's reference tags). Designations are
//! unique by construction and need no letters. dima's own reference lists start the entry with
//! the designation, so the label leads to it; exported records do not carry it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::sync::OnceLock;

#[path = "citations_data.rs"]
mod data;

/// Citation-policy tier of a reference (`AGENTS.md`, "Citation policy").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// Peer-reviewed work in a bioinformatics, computational-biology, virology, ecology or
    /// biostatistics venue.
    Domain,
    /// Peer-reviewed statistics, mathematics (including numerical analysis), HCI, visualisation or
    /// computer-science work, cited only as the primary source of something no domain source
    /// provides, or as a peer-reviewed statement of a classical mathematical result in the form
    /// used.
    General,
    /// A standard or specification, cited for what it defines: a W3C, IETF, Unicode, ISO/IEC or
    /// BIPM document, or a file or data format's specification by its maintainer (including a
    /// database's or tool's definition of the symbols in the files it distributes, where no
    /// peer-reviewed source documents them).
    Standard,
}

impl Tier {
    /// The label shown next to the reference on the docs *Design rationale* page.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Domain => "",
            Self::General => "[general]",
            Self::Standard => "[standard]",
        }
    }
}

/// Publication type, used to choose the BibTeX/RIS/CFF entry type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Journal article.
    Article,
    /// Paper in conference proceedings.
    Proceedings,
    /// Chapter in an edited book or a lecture-notes volume.
    Chapter,
    /// A standard or specification (RFC, W3C recommendation, Unicode annex, SI Brochure, format
    /// or symbol definition).
    Standard,
}

/// One bibliographic reference.
#[derive(Debug, PartialEq, Eq)]
pub struct Citation {
    /// Stable citation key (`familyYYYY`, lower-case ASCII).
    pub key: &'static str,
    /// Authors in publication order as `(family, given)`. A group author has an empty `given`.
    pub authors: &'static [(&'static str, &'static str)],
    /// Year of the issue (for the DiMA paper: 2025, although it was published online in 2024).
    pub year: u16,
    /// Title, sentence case as published.
    pub title: &'static str,
    /// Journal, proceedings, book or issuing body.
    pub venue: &'static str,
    /// Volume, if any.
    pub volume: Option<&'static str>,
    /// Issue, if any.
    pub issue: Option<&'static str>,
    /// Page range (`first-last`) or article number.
    pub pages: Option<&'static str>,
    /// DOI without the resolver prefix.
    pub doi: Option<&'static str>,
    /// Canonical URL, for references without a DOI.
    pub url: Option<&'static str>,
    /// Publication type.
    pub kind: Kind,
    /// Citation-policy tier.
    pub tier: Tier,
}

/// Citation key of the DiMA paper (Tharanga et al. 2025).
pub const DIMA_PAPER: &str = "tharanga2025";

/// The DiMA paper: Tharanga et al., *Brief Bioinform* 2025;26(1):bbae607.
#[must_use]
pub fn dima_paper() -> &'static Citation {
    by_key(DIMA_PAPER).expect("the DiMA paper is always present")
}

/// Authors of this software (`(family, given)`), as listed in `Cargo.toml`.
pub const SOFTWARE_AUTHORS: &[(&str, &str)] = &[("Tharanga", "Shan")];

/// Public source repository of this software.
pub const SOFTWARE_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// The documentation this build links to (D113): that of its own version (`…/vX.Y/`) for a
/// release build, that of the development version (`…/latest/`) otherwise.
pub const DOCS_URL: &str = env!("DIMA_DOCS_URL");

/// The documentation of a release of this software, e.g. `…/dima-perf/v1.0/` for 1.0.0.
#[must_use]
pub fn release_docs_url(version: &str) -> String {
    let mut parts = version.split('.');
    format!(
        "{}v{}.{}/",
        env!("CARGO_PKG_HOMEPAGE"),
        parts.next().unwrap_or("0"),
        parts.next().unwrap_or("0")
    )
}

/// Keywords describing this software (CITATION.cff, .zenodo.json).
pub const SOFTWARE_KEYWORDS: &[&str] = &[
    "bioinformatics",
    "virology",
    "sequence diversity",
    "Shannon entropy",
    "k-mer",
    "multiple sequence alignment",
];

/// Version of this software.
pub const SOFTWARE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// References without a Crossref DOI (books, older papers, specifications). Sorted by key.
static OTHER_CITATIONS: &[Citation] = &[
    Citation {
        key: "charmodnorm",
        authors: &[("Phillips", "Addison")],
        year: 2021,
        title: "Character Model for the World Wide Web: String Matching",
        venue: "W3C Working Group Note",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.w3.org/TR/2021/NOTE-charmod-norm-20210811/"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
    Citation {
        key: "dpubaria11",
        authors: &[("Garrish", "Matt"), ("Siegman", "Tzviya")],
        year: 2025,
        title: "Digital Publishing WAI-ARIA Module 1.1",
        venue: "W3C Recommendation",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.w3.org/TR/2025/REC-dpub-aria-1.1-20250612/"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
    Citation {
        key: "kuiken2003",
        authors: &[
            ("Kuiken", "Carla"),
            ("Korber", "Bette"),
            ("Shafer", "Robert W."),
        ],
        year: 2003,
        title: "HIV sequence databases",
        venue: "AIDS Reviews",
        volume: Some("5"),
        issue: Some("1"),
        pages: Some("52-61"),
        doi: None,
        url: Some("https://europepmc.org/article/MED/12875108"),
        kind: Kind::Article,
        tier: Tier::Domain,
    },
    Citation {
        key: "lanlcodes",
        authors: &[("Los Alamos HIV Sequence Database", "")],
        year: 2009,
        title: "Codes and Symbols in Sequence Alignments",
        venue: "Los Alamos National Laboratory",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.hiv.lanl.gov/content/sequence/HelpDocs/IUPAC.html"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
    Citation {
        key: "lanlgenecutter",
        authors: &[("Los Alamos HIV Sequence Database", "")],
        year: 2024,
        title: "Gene Cutter Help and Explanation",
        venue: "Los Alamos National Laboratory",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.hiv.lanl.gov/content/sequence/GENE_CUTTER/cutter-help.html"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
    Citation {
        key: "miller1955",
        authors: &[("Miller", "George A.")],
        year: 1955,
        title: "Note on the bias of information estimates",
        venue: "Information Theory in Psychology: Problems and Methods (Quastler H, ed.), Free Press, Glencoe, IL",
        volume: None,
        issue: None,
        pages: Some("95-100"),
        doi: None,
        url: None,
        kind: Kind::Chapter,
        tier: Tier::General,
    },
    Citation {
        key: "rao1945",
        authors: &[("Rao", "C. Radhakrishna")],
        year: 1945,
        title: "Information and the accuracy attainable in the estimation of statistical parameters",
        venue: "Bulletin of the Calcutta Mathematical Society",
        volume: Some("37"),
        issue: None,
        pages: Some("81-91"),
        doi: None,
        url: None,
        kind: Kind::Article,
        tier: Tier::General,
    },
    Citation {
        key: "uax15",
        authors: &[("Whistler", "Ken")],
        year: 2025,
        title: "Unicode Standard Annex #15: Unicode Normalization Forms",
        venue: "The Unicode Consortium",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.unicode.org/reports/tr15/tr15-57.html"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
    Citation {
        key: "uax44",
        authors: &[("Unicode Consortium", "")],
        year: 2025,
        title: "Unicode Standard Annex #44: Unicode Character Database",
        venue: "The Unicode Consortium",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.unicode.org/reports/tr44/tr44-36.html"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
    Citation {
        key: "wcag22",
        authors: &[
            ("Campbell", "Alastair"),
            ("Adams", "Chuck"),
            ("Bradley Montgomery", "Rachael"),
            ("Cooper", "Michael"),
            ("Kirkpatrick", "Andrew"),
        ],
        year: 2024,
        title: "Web Content Accessibility Guidelines (WCAG) 2.2",
        venue: "W3C Recommendation",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.w3.org/TR/2024/REC-WCAG22-20241212/"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
    Citation {
        key: "wcag2ict",
        authors: &[("W3C Accessibility Guidelines Working Group", "")],
        year: 2025,
        title: "Guidance on Applying WCAG 2 to Non-Web Information and Communications Technologies (WCAG2ICT)",
        venue: "W3C Group Note",
        volume: None,
        issue: None,
        pages: None,
        doi: None,
        url: Some("https://www.w3.org/TR/2025/NOTE-wcag2ict-22-20251211/"),
        kind: Kind::Standard,
        tier: Tier::Standard,
    },
];

/// The designations of standards labelled by them (see the module documentation), by key.
/// Sorted by key.
static DESIGNATIONS: &[(&str, &str)] = &[
    ("bipm2019", "SI Brochure"),
    ("dpubaria11", "DPUB-ARIA 1.1"),
    ("rfc4180", "RFC 4180"),
    ("rfc6596", "RFC 6596"),
    ("rfc7322", "RFC 7322"),
    ("rfc8259", "RFC 8259"),
    ("rfc9309", "RFC 9309"),
    ("uax15", "UAX #15"),
    ("uax44", "UAX #44"),
    ("wcag22", "WCAG 2.2"),
    ("wcag2ict", "WCAG2ICT"),
];

/// Looks a reference up by key.
#[must_use]
pub fn by_key(key: &str) -> Option<&'static Citation> {
    [data::DOI_CITATIONS, OTHER_CITATIONS]
        .into_iter()
        .find_map(|list| {
            list.binary_search_by(|c| c.key.cmp(key))
                .ok()
                .map(|i| &list[i])
        })
}

/// Every reference, sorted by key.
#[must_use]
pub fn all() -> &'static [&'static Citation] {
    static ALL: OnceLock<Vec<&'static Citation>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v: Vec<&'static Citation> =
            data::DOI_CITATIONS.iter().chain(OTHER_CITATIONS).collect();
        v.sort_unstable_by_key(|c| c.key);
        v
    })
}

/// A use of a reference, optionally pointing at a specific rule, figure or section
/// (for example `Cite::new("list2017").at("Rule 5")`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cite {
    /// Citation key.
    pub key: &'static str,
    /// Location inside the work ("Rule 5", "Fig. 2", "SC 1.4.11").
    pub locator: Option<&'static str>,
}

impl Cite {
    /// A whole-work citation.
    #[must_use]
    pub const fn new(key: &'static str) -> Self {
        Self { key, locator: None }
    }

    /// The same citation pointing at a specific location.
    #[must_use]
    pub const fn at(self, locator: &'static str) -> Self {
        Self {
            key: self.key,
            locator: Some(locator),
        }
    }

    /// The referenced work.
    ///
    /// # Panics
    /// If the key is unknown; every key used in the code base is checked by a unit test.
    #[must_use]
    pub fn citation(self) -> &'static Citation {
        by_key(self.key).unwrap_or_else(|| panic!("unknown citation key {:?}", self.key))
    }

    /// Short in-text form with locator, e.g. `List et al. 2017, Rule 5`.
    #[must_use]
    pub fn short(self) -> String {
        let mut s = self.citation().short();
        if let Some(loc) = self.locator {
            s.push_str(", ");
            s.push_str(loc);
        }
        s
    }

    /// Narrative in-text form, with the year and locator in parentheses, e.g.
    /// `Tharanga et al. (2025, Fig. 2)`.
    #[must_use]
    pub fn narrative(self) -> String {
        let c = self.citation();
        if let Some(d) = c.designation() {
            return self
                .locator
                .map_or_else(|| d.to_owned(), |loc| format!("{d} ({loc})"));
        }
        let mut s = format!("{} ({}", c.label_names(), c.listed_year());
        if let Some(loc) = self.locator {
            s.push_str(", ");
            s.push_str(loc);
        }
        s.push(')');
        s
    }

    /// Short form with locator and DOI (or URL), as used at the end of CLI long help,
    /// e.g. `Chao et al. 2013, doi:10.1111/2041-210X.12108`.
    #[must_use]
    pub fn reference(self) -> String {
        let c = self.citation();
        let mut s = self.short();
        if let Some(link) = c.link() {
            s.push_str(", ");
            s.push_str(&link);
        }
        s
    }
}

/// Initials of a given name: the first letter of each space-, hyphen- or dot-separated part
/// (`"Hans-Georg"` → `"HG"`, `"C. Titus"` → `"CT"`).
fn initials(given: &str) -> String {
    given
        .split(|c: char| c.is_whitespace() || c == '-' || c == '.')
        .filter_map(|part| part.chars().next())
        .flat_map(char::to_uppercase)
        .collect()
}

/// Escapes a BibTeX field value: LaTeX special characters are backslash-escaped and braces are
/// balanced by escaping them too.
pub(crate) fn bibtex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(ch);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\\' => out.push_str("\\textbackslash{}"),
            _ => out.push(ch),
        }
    }
    out
}

/// Splits `first-last` into its two pages; an article number yields `(number, None)`.
fn page_range(pages: &str) -> (&str, Option<&str>) {
    match pages.split_once('-') {
        Some((a, b)) if !a.is_empty() && !b.is_empty() => (a, Some(b)),
        _ => (pages, None),
    }
}

/// Upper bound on authors listed in full by [`Citation::text`] before `et al.` (APA 7 lists up to
/// 20 authors; long consortium author lists would otherwise dominate the output).
const TEXT_MAX_AUTHORS: usize = 20;

/// The in-text label of a reference without its year: the family names (`names`) and the year
/// letter that tells it apart from works with identical names and year (`suffix`, usually empty).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Label {
    names: String,
    suffix: String,
}

/// The family names of an in-text label: the first `n`, then "et al.", which always stands for at
/// least two names (APA 7 §8.18), so a list at most one name longer than `n` is written in full.
/// A reference without authors is labelled by its issuing body.
fn label_names(c: &Citation, n: usize) -> String {
    let family: Vec<&str> = c.authors.iter().map(|&(f, _)| f).collect();
    match family.as_slice() {
        [] => c.venue.to_owned(),
        [a] => (*a).to_owned(),
        [rest @ .., last] if family.len() <= n + 1 => format!("{} & {last}", rest.join(", ")),
        _ => format!("{} et al.", family[..n].join(", ")),
    }
}

/// Year letter number `i` (from 0): `a` … `z`, `aa`, `ab`, … (CSL 1.0.2 year-suffix).
fn year_suffix(mut i: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push(char::from(b'a' + (i % 26) as u8));
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    letters.iter().rev().collect()
}

/// The title order of APA 7 §9.47: case-insensitive, ignoring a leading A, An or The.
fn title_order(title: &str) -> String {
    let t = title.to_lowercase();
    ["a ", "an ", "the "]
        .iter()
        .find_map(|article| t.strip_prefix(article))
        .unwrap_or(&t)
        .to_owned()
}

/// The labels of `cites` by key, by the rule in the module documentation.
fn disambiguate<'a>(cites: &[&'a Citation]) -> HashMap<&'a str, Label> {
    let mut groups: BTreeMap<(String, u16), Vec<&'a Citation>> = BTreeMap::new();
    for &c in cites {
        groups
            .entry((label_names(c, 1), c.year))
            .or_default()
            .push(c);
    }
    let mut out = HashMap::with_capacity(cites.len());
    for group in groups.into_values() {
        let longest = group
            .iter()
            .map(|c| c.authors.len())
            .max()
            .unwrap_or(1)
            .max(1);
        let distinct = |n: usize| {
            group
                .iter()
                .map(|c| label_names(c, n))
                .collect::<BTreeSet<_>>()
                .len()
        };
        let most = distinct(longest);
        let n = (1..=longest)
            .find(|&n| distinct(n) == most)
            .unwrap_or(longest);
        let mut same: BTreeMap<String, Vec<&'a Citation>> = BTreeMap::new();
        for &c in &group {
            same.entry(label_names(c, n)).or_default().push(c);
        }
        for (names, mut works) in same {
            if let [only] = works.as_slice() {
                out.insert(
                    only.key,
                    Label {
                        names,
                        suffix: String::new(),
                    },
                );
                continue;
            }
            works.sort_by_cached_key(|c| (title_order(c.title), c.key));
            for (i, c) in works.into_iter().enumerate() {
                out.insert(
                    c.key,
                    Label {
                        names: names.clone(),
                        suffix: year_suffix(i),
                    },
                );
            }
        }
    }
    out
}

/// The name-year label of every reference in the registry without a designation, by key.
fn labels() -> &'static HashMap<&'static str, Label> {
    static LABELS: OnceLock<HashMap<&'static str, Label>> = OnceLock::new();
    LABELS.get_or_init(|| {
        let named: Vec<&'static Citation> = all()
            .iter()
            .copied()
            .filter(|c| c.designation().is_none())
            .collect();
        disambiguate(&named)
    })
}

impl Citation {
    /// `true` if this is the registry's entry for its key.
    fn in_registry(&self) -> bool {
        by_key(self.key).is_some_and(|c| c == self)
    }

    /// The registry's name-year label of this reference, if it is the registry's entry for its
    /// key and has no designation.
    fn registry_label(&self) -> Option<&'static Label> {
        labels().get(self.key).filter(|_| self.in_registry())
    }

    /// The short name its publisher gives this standard, used as its label: `RFC 8259`,
    /// `UAX #15`, `WCAG 2.2`; `None` for every other reference.
    #[must_use]
    pub fn designation(&self) -> Option<&'static str> {
        DESIGNATIONS
            .binary_search_by(|&(k, _)| k.cmp(self.key))
            .ok()
            .map(|i| DESIGNATIONS[i].1)
            .filter(|_| self.in_registry())
    }

    /// The family names of the in-text label: `Tharanga et al.`, `Grüning, Dale et al.`. A
    /// reference outside the registry has the plain form.
    fn label_names(&self) -> String {
        self.registry_label()
            .map_or_else(|| label_names(self, 1), |l| l.names.clone())
    }

    /// The in-text label, the same on every surface: `Tharanga et al. 2025`,
    /// `Grüning, Dale et al. 2018`, `RFC 8259` (see the module documentation for the rule).
    #[must_use]
    pub fn short(&self) -> String {
        match self.designation() {
            Some(d) => d.to_owned(),
            None => format!("{} {}", self.label_names(), self.listed_year()),
        }
    }

    /// The year as dima's own name-year lists show it: `2018`, or `2024a` when a year letter
    /// tells apart works with identical authors and year.
    #[must_use]
    pub fn listed_year(&self) -> String {
        let suffix = self.registry_label().map_or("", |l| l.suffix.as_str());
        format!("{}{suffix}", self.year)
    }

    /// `doi:…` if the reference has a DOI, else its URL.
    #[must_use]
    pub fn link(&self) -> Option<String> {
        self.doi
            .map(|d| format!("doi:{d}"))
            .or_else(|| self.url.map(str::to_owned))
    }

    /// Resolvable URL: `https://doi.org/…` for a DOI, else the stored URL.
    #[must_use]
    pub fn href(&self) -> Option<String> {
        self.doi
            .map(|d| format!("https://doi.org/{d}"))
            .or_else(|| self.url.map(str::to_owned))
    }

    /// One author in Vancouver form: `Tharanga S`; a group author verbatim.
    fn vancouver_author((family, given): (&str, &str)) -> String {
        let ini = initials(given);
        if ini.is_empty() {
            family.to_owned()
        } else {
            format!("{family} {ini}")
        }
    }

    /// Full plain-text reference (Vancouver/NLM style), as exported (`run.references`,
    /// `dima cite`), e.g.
    /// `Tharanga S, Ünlü ES, … Khan AM. DiMA: sequence diversity dynamics analyser for viruses.
    /// Briefings in Bioinformatics. 2025;26(1):bbae607. doi:10.1093/bib/bbae607`.
    #[must_use]
    pub fn text(&self) -> String {
        self.render(&self.year.to_string())
    }

    /// [`Citation::text`] as dima's own reference lists show it (the documentation and its
    /// Bibliography): led by the designation of a standard that has one, and with a year letter
    /// where the label has one ([`Citation::listed_year`]).
    #[must_use]
    pub fn listed_text(&self) -> String {
        let text = self.render(&self.listed_year());
        match self.designation() {
            Some(d) => format!("{d}. {text}"),
            None => text,
        }
    }

    /// The full reference with `year` as its year.
    fn render(&self, year: &str) -> String {
        let mut s = String::new();
        let listed = self.authors.len().min(TEXT_MAX_AUTHORS);
        for (i, &a) in self.authors[..listed].iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&Self::vancouver_author(a));
        }
        if self.authors.len() > listed {
            s.push_str(", et al");
        }
        if !s.is_empty() {
            s.push_str(". ");
        }
        s.push_str(self.title);
        if !self.title.ends_with(['?', '!', '.']) {
            s.push('.');
        }
        match self.kind {
            Kind::Article => {
                let _ = write!(s, " {}. {year}", self.venue);
                if let Some(v) = self.volume {
                    let _ = write!(s, ";{v}");
                    if let Some(i) = self.issue {
                        let _ = write!(s, "({i})");
                    }
                }
                if let Some(p) = self.pages {
                    let _ = write!(s, ":{p}");
                }
                s.push('.');
            }
            Kind::Proceedings | Kind::Chapter => {
                let _ = write!(s, " In: {}. {year}", self.venue);
                if let Some(v) = self.volume {
                    let _ = write!(s, ";{v}");
                }
                if let Some(p) = self.pages {
                    let _ = write!(s, ". p. {p}");
                }
                s.push('.');
            }
            Kind::Standard => {
                let _ = write!(s, " {}; {year}.", self.venue);
            }
        }
        if let Some(d) = self.doi {
            let _ = write!(s, " doi:{d}");
        } else if let Some(u) = self.url {
            let _ = write!(s, " {u}");
        }
        s
    }

    /// BibTeX entry.
    #[must_use]
    pub fn bibtex(&self) -> String {
        let (ty, venue_field) = match self.kind {
            Kind::Article => ("article", "journal"),
            Kind::Proceedings => ("inproceedings", "booktitle"),
            Kind::Chapter => ("incollection", "booktitle"),
            Kind::Standard => ("misc", "howpublished"),
        };
        let authors = self
            .authors
            .iter()
            .map(|&(family, given)| {
                if given.is_empty() {
                    // Braces keep a group author from being parsed as "Last, First".
                    format!("{{{}}}", bibtex_escape(family))
                } else {
                    format!("{{{}}}, {}", bibtex_escape(family), bibtex_escape(given))
                }
            })
            .collect::<Vec<_>>()
            .join(" and ");
        let mut s = format!("@{ty}{{{},\n", self.key);
        let mut field = |name: &str, value: &str| {
            let _ = writeln!(s, "  {name:<9} = {{{value}}},");
        };
        if !authors.is_empty() {
            field("author", &authors);
        }
        // Double braces preserve the published capitalisation.
        field("title", &format!("{{{}}}", bibtex_escape(self.title)));
        field(venue_field, &bibtex_escape(self.venue));
        field("year", &self.year.to_string());
        if let Some(v) = self.volume {
            field("volume", &bibtex_escape(v));
        }
        if let Some(i) = self.issue {
            field("number", &bibtex_escape(i));
        }
        if let Some(p) = self.pages {
            match page_range(p) {
                (a, Some(b)) => field("pages", &format!("{a}--{b}")),
                (n, None) => field("pages", &bibtex_escape(n)),
            }
        }
        if let Some(d) = self.doi {
            field("doi", d);
        }
        if let Some(u) = self.url {
            field("url", u);
        }
        s.push('}');
        s
    }

    /// RIS record (one tag per line, terminated by `ER  -`).
    #[must_use]
    pub fn ris(&self) -> String {
        let ty = match self.kind {
            Kind::Article => "JOUR",
            Kind::Proceedings => "CONF",
            Kind::Chapter => "CHAP",
            Kind::Standard => "STAND",
        };
        let mut s = String::new();
        let mut tag = |t: &str, v: &str| {
            let _ = writeln!(s, "{t}  - {v}");
        };
        tag("TY", ty);
        for &(family, given) in self.authors {
            if given.is_empty() {
                tag("AU", family);
            } else {
                tag("AU", &format!("{family}, {given}"));
            }
        }
        tag("TI", self.title);
        tag("T2", self.venue);
        tag("PY", &self.year.to_string());
        if let Some(v) = self.volume {
            tag("VL", v);
        }
        if let Some(i) = self.issue {
            tag("IS", i);
        }
        if let Some(p) = self.pages {
            let (a, b) = page_range(p);
            tag("SP", a);
            if let Some(b) = b {
                tag("EP", b);
            }
        }
        if let Some(d) = self.doi {
            tag("DO", d);
        }
        if let Some(u) = self.href() {
            tag("UR", &u);
        }
        tag("ER", "");
        // RIS readers expect "ER  - " followed by end of line; keep the trailing space.
        s
    }

    /// The reference as a CFF 1.2.0 `reference` object, as YAML lines indented by `indent`
    /// spaces (the first line starts with `type:`).
    #[must_use]
    pub fn cff(&self, indent: usize) -> String {
        let pad = " ".repeat(indent);
        let q = |v: &str| serde_json::to_string(v).expect("strings always serialise");
        let ty = match self.kind {
            Kind::Article => "article",
            Kind::Proceedings => "conference-paper",
            Kind::Chapter => "book",
            Kind::Standard => "standard",
        };
        let mut s = String::new();
        let _ = writeln!(s, "{pad}type: {ty}");
        let _ = writeln!(s, "{pad}authors:");
        for &(family, given) in self.authors {
            if given.is_empty() {
                let _ = writeln!(s, "{pad}  - name: {}", q(family));
            } else {
                let _ = writeln!(s, "{pad}  - family-names: {}", q(family));
                let _ = writeln!(s, "{pad}    given-names: {}", q(given));
            }
        }
        let _ = writeln!(s, "{pad}title: {}", q(self.title));
        let venue_key = match self.kind {
            Kind::Article => "journal",
            Kind::Proceedings => "conference",
            Kind::Chapter => "collection-title",
            Kind::Standard => "publisher",
        };
        if self.kind == Kind::Proceedings || self.kind == Kind::Standard {
            let _ = writeln!(s, "{pad}{venue_key}:");
            let _ = writeln!(s, "{pad}  name: {}", q(self.venue));
        } else {
            let _ = writeln!(s, "{pad}{venue_key}: {}", q(self.venue));
        }
        let _ = writeln!(s, "{pad}year: {}", self.year);
        if let Some(v) = self.volume {
            let _ = writeln!(s, "{pad}volume: {}", q(v));
        }
        if let Some(i) = self.issue {
            let _ = writeln!(s, "{pad}issue: {}", q(i));
        }
        if let Some(p) = self.pages {
            match page_range(p) {
                (a, Some(b)) => {
                    let _ = writeln!(s, "{pad}start: {}", q(a));
                    let _ = writeln!(s, "{pad}end: {}", q(b));
                }
                (n, None) => {
                    let _ = writeln!(s, "{pad}start: {}", q(n));
                }
            }
        }
        if let Some(d) = self.doi {
            let _ = writeln!(s, "{pad}doi: {}", q(d));
        }
        if let Some(u) = self.url {
            let _ = writeln!(s, "{pad}url: {}", q(u));
        }
        s
    }
}

/// Plain-text citation of a release of this software (usually [`SOFTWARE_VERSION`]; the version
/// recorded in a results file for `dima cite --from`), e.g.
/// `Tharanga S. dima: sequence diversity dynamics analyser for viruses (Rust implementation),
/// version 1.0.0. https://github.com/…`.
#[must_use]
pub fn software_text(version: &str) -> String {
    let authors = SOFTWARE_AUTHORS
        .iter()
        .map(|&a| Citation::vancouver_author(a))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{authors}. {SOFTWARE_TITLE}, version {version} [software]. {SOFTWARE_REPOSITORY}")
}

/// Title of this software in citations.
pub const SOFTWARE_TITLE: &str =
    "dima: sequence diversity dynamics analyser for viruses (Rust implementation)";

/// BibTeX entry for a release of this software.
#[must_use]
pub fn software_bibtex(version: &str) -> String {
    let authors = SOFTWARE_AUTHORS
        .iter()
        .map(|&(f, g)| format!("{{{f}}}, {g}"))
        .collect::<Vec<_>>()
        .join(" and ");
    let key = format!("dima_rs_{}", version.replace(['.', '-', '+'], "_"));
    let title = bibtex_escape(SOFTWARE_TITLE);
    format!(
        "@software{{{key},\n  author    = {{{authors}}},\n  title     = {{{{{title}}}}},\n  \
         version   = {{{version}}},\n  url       = {{{SOFTWARE_REPOSITORY}}},\n  \
         note      = {{Implements the method of Tharanga et al. 2025, doi:10.1093/bib/bbae607}},\n}}"
    )
}

/// RIS record for a release of this software.
#[must_use]
pub fn software_ris(version: &str) -> String {
    let mut s = String::from("TY  - COMP\n");
    for &(f, g) in SOFTWARE_AUTHORS {
        let _ = writeln!(s, "AU  - {f}, {g}");
    }
    let _ = writeln!(s, "TI  - {SOFTWARE_TITLE}");
    let _ = writeln!(s, "ET  - {version}");
    let _ = writeln!(s, "UR  - {SOFTWARE_REPOSITORY}");
    s.push_str("ER  - \n");
    s
}

/// A CFF 1.2.0 document describing a release of this software, with the DiMA paper as
/// `preferred-citation` and `extra` as additional `references` (the content of `CITATION.cff`,
/// which `cargo xtask gen` prefixes with a generated-file comment).
#[must_use]
pub fn cff_document(version: &str, extra: &[&Citation]) -> String {
    let q = |v: &str| serde_json::to_string(v).expect("strings always serialise");
    let mut s = String::new();
    s.push_str("cff-version: 1.2.0\n");
    s.push_str("message: \"If you use DiMA, please cite the DiMA paper (preferred-citation) and the software version you used.\"\n");
    s.push_str("type: software\n");
    let _ = writeln!(s, "title: {}", q(SOFTWARE_TITLE));
    let _ = writeln!(s, "version: {}", q(version));
    s.push_str("license: MIT\n");
    let _ = writeln!(s, "repository-code: {}", q(SOFTWARE_REPOSITORY));
    let _ = writeln!(s, "url: {}", q(&release_docs_url(version)));
    s.push_str("authors:\n");
    for &(f, g) in SOFTWARE_AUTHORS {
        let _ = writeln!(s, "  - family-names: {}\n    given-names: {}", q(f), q(g));
    }
    s.push_str("keywords:\n");
    for k in SOFTWARE_KEYWORDS {
        let _ = writeln!(s, "  - {}", q(k));
    }
    s.push_str("preferred-citation:\n");
    s.push_str(&dima_paper().cff(2));
    if !extra.is_empty() {
        s.push_str("references:\n");
        for c in extra {
            let body = c.cff(4);
            // First line becomes the list item.
            let body = body.replacen("    type:", "  - type:", 1);
            s.push_str(&body);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_sorted_unique_and_well_formed() {
        for list in [data::DOI_CITATIONS, OTHER_CITATIONS] {
            for w in list.windows(2) {
                assert!(w[0].key < w[1].key, "{} !< {}", w[0].key, w[1].key);
            }
        }
        let mut keys: Vec<_> = all().iter().map(|c| c.key).collect();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate keys across the two lists");
        for c in all() {
            assert!(
                c.key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
                "{}",
                c.key
            );
            assert!(by_key(c.key).is_some());
        }
        assert!(by_key("nope1900").is_none());
    }

    #[test]
    fn records_are_complete() {
        for c in all() {
            assert!(!c.title.trim().is_empty(), "{}", c.key);
            assert!(!c.venue.trim().is_empty(), "{}", c.key);
            assert!(!c.authors.is_empty(), "{}", c.key);
            assert!(
                c.doi.is_some() || c.url.is_some() || c.kind != Kind::Standard,
                "{}",
                c.key
            );
            assert!((1900..=2100).contains(&c.year), "{}", c.key);
            for &(f, g) in c.authors {
                assert!(!f.trim().is_empty(), "{}", c.key);
                assert_eq!(f.trim(), f, "{}", c.key);
                assert_eq!(g.trim(), g, "{}", c.key);
            }
            for s in [c.title, c.venue] {
                assert!(
                    !s.contains('<') && !s.contains("&amp;") && !s.contains("  "),
                    "{}",
                    c.key
                );
            }
            if let Some(d) = c.doi {
                assert!(d.starts_with("10.") && d.contains('/'), "{}", c.key);
                assert!(!d.contains(char::is_whitespace), "{}", c.key);
            }
            if let Some(u) = c.url {
                assert!(u.starts_with("https://"), "{}", c.key);
            }
            if c.tier == Tier::Standard {
                assert_eq!(c.kind, Kind::Standard, "{}", c.key);
            }
        }
    }

    #[test]
    fn dima_paper_is_exact() {
        let p = dima_paper();
        assert_eq!(p.doi, Some("10.1093/bib/bbae607"));
        assert_eq!(p.year, 2025);
        assert_eq!(p.short(), "Tharanga et al. 2025");
        assert_eq!(
            p.text(),
            "Tharanga S, Ünlü ES, Hu Y, Sjaugi MF, Çelik MA, Hekimoğlu H, Miotto O, Öncel MM, \
             Khan AM. DiMA: sequence diversity dynamics analyser for viruses. Briefings in \
             Bioinformatics. 2025;26(1):bbae607. doi:10.1093/bib/bbae607"
        );
    }

    #[test]
    fn short_forms() {
        assert_eq!(by_key("shannon1948").unwrap().short(), "Shannon 1948");
        assert_eq!(by_key("chao2012").unwrap().short(), "Chao & Jost 2012");
        assert_eq!(
            Cite::new("list2017").at("Rule 5").short(),
            "List et al. 2017, Rule 5"
        );
        assert_eq!(
            Cite::new("chao2013").reference(),
            "Chao et al. 2013, doi:10.1111/2041-210X.12108"
        );
        assert_eq!(
            Cite::new("wcag22").at("SC 1.4.11").reference(),
            "WCAG 2.2, SC 1.4.11, https://www.w3.org/TR/2024/REC-WCAG22-20241212/"
        );
        assert_eq!(
            Cite::new("rfc6596").at("Section 3").narrative(),
            "RFC 6596 (Section 3)"
        );
        assert_eq!(by_key("uax15").unwrap().short(), "UAX #15");
    }

    #[test]
    fn labels_are_unique() {
        let mut seen = HashMap::new();
        for c in all() {
            if let Some(other) = seen.insert(c.short(), c.key) {
                panic!("{} and {} share the label {:?}", other, c.key, c.short());
            }
        }
    }

    #[test]
    fn colliding_works_add_names() {
        for (key, label) in [
            ("gruning2018bioconda", "Grüning, Dale et al. 2018"),
            ("gruning2018repro", "Grüning, Chilton et al. 2018"),
            ("mangul2019bench", "Mangul, Martin et al. 2019"),
            ("mangul2019install", "Mangul, Mosqueiro et al. 2019"),
        ] {
            assert_eq!(by_key(key).unwrap().short(), label);
        }
        assert_eq!(
            Cite::new("gruning2018repro").at("Box 1").narrative(),
            "Grüning, Chilton et al. (2018, Box 1)"
        );
        assert_eq!(
            Cite::new("tharanga2025").at("Fig. 2").narrative(),
            "Tharanga et al. (2025, Fig. 2)"
        );
    }

    #[test]
    fn designations_label_standards_only() {
        for w in DESIGNATIONS.windows(2) {
            assert!(w[0].0 < w[1].0, "{} !< {}", w[0].0, w[1].0);
        }
        for &(key, d) in DESIGNATIONS {
            let c = by_key(key).unwrap_or_else(|| panic!("{key} is not in the registry"));
            assert_eq!(c.tier, Tier::Standard, "{key}");
            assert!(
                !d.is_empty() && d.trim() == d && d.is_ascii(),
                "{key}: {d:?}"
            );
            assert_eq!(c.short(), d);
            assert!(
                labels().get(key).is_none(),
                "{key} also has a name-year label"
            );
            assert_eq!(c.listed_text(), format!("{d}. {}", c.text()));
            assert!(
                !c.text().starts_with(d),
                "{key}: exported text carries the designation"
            );
        }
    }

    #[test]
    fn et_al_stands_for_at_least_two_names() {
        for c in all() {
            let Some(label) = labels().get(c.key) else {
                continue;
            };
            if let Some(shown) = label.names.strip_suffix(" et al.") {
                let names = shown.split(", ").count();
                assert!(c.authors.len() >= names + 2, "{}: {:?}", c.key, label);
            }
        }
    }

    #[test]
    fn year_letters_only_for_identical_authors() {
        // Different people with the same family names would need initials (APA 7 §8.20), not
        // letters, which say "the same authors".
        let mut lettered: BTreeMap<(&str, u16), Vec<&Citation>> = BTreeMap::new();
        for c in all() {
            let Some(label) = labels().get(c.key) else {
                continue;
            };
            if !label.suffix.is_empty() {
                lettered
                    .entry((label.names.as_str(), c.year))
                    .or_default()
                    .push(c);
            }
        }
        for works in lettered.values() {
            let people = |c: &Citation| -> Vec<(String, String)> {
                c.authors
                    .iter()
                    .map(|&(f, g)| (f.to_owned(), initials(g)))
                    .collect()
            };
            assert!(
                works.iter().all(|c| people(c) == people(works[0])),
                "different people share a label: add initials (APA 7 §8.20): {:?}",
                works.iter().map(|c| c.key).collect::<Vec<_>>()
            );
        }
        // No year letter reaches the exported text.
        for c in all() {
            assert!(c.text().contains(&c.year.to_string()), "{}", c.key);
            if c.designation().is_none() && labels()[c.key].suffix.is_empty() {
                assert_eq!(c.listed_text(), c.text(), "{}", c.key);
            }
        }
    }

    fn fixture(
        key: &'static str,
        authors: &'static [(&'static str, &'static str)],
        title: &'static str,
    ) -> Citation {
        Citation {
            key,
            authors,
            year: 2018,
            title,
            venue: "Venue",
            volume: None,
            issue: None,
            pages: None,
            doi: None,
            url: None,
            kind: Kind::Article,
            tier: Tier::Domain,
        }
    }

    fn fixture_labels(fixtures: &[Citation]) -> Vec<String> {
        let refs: Vec<&Citation> = fixtures.iter().collect();
        let labels = disambiguate(&refs);
        fixtures
            .iter()
            .map(|c| {
                let l = &labels[c.key];
                format!("{} {}{}", l.names, c.year, l.suffix)
            })
            .collect()
    }

    #[test]
    fn references_outside_the_registry_have_plain_labels() {
        let outside = fixture(
            "gruning2018repro",
            &[("Grüning", "B"), ("Other", "A"), ("Third", "C")],
            "Elsewhere",
        );
        assert_eq!(outside.short(), "Grüning et al. 2018");
        assert_eq!(outside.listed_text(), outside.text());
        let unknown = fixture("nope2018", &[("Doe", "J")], "Unknown");
        assert_eq!(unknown.short(), "Doe 2018");
    }

    #[test]
    fn label_rule_on_fixtures() {
        // APA 7 §8.18: names until the lists differ.
        let kapoor = [
            fixture(
                "k1",
                &[
                    ("Kapoor", "A"),
                    ("Bloom", "B"),
                    ("Montez", "C"),
                    ("Lee", "D"),
                    ("Ng", "E"),
                ],
                "One",
            ),
            fixture(
                "k2",
                &[
                    ("Kapoor", "A"),
                    ("Bloom", "B"),
                    ("Zucker", "C"),
                    ("Ng", "D"),
                    ("Lee", "E"),
                ],
                "Two",
            ),
        ];
        assert_eq!(
            fixture_labels(&kapoor),
            [
                "Kapoor, Bloom, Montez et al. 2018",
                "Kapoor, Bloom, Zucker et al. 2018"
            ]
        );
        // Only the last name differs: "et al." cannot stand for one name, so all are written.
        let hasan = [
            fixture(
                "h1",
                &[
                    ("Hasan", "A"),
                    ("Liang", "B"),
                    ("Kahn", "C"),
                    ("Jones-Miller", "D"),
                ],
                "One",
            ),
            fixture(
                "h2",
                &[
                    ("Hasan", "A"),
                    ("Liang", "B"),
                    ("Kahn", "C"),
                    ("Weintraub", "D"),
                ],
                "Two",
            ),
        ];
        assert_eq!(
            fixture_labels(&hasan),
            [
                "Hasan, Liang, Kahn & Jones-Miller 2018",
                "Hasan, Liang, Kahn & Weintraub 2018"
            ]
        );
        // One list is a prefix of the other.
        let prefix = [
            fixture("p1", &[("X", "A"), ("Y", "B"), ("Z", "C")], "One"),
            fixture(
                "p2",
                &[("X", "A"), ("Y", "B"), ("Z", "C"), ("W", "D")],
                "Two",
            ),
        ];
        assert_eq!(
            fixture_labels(&prefix),
            ["X, Y & Z 2018", "X, Y et al. 2018"]
        );
        // Identical lists get letters in title order, ignoring a leading article; a work with
        // a different list in the same group gets names only.
        let mixed = [
            fixture("m1", &[("G", "A"), ("D", "B"), ("S", "C")], "The zebra"),
            fixture("m2", &[("G", "A"), ("D", "B"), ("S", "C")], "Apples"),
            fixture("m3", &[("G", "A"), ("C", "B"), ("K", "C")], "Middle"),
        ];
        assert_eq!(
            fixture_labels(&mixed),
            ["G, D & S 2018b", "G, D & S 2018a", "G, C & K 2018"]
        );
        // A group author with identical works; a lone work keeps its plain label.
        let group = [
            fixture(
                "g1",
                &[("Los Alamos HIV Sequence Database", "")],
                "Landmarks",
            ),
            fixture(
                "g2",
                &[("Los Alamos HIV Sequence Database", "")],
                "Gene Cutter help",
            ),
            fixture("g3", &[("Shannon", "C")], "A mathematical theory"),
        ];
        assert_eq!(
            fixture_labels(&group),
            [
                "Los Alamos HIV Sequence Database 2018b",
                "Los Alamos HIV Sequence Database 2018a",
                "Shannon 2018",
            ]
        );
        assert_eq!(year_suffix(0), "a");
        assert_eq!(year_suffix(25), "z");
        assert_eq!(year_suffix(26), "aa");
        assert_eq!(year_suffix(27), "ab");
        assert_eq!(year_suffix(26 + 26 * 26), "aaa");
        assert_eq!(title_order("An Example"), "example");
        assert_eq!(title_order("Another"), "another");
    }

    #[test]
    fn initials_handle_compound_and_dotted_names() {
        assert_eq!(initials("Hans-Georg"), "HG");
        assert_eq!(initials("C. Titus"), "CT");
        assert_eq!(initials("Justin J. J."), "JJJ");
        assert_eq!(initials("Eyyüb Selim"), "ES");
        assert_eq!(initials(""), "");
    }

    #[test]
    fn group_authors_and_long_lists() {
        let g = by_key("goecks2010").unwrap();
        assert!(
            g.text()
                .starts_with("Goecks J, Nekrutenko A, Taylor J, Galaxy Team. Galaxy:")
        );
        assert!(g.bibtex().contains("{Taylor}, James and {Galaxy Team}"));
        let b = by_key("bolyen2019").unwrap();
        assert!(b.authors.len() > TEXT_MAX_AUTHORS);
        assert!(b.text().contains(", et al. Reproducible"));
        let bib = b.bibtex();
        let author_line = bib
            .lines()
            .find(|l| l.trim_start().starts_with("author"))
            .unwrap();
        assert_eq!(author_line.matches(" and ").count(), b.authors.len() - 1);
    }

    #[test]
    fn bibtex_is_balanced_and_escaped() {
        for c in all() {
            let b = c.bibtex();
            let mut depth = 0i32;
            let mut prev = '\0';
            for ch in b.chars() {
                if prev != '\\' {
                    match ch {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                }
                assert!(depth >= 0, "{}", c.key);
                prev = ch;
            }
            assert_eq!(depth, 0, "{}", c.key);
            assert!(b.starts_with('@') && b.ends_with('}'), "{}", c.key);
        }
        let m = by_key("maceachren2012").unwrap().bibtex();
        assert!(m.contains("Visual Semiotics \\& Uncertainty"));
        assert!(
            by_key("brown2001")
                .unwrap()
                .bibtex()
                .contains("pages     = {101--133}")
        );
    }

    #[test]
    fn ris_records() {
        let r = dima_paper().ris();
        assert!(r.starts_with("TY  - JOUR\nAU  - Tharanga, Shan\n"));
        assert!(r.contains("SP  - bbae607\n"));
        assert!(!r.contains("EP  -"));
        assert!(r.ends_with("ER  - \n"));
        let r = by_key("brown2001").unwrap().ris();
        assert!(r.contains("SP  - 101\nEP  - 133\n"));
    }

    #[test]
    fn cff_document_shape() {
        let doc = cff_document(SOFTWARE_VERSION, &[by_key("chao2013").unwrap()]);
        assert!(doc.contains("cff-version: 1.2.0\n"));
        assert!(doc.contains(&format!("version: \"{SOFTWARE_VERSION}\"\n")));
        assert!(doc.contains("preferred-citation:\n  type: article\n  authors:\n    - family-names: \"Tharanga\"\n      given-names: \"Shan\"\n"));
        assert!(doc.contains("  doi: \"10.1093/bib/bbae607\"\n"));
        assert!(doc.contains("references:\n  - type: article\n    authors:\n"));
        // Every line is either a comment, a top-level key, or indented by an even number of spaces.
        for line in doc.lines() {
            let indent = line.len() - line.trim_start().len();
            assert_eq!(indent % 2, 0, "{line}");
        }
    }

    #[test]
    fn software_citations() {
        let t = software_text("0.9.1");
        assert!(t.starts_with("Tharanga S. dima: sequence diversity dynamics analyser"));
        assert!(t.contains(", version 0.9.1 [software]. "));
        let b = software_bibtex("0.9.1");
        assert!(b.starts_with("@software{dima_rs_0_9_1,\n"));
        assert!(b.contains("version   = {0.9.1}"));
        let r = software_ris("0.9.1");
        assert!(r.contains("ET  - 0.9.1\n") && r.ends_with("ER  - \n"));
        let cff = cff_document("0.9.1", &[]);
        assert!(cff.contains("version: \"0.9.1\"\n"));
        assert!(cff.contains("url: \"https://shanweera.github.io/dima-perf/v0.9/\"\n"));
        assert!(
            DOCS_URL.ends_with("/latest/") || DOCS_URL == release_docs_url(SOFTWARE_VERSION),
            "{DOCS_URL}"
        );
        assert!(env!("CARGO_PKG_AUTHORS").starts_with("Shan Tharanga"));
    }
}
