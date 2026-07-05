//! Output format handling for DiMA analysis results.
//!
//! Supports JSON (pretty/compact), TSV (17-column vDiveR-aligned), JSONL (one position
//! per line), and binary .dima format. Follows samtools/bcftools convention for format
//! resolution: explicit -O flag > output file extension inference > json fallback.

use std::io::{self, IsTerminal, Write};
use std::path::Path;

use clap::ValueEnum;

use crate::models::{Results, Position, Variant};

/// Supported output formats, following BCFtools -O convention.
#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum OutputType {
    /// Pretty-printed or compact JSON (auto-detect from terminal)
    Json,
    /// Tab-separated values with 17 columns (vDiveR-aligned)
    Tsv,
    /// Newline-delimited JSON (one Position object per line)
    Jsonl,
    /// Binary .dima format (compact, compressed)
    Dima,
}

/// Options controlling output behavior (independent of format selection).
pub struct OutputOptions {
    /// Whether JSON output should be pretty-printed (indented)
    pub pretty: bool,
    /// Whether TSV output includes a header row
    pub include_header: bool,
}

/// Resolve output format following samtools convention:
/// explicit -O flag > output file extension > json fallback.
///
/// This is a pure function for easy unit testing.
pub fn resolve_output_type(
    explicit: Option<OutputType>,
    output_path: Option<&Path>,
) -> OutputType {
    if let Some(t) = explicit {
        return t;
    }
    output_path
        .and_then(|p| p.extension())
        .and_then(|ext| match ext.to_str()? {
            "dima" => Some(OutputType::Dima),
            "tsv" | "tab" => Some(OutputType::Tsv),
            "jsonl" | "ndjson" => Some(OutputType::Jsonl),
            _ => None,
        })
        .unwrap_or(OutputType::Json)
}

/// High-level output dispatcher: resolves destination (file vs stdout),
/// configures options, and delegates to format-specific writers.
///
/// Handles file output with atomic write, and stdout with terminal-aware formatting.
/// Binary .dima output is NOT handled here (uses Results::to_binary directly).
pub fn write_results_to_output(
    results: &Results,
    output_path: Option<&Path>,
    output_type: OutputType,
    no_header: bool,
) -> anyhow::Result<()> {
    match output_path {
        Some(path) => {
            let options = OutputOptions {
                pretty: true,
                include_header: !no_header,
            };
            crate::io::atomic_write(path, |writer| {
                write_results(results, writer, output_type, &options)
            })?;
        }
        None => {
            let stdout = io::stdout();
            let mut writer = io::BufWriter::new(stdout.lock());
            let options = OutputOptions {
                pretty: io::stdout().is_terminal(),
                include_header: !no_header,
            };
            write_results(results, &mut writer, output_type, &options)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

/// Format-specific writer dispatcher.
fn write_results(
    results: &Results,
    writer: &mut dyn Write,
    output_type: OutputType,
    options: &OutputOptions,
) -> io::Result<()> {
    match output_type {
        OutputType::Json => write_json(results, writer, options),
        OutputType::Tsv => write_tsv(results, writer, options),
        OutputType::Jsonl => write_jsonl(results, writer),
        OutputType::Dima => {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Binary .dima output must use Results::to_binary directly",
            ))
        }
    }
}

/// Write results as JSON (pretty or compact).
fn write_json(results: &Results, writer: &mut dyn Write, options: &OutputOptions) -> io::Result<()> {
    if options.pretty {
        serde_json::to_writer_pretty(writer, results).map_err(io::Error::other)
    } else {
        serde_json::to_writer(writer, results).map_err(io::Error::other)
    }
}

/// Write results as JSONL (one Position object per line, no envelope).
fn write_jsonl(results: &Results, writer: &mut dyn Write) -> io::Result<()> {
    for position in &results.results {
        serde_json::to_writer(&mut *writer, position).map_err(io::Error::other)?;
        writeln!(writer)?;
    }
    Ok(())
}

/// TSV header row (17 columns, vDiveR-aligned).
const TSV_HEADER: &[&str] = &[
    "position",
    "entropy",
    "support",
    "low_support",
    "distinct_variants_count",
    "distinct_variants_incidence",
    "total_variants_incidence",
    "index_sequence",
    "index_count",
    "index_incidence",
    "major_sequence",
    "major_count",
    "major_incidence",
    "minor_count",
    "minor_incidence",
    "unique_count",
    "unique_incidence",
];

/// Write results as tab-separated values (17 columns per row).
///
/// Multi-value handling for tied motifs: comma-separated within cells (VCF v4.5 convention).
/// Missing values: '.' (dot) — standard in VCF/BED genomic formats.
fn write_tsv(results: &Results, writer: &mut dyn Write, options: &OutputOptions) -> io::Result<()> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_writer(writer);

    if options.include_header {
        wtr.write_record(TSV_HEADER)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }

    for position in &results.results {
        let row = build_tsv_row(position);
        wtr.write_record(&row)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }

    wtr.flush().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(())
}

/// Build a 17-element TSV row from a Position, extracting motif-specific columns.
///
/// Motif classification per DiMA publication:
/// - Index ("I"): highest-frequency variant(s)
/// - Major ("Ma"): second highest frequency
/// - Minor ("Mi"): third-to-second-last frequency
/// - Unique ("U"): single-occurrence variants
fn build_tsv_row(position: &Position) -> Vec<String> {
    let mut row = Vec::with_capacity(17);

    // Columns 1-7: position-level fields
    row.push(position.position.to_string());
    row.push(format!("{:.6}", position.entropy));
    row.push(position.support.to_string());
    row.push(position.low_support.as_deref().unwrap_or(".").to_string());
    row.push(position.distinct_variants_count.to_string());
    row.push(format!("{:.2}", position.distinct_variants_incidence));
    row.push(format!("{:.2}", position.total_variants_incidence));

    // Columns 8-17: motif-specific fields
    let (index, major, minor, unique) = extract_motif_groups(position);

    // Index: sequence, count, incidence (comma-separated for ties)
    push_motif_sequence_fields(&mut row, &index);

    // Major: sequence, count, incidence (comma-separated for ties)
    push_motif_sequence_fields(&mut row, &major);

    // Minor: aggregated count and incidence (no sequence — may be many)
    push_aggregated_count_incidence(&mut row, &minor);

    // Unique: aggregated count and incidence
    push_aggregated_count_incidence(&mut row, &unique);

    row
}

/// Extract variants grouped by motif classification.
fn extract_motif_groups(position: &Position) -> (Vec<&Variant>, Vec<&Variant>, Vec<&Variant>, Vec<&Variant>) {
    let mut index = Vec::new();
    let mut major = Vec::new();
    let mut minor = Vec::new();
    let mut unique = Vec::new();

    if let Some(ref motifs) = position.diversity_motifs {
        for variant in motifs {
            match variant.motif_short.as_deref() {
                Some("I") => index.push(variant),
                Some("Ma") => major.push(variant),
                Some("Mi") => minor.push(variant),
                Some("U") => unique.push(variant),
                _ => {}
            }
        }
    }

    (index, major, minor, unique)
}

/// Push sequence, count, and incidence fields for a motif group.
/// Multiple tied variants are comma-separated (VCF convention).
fn push_motif_sequence_fields(row: &mut Vec<String>, variants: &[&Variant]) {
    if variants.is_empty() {
        row.push(".".to_string());
        row.push(".".to_string());
        row.push(".".to_string());
    } else {
        let seqs: Vec<&str> = variants.iter().map(|v| v.sequence.as_str()).collect();
        let counts: Vec<String> = variants.iter().map(|v| v.count.to_string()).collect();
        let incidences: Vec<String> = variants.iter().map(|v| format!("{:.2}", v.incidence)).collect();
        row.push(seqs.join(","));
        row.push(counts.join(","));
        row.push(incidences.join(","));
    }
}

/// Push aggregated count and incidence for minor/unique groups.
fn push_aggregated_count_incidence(row: &mut Vec<String>, variants: &[&Variant]) {
    if variants.is_empty() {
        row.push(".".to_string());
        row.push(".".to_string());
    } else {
        let total_count: usize = variants.iter().map(|v| v.count).sum();
        let total_incidence: f64 = variants.iter().map(|v| v.incidence).sum();
        row.push(total_count.to_string());
        row.push(format!("{:.2}", total_incidence));
    }
}

/// Serializable Position for JSONL output (uses the same structure as JSON).
impl Position {
    // Position already derives Serialize via serde, so JSONL just serializes each one.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_resolve_output_type_explicit_wins() {
        assert_eq!(
            resolve_output_type(Some(OutputType::Tsv), Some(Path::new("results.json"))),
            OutputType::Tsv
        );
    }

    #[test]
    fn test_resolve_output_type_extension_inference() {
        assert_eq!(resolve_output_type(None, Some(Path::new("out.tsv"))), OutputType::Tsv);
        assert_eq!(resolve_output_type(None, Some(Path::new("out.tab"))), OutputType::Tsv);
        assert_eq!(resolve_output_type(None, Some(Path::new("out.jsonl"))), OutputType::Jsonl);
        assert_eq!(resolve_output_type(None, Some(Path::new("out.ndjson"))), OutputType::Jsonl);
        assert_eq!(resolve_output_type(None, Some(Path::new("out.dima"))), OutputType::Dima);
    }

    #[test]
    fn test_resolve_output_type_unknown_extension_defaults_json() {
        assert_eq!(resolve_output_type(None, Some(Path::new("out.json"))), OutputType::Json);
        assert_eq!(resolve_output_type(None, Some(Path::new("out.txt"))), OutputType::Json);
        assert_eq!(resolve_output_type(None, Some(Path::new("results"))), OutputType::Json);
    }

    #[test]
    fn test_resolve_output_type_no_path_defaults_json() {
        assert_eq!(resolve_output_type(None, None), OutputType::Json);
    }
}
