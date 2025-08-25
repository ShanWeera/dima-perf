use hashbrown::HashMap;
use rayon::prelude::*;
use statrs::statistics::Statistics;

use crate::io::get_kmers_and_headers_encoded;
use crate::entropy::calculate_entropy_encoded;
use crate::kmer::{count_kmers_encoded, decode_kmer};
use crate::models::{Results, Position, Variant, highest_entropy};

pub fn get_results_objs(
    path: String,
    kmer_length: usize,
    support_threshold: usize,
    query_name: String,
    header_format: Option<Vec<String>>,
    alphabet: Option<String>,
    header_fillna: Option<String>,
    metadata_fields: Option<Vec<String>>,
) -> Results {
    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let early_pb = if show_progress {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_message("Reading FASTA and building k-mer matrix...");
        pb.enable_steady_tick(std::time::Duration::from_millis(1));
        Some(pb)
    } else { None };

    let (encoded_kmers, headers, sequence_count, is_protein) = get_kmers_and_headers_encoded(
        &path,
        &kmer_length,
        header_format.as_ref(),
        header_fillna.as_ref(),
        alphabet.as_ref(),
        None,
    );

    if let Some(pb) = early_pb { pb.finish_and_clear(); }

    let position_entropies: Vec<f64> = if show_progress {
        let pb = indicatif::ProgressBar::new(encoded_kmers.len() as u64);
        pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} Entropy")
            .unwrap()
            .progress_chars("##-"));
        let ent = encoded_kmers
            .par_iter()
            .map(|position_kmers| {
                let v = calculate_entropy_encoded(position_kmers, &support_threshold);
                pb.inc(1);
                v
            })
            .collect();
        pb.finish_and_clear();
        ent
    } else {
        encoded_kmers
            .par_iter()
            .map(|position_kmers| calculate_entropy_encoded(position_kmers, &support_threshold))
            .collect()
    };

    let positions: Vec<Position> = if show_progress {
        let pb = indicatif::ProgressBar::new(encoded_kmers.len() as u64);
        pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} Positions")
            .unwrap()
            .progress_chars("##-"));
        let pos = encoded_kmers
            .par_iter()
            .map(|position_kmers| count_kmers_encoded(position_kmers))
            .enumerate()
            .map(|(idx, position_count)| {
                let support = encoded_kmers[idx].len();
                let out = {
                    let mut variants = position_count
                        .iter()
                        .map(|(&encoded_sequence, count_data)| {
                            let sequence = decode_kmer(encoded_sequence, kmer_length, is_protein);
                            let mut variant = Variant {
                                sequence,
                                count: count_data.0,
                                incidence: ((count_data.0 as f32 / encoded_kmers[idx].len() as f32) * 100_f32),
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
        encoded_kmers
            .par_iter()
            .map(|position_kmers| count_kmers_encoded(position_kmers))
            .enumerate()
            .map(|(idx, position_count)| {
                let support = encoded_kmers[idx].len();

                let mut variants = position_count
                    .iter()
                    .map(|(&encoded_sequence, count_data)| {
                        let sequence = decode_kmer(encoded_sequence, kmer_length, is_protein);
                        let mut variant = Variant {
                            sequence,
                            count: count_data.0,
                            incidence: ((count_data.0 as f32 / encoded_kmers[idx].len() as f32) * 100_f32),
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