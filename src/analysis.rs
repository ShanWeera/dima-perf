use hashbrown::HashMap;
use rayon::prelude::*;
use statrs::statistics::Statistics;

use crate::io::{get_kmers_and_headers, estimate_msa_dimensions};
use crate::entropy::calculate_entropy;
use crate::kmer::count_kmers;
use crate::models::{Results, Position, Variant, highest_entropy};

pub fn get_results_objs(
    path: String,
    kmer_length: usize,
    support_threshold: usize,
    query_name: String,
    header_format: Option<Vec<String>>,
    alphabet: Option<String>,
    header_fillna: Option<String>,
    metadata_fields: Option<Vec<String>>, // new
    summary_only: bool,                   // new
) -> Results {
    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let early_pb = if show_progress {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_message("Preparing analysis...");
        pb.enable_steady_tick(std::time::Duration::from_millis(1));
        Some(pb)
    } else { None };

    // Heuristic memory check
    let (seq_count, seq_len) = estimate_msa_dimensions(&path).unwrap_or((0, 0));
    let positions = if seq_len >= kmer_length { seq_len - kmer_length + 1 } else { 0 };
    let avg_kmer_bytes = kmer_length; // lower bound; UTF-8 bytes == chars here
    let estimated_bytes: u128 = (positions as u128)
        .saturating_mul(seq_count as u128)
        .saturating_mul(avg_kmer_bytes as u128);

    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let avail_bytes = sys.available_memory() as u128; // in bytes

    let mut force_summary = summary_only;
    if !summary_only && estimated_bytes > (avail_bytes / 4) {
        force_summary = true;
    }

    if let Some(pb) = &early_pb { pb.set_message("Reading FASTA and building k-mer matrix..."); }

    let (kmers, headers, sequence_count) = get_kmers_and_headers(
        &path,
        &kmer_length,
        header_format.as_ref(),
        header_fillna.as_ref(),
        alphabet.as_ref(),
        if seq_count > 0 { Some(seq_count) } else { None },
    );

    if let Some(pb) = early_pb { pb.finish_and_clear(); }

    let position_slices = kmers
        .into_par_iter()
        .map(|position_kmers| position_kmers
            .into_iter()
            .map(|item| item.into_boxed_str())
            .collect::<Vec<Box<str>>>())
        .collect::<Vec<Vec<Box<str>>>>();

    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");

    if show_progress && force_summary && !summary_only {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_message("Low memory detected. Switching to summary-only mode.");
        pb.finish_and_clear();
    }

    let position_entropies: Vec<f64> = if show_progress {
        let pb = indicatif::ProgressBar::new(position_slices.len() as u64);
        pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} Entropy")
            .unwrap()
            .progress_chars("##-"));
        let ent = position_slices
            .par_iter()
            .map(|position_kmers| {
                let v = calculate_entropy(position_kmers, &support_threshold);
                pb.inc(1);
                v
            })
            .collect();
        pb.finish_and_clear();
        ent
    } else {
        position_slices
            .par_iter()
            .map(|position_kmers| calculate_entropy(position_kmers, &support_threshold))
            .collect()
    };

    let positions: Vec<Position> = if show_progress {
        let pb = indicatif::ProgressBar::new(position_slices.len() as u64);
        pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} Positions")
            .unwrap()
            .progress_chars("##-"));
        let pos = position_slices
            .par_iter()
            .map(|position_kmers| count_kmers(position_kmers))
            .enumerate()
            .map(|(idx, position_count)| {
                let support = position_slices[idx].len();
                let out = if force_summary {
                    Position::new(
                        idx + 1,
                        position_entropies[idx],
                        support,
                        None,
                        if support == 0 {
                            Some("NS".to_owned())
                        } else if support < support_threshold {
                            Some("LS".to_owned())
                        } else if support == support_threshold {
                            Some("ELS".to_owned())
                        } else {
                            None
                        },
                    )
                } else {
                    let mut variants = position_count
                        .iter()
                        .map(|(sequence, count_data)| {
                            let mut variant = Variant {
                                sequence: sequence.to_string(),
                                count: count_data.0,
                                incidence: ((count_data.0 as f32 / position_slices[idx].len() as f32) * 100_f32),
                                metadata: None,
                                motif_short: None,
                                motif_long: None,
                            };

                            if let (Some(header_components), Some(headers)) = (&header_format, &headers) {
                                let fields: Vec<&String> = match &metadata_fields {
                                    Some(only) => header_components.iter().filter(|f| only.contains(f)).collect(),
                                    None => header_components.iter().collect(),
                                };

                                if !fields.is_empty() {
                                    let mut metadata: HashMap<String, HashMap<String, usize>> = HashMap::new();
                                    count_data.1.iter().for_each(|idx| {
                                        fields.iter().for_each(|item| {
                                            let metadata_entry_hashmap =
                                                metadata.entry((**item).to_string()).or_insert(HashMap::new());

                                            let metadata_entry = headers[*idx]
                                                .as_ref()
                                                .unwrap()
                                                .get(*item)
                                                .unwrap()
                                                .to_owned();

                                            metadata_entry_hashmap
                                                .entry(metadata_entry)
                                                .and_modify(|count| *count += 1)
                                                .or_insert(1);
                                        });
                                    });
                                    variant.metadata = Some(metadata);
                                }
                            }

                            variant
                        })
                        .collect::<Vec<Variant>>();

                    Position::new(
                        idx + 1,
                        position_entropies[idx],
                        support,
                        if variants.is_empty() { None } else { Some(&mut variants) },
                        if support == 0 {
                            Some("NS".to_owned())
                        } else if support < support_threshold {
                            Some("LS".to_owned())
                        } else if support == support_threshold {
                            Some("ELS".to_owned())
                        } else {
                            None
                        },
                    )
                };
                pb.inc(1);
                out
            })
            .collect();
        pb.finish_and_clear();
        pos
    } else {
        position_slices
            .par_iter()
            .map(|position_kmers| count_kmers(position_kmers))
            .enumerate()
            .map(|(idx, position_count)| {
                let support = position_slices[idx].len();
                if force_summary {
                    return Position::new(
                        idx + 1,
                        position_entropies[idx],
                        support,
                        None,
                        if support == 0 {
                            Some("NS".to_owned())
                        } else if support < support_threshold {
                            Some("LS".to_owned())
                        } else if support == support_threshold {
                            Some("ELS".to_owned())
                        } else {
                            None
                        },
                    );
                }
                let mut variants = position_count
                    .iter()
                    .map(|(sequence, count_data)| {
                        let mut variant = Variant {
                            sequence: sequence.to_string(),
                            count: count_data.0,
                            incidence: ((count_data.0 as f32 / position_slices[idx].len() as f32) * 100_f32),
                            metadata: None,
                            motif_short: None,
                            motif_long: None,
                        };

                        if let (Some(header_components), Some(headers)) = (&header_format, &headers) {
                            let fields: Vec<&String> = match &metadata_fields {
                                Some(only) => header_components.iter().filter(|f| only.contains(f)).collect(),
                                None => header_components.iter().collect(),
                            };

                            if !fields.is_empty() {
                                let mut metadata: HashMap<String, HashMap<String, usize>> = HashMap::new();
                                count_data.1.iter().for_each(|idx| {
                                    fields.iter().for_each(|item| {
                                        let metadata_entry_hashmap =
                                            metadata.entry((**item).to_string()).or_insert(HashMap::new());

                                        let metadata_entry = headers[*idx]
                                            .as_ref()
                                            .unwrap()
                                            .get(*item)
                                            .unwrap()
                                            .to_owned();

                                        metadata_entry_hashmap
                                            .entry(metadata_entry)
                                            .and_modify(|count| *count += 1)
                                            .or_insert(1);
                                    });
                                });
                                variant.metadata = Some(metadata);
                            }
                        }

                        variant
                    })
                    .collect::<Vec<Variant>>();

                Position::new(
                    idx + 1,
                    position_entropies[idx],
                    support,
                    if variants.is_empty() { None } else { Some(&mut variants) },
                    if support == 0 {
                        Some("NS".to_owned())
                    } else if support < support_threshold {
                        Some("LS".to_owned())
                    } else if support == support_threshold {
                        Some("ELS".to_owned())
                    } else {
                        None
                    },
                )
            })
            .collect()
    };

    Results {
        support_threshold,
        kmer_length,
        sequence_count,
        highest_entropy: highest_entropy(position_entropies.as_slice()),
        average_entropy: position_entropies.mean(),
        low_support_count: positions
            .iter()
            .filter(|position| position.low_support.is_some())
            .count(),
        query_name,
        results: positions,
    }
} 