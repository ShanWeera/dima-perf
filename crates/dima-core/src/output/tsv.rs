//! Tidy tab-separated tables (D22, D50): one observation per row, one variable per column
//! (Wickham 2014; Broman & Woo 2018), a header row, RFC 4180 minimal quoting with a tab
//! delimiter, `NA` for undefined values, and floating-point numbers in their shortest
//! round-trip form.

use std::io::{self, Write};

use crate::model::{HcsResults, Results};

/// The tables DiMA writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    /// One row per position.
    Positions,
    /// One row per (position, variant).
    Variants,
    /// One row per (position, variant, field, value).
    Metadata,
    /// One row per conserved region.
    Hcs,
}

impl Table {
    /// Every table, in file order.
    pub const ALL: [Self; 4] = [Self::Positions, Self::Variants, Self::Metadata, Self::Hcs];

    /// Name used in file names (`<base>.<name>.tsv`) and on the command line.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Positions => "positions",
            Self::Variants => "variants",
            Self::Metadata => "metadata",
            Self::Hcs => "hcs",
        }
    }

    /// Column names.
    #[must_use]
    pub const fn columns(self) -> &'static [&'static str] {
        match self {
            Self::Positions => &[
                "position",
                "low_support",
                "support",
                "distinct_variants_count",
                "entropy",
                "observed_entropy",
                "entropy_method",
                "entropy_fallback",
                "distinct_variants_incidence",
                "total_variants_incidence",
                "sample_coverage",
                "below_min_coverage",
            ],
            Self::Variants => &[
                "position",
                "sequence",
                "count",
                "incidence",
                "motif_short",
                "motif_long",
            ],
            Self::Metadata => &["position", "sequence", "field", "value", "count"],
            Self::Hcs => &[
                "region_id",
                "start_column",
                "end_column",
                "length",
                "min_incidence",
                "min_support",
                "conservation_lower_bound_95",
                "flagged",
                "low_support_position_count",
                "sequence",
            ],
        }
    }
}

/// Missing-value marker.
pub const NA: &str = "NA";

struct Row<'a> {
    w: csv::Writer<&'a mut dyn Write>,
    float: ryu::Buffer,
    int: String,
}

impl Row<'_> {
    fn str(&mut self, s: &str) -> io::Result<()> {
        self.w.write_field(s).map_err(csv_error)
    }

    fn int(&mut self, v: u64) -> io::Result<()> {
        use std::fmt::Write as _;
        self.int.clear();
        let _ = write!(self.int, "{v}");
        self.w.write_field(self.int.as_bytes()).map_err(csv_error)
    }

    fn float(&mut self, v: Option<f64>) -> io::Result<()> {
        match v {
            Some(x) if x.is_finite() => {
                let s = self.float.format_finite(x);
                self.w.write_field(s).map_err(csv_error)
            }
            _ => self.str(NA),
        }
    }

    fn opt_str(&mut self, v: Option<&str>) -> io::Result<()> {
        self.str(v.unwrap_or(NA))
    }

    fn opt_bool(&mut self, v: Option<bool>) -> io::Result<()> {
        self.str(match v {
            Some(true) => "true",
            Some(false) => "false",
            None => NA,
        })
    }

    fn end(&mut self) -> io::Result<()> {
        self.w.write_record(None::<&[u8]>).map_err(csv_error)
    }
}

fn csv_error(e: csv::Error) -> io::Error {
    match e.into_kind() {
        csv::ErrorKind::Io(e) => e,
        other => io::Error::other(format!("{other:?}")),
    }
}

/// Writes `table` of `results` (and `hcs` for the HCS table) to `out`.
///
/// # Errors
/// I/O errors of `out`; [`io::ErrorKind::InvalidInput`] when the HCS table is requested
/// without HCS results.
pub fn write_table(
    results: &Results,
    hcs: Option<&HcsResults>,
    table: Table,
    out: &mut dyn Write,
) -> io::Result<()> {
    write_any(Some(results), hcs, table, out)
}

/// Writes the HCS table from conserved regions alone.
///
/// # Errors
/// I/O errors of `out`.
pub fn write_table_hcs(hcs: &HcsResults, out: &mut dyn Write) -> io::Result<()> {
    write_any(None, Some(hcs), Table::Hcs, out)
}

fn write_any(
    results: Option<&Results>,
    hcs: Option<&HcsResults>,
    table: Table,
    out: &mut dyn Write,
) -> io::Result<()> {
    let positions: &[crate::model::PositionResult] = results.map_or(&[], |r| &r.results);
    let w = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .quote_style(csv::QuoteStyle::Necessary)
        .terminator(csv::Terminator::Any(b'\n'))
        .has_headers(false)
        .flexible(true)
        .from_writer(out);
    let mut row = Row {
        w,
        float: ryu::Buffer::new(),
        int: String::new(),
    };
    for c in table.columns() {
        row.str(c)?;
    }
    row.end()?;
    match table {
        Table::Positions => {
            for p in positions {
                row.int(p.position)?;
                row.opt_str(p.low_support.map(|l| l.as_str()))?;
                row.int(p.support)?;
                row.int(p.distinct_variants_count)?;
                row.float(p.entropy)?;
                row.float(p.observed_entropy)?;
                row.str(p.entropy_method.as_str())?;
                row.opt_str(p.entropy_fallback.map(|f| f.as_str()))?;
                row.float(p.distinct_variants_incidence)?;
                row.float(p.total_variants_incidence)?;
                row.float(p.sample_coverage)?;
                row.opt_bool(p.below_min_coverage)?;
                row.end()?;
            }
        }
        Table::Variants => {
            for p in positions {
                for v in &p.diversity_motifs {
                    row.int(p.position)?;
                    row.str(&v.sequence)?;
                    row.int(v.count)?;
                    row.float(Some(v.incidence))?;
                    row.str(v.motif.short())?;
                    row.str(v.motif.long())?;
                    row.end()?;
                }
            }
        }
        Table::Metadata => {
            for p in positions {
                for v in &p.diversity_motifs {
                    for f in v.metadata.iter().flat_map(|m| &m.0) {
                        for (value, n) in &f.values {
                            row.int(p.position)?;
                            row.str(&v.sequence)?;
                            row.str(&f.field)?;
                            row.str(value)?;
                            row.int(*n)?;
                            row.end()?;
                        }
                    }
                }
            }
        }
        Table::Hcs => {
            let hcs = hcs.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the HCS table needs HCS results",
                )
            })?;
            for g in &hcs.regions {
                row.int(g.id)?;
                row.int(g.start_column)?;
                row.int(g.end_column)?;
                row.int(g.length)?;
                row.float(Some(g.min_incidence))?;
                row.int(g.min_support)?;
                row.float(Some(g.conservation_lower_bound_95))?;
                row.opt_bool(Some(g.flagged))?;
                row.int(g.low_support_positions.len() as u64)?;
                row.str(&g.sequence)?;
                row.end()?;
            }
        }
    }
    row.w.flush()
}
