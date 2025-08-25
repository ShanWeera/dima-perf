use bio::io::fasta;
use hashbrown::HashMap;
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, Write};

use crate::kmer::{sliding_window, sliding_window_encoded};

pub fn save_file(content: &str, path: &str) -> Result<(), io::Error> {
    if let Ok(mut f) = File::create(path) {
        if f.write_all(content.as_bytes()).is_ok() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Cannot write to file."))
        }
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, "Unable to create on disk."))
    }
}

pub fn estimate_msa_dimensions(path: &String) -> io::Result<(usize, usize)> {
    // Returns (sequence_count, sequence_length)
    let mut count = 0usize;
    let mut length = 0usize;
    for rec in fasta::Reader::new(File::open(path)?).records() {
        let rec = rec?;
        if length == 0 { length = rec.seq().len(); }
        count += 1;
    }
    Ok((count, length))
}

pub fn parse_header(
    header: &String,
    format: &Vec<String>,
    fill_na: &String,
) -> HashMap<String, String> {
    let metadata = header
        .split("|")
        .map(|component| {
            return if !component.is_empty() {
                component.trim()
            } else {
                if !fill_na.is_empty() {
                    return fill_na.as_str();
                }
                component
            };
        })
        .collect::<Vec<&str>>();

    assert_eq!(
        metadata.iter().filter(|item| item.len() == 0).count(),
        0,
        "\n\nThe FASTA header looks invalid:\n\tFormat: {}\n\tHeader: {}\n\n",
        format.join("|"),
        header
    );

    assert_eq!(
        metadata.len(),
        format.len(),
        "\n\nThe header format provided does not match the header:\n\tFormat: {}\n\tHeader: {}\n\n",
        format.join("|"),
        header
    );

    format
        .iter()
        .enumerate()
        .map(|(idx, item)| (item.to_string(), metadata[idx].to_owned()))
        .collect::<HashMap<String, String>>()
}

pub fn get_kmers_and_headers(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<String>>,                      // transposed kmers
    Option<Vec<Option<HashMap<String, String>>>>, // headers
    usize,                                 // sequence count
) {
    let protein_ambiguous_chars = vec!['-', 'X', 'B', 'J', 'Z', 'O', 'U'];
    let nucleotide_ambiguous_chars = vec!['-', 'R', 'Y', 'K', 'M', 'S', 'W', 'B', 'D', 'H', 'V', 'N'];

    let illegal_chars = if let Some(residue_alphabet) = alphabet {
        if residue_alphabet == "protein" { protein_ambiguous_chars } else { nucleotide_ambiguous_chars }
    } else { protein_ambiguous_chars };

    let mut transposed_kmers: Vec<Vec<String>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let pb = if show_progress {
        match expected_count {
            Some(len) => {
                let pb = indicatif::ProgressBar::new(len as u64);
                pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA")
                    .unwrap()
                    .progress_chars("##-"));
                Some(pb)
            }
            None => {
                let pb = indicatif::ProgressBar::new_spinner();
                pb.set_message("Reading FASTA...");
                pb.enable_steady_tick(std::time::Duration::from_millis(1));
                Some(pb)
            }
        }
    } else { None };

    for record in fasta::Reader::new(File::open(path).expect("Failed to read FASTA file")).records() {
        let record_unwrapped = record.as_ref().unwrap();
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        let kmers = sliding_window(
            &String::from_utf8(Vec::from(record_unwrapped.seq())).unwrap(),
            &kmer_length,
            &illegal_chars,
        );

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); kmers.len()];
        }

        for (i, k) in kmers.into_iter().enumerate() {
            transposed_kmers[i].push(k);
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record_unwrapped.desc() {
                [record_unwrapped.id(), desc].join(" ")
            } else {
                record_unwrapped.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    if let Some(pb) = pb { pb.finish_and_clear(); }

    transposed_kmers
        .par_iter_mut()
        .for_each(|kmer_position| kmer_position.retain(|kmer| kmer != "NA"));

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    (transposed_kmers, headers, sequence_count)
}

pub fn get_kmers_and_headers_encoded(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<u64>>,                        // transposed encoded kmers
    Option<Vec<Option<HashMap<String, String>>>>, // headers
    usize,                                // sequence count
    bool,                                 // is_protein flag
) {
    let protein_ambiguous_chars = vec![b'-', b'X', b'B', b'J', b'Z', b'O', b'U'];
    let nucleotide_ambiguous_chars = vec![b'-', b'R', b'Y', b'K', b'M', b'S', b'W', b'B', b'D', b'H', b'V', b'N'];

    let is_protein = if let Some(residue_alphabet) = alphabet {
        residue_alphabet == "protein"
    } else {
        true // default to protein
    };

    let illegal_chars = if is_protein { protein_ambiguous_chars } else { nucleotide_ambiguous_chars };

    let mut transposed_kmers: Vec<Vec<u64>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let pb = if show_progress {
        match expected_count {
            Some(len) => {
                let pb = indicatif::ProgressBar::new(len as u64);
                pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA")
                    .unwrap()
                    .progress_chars("##-"));
                Some(pb)
            }
            None => {
                let pb = indicatif::ProgressBar::new_spinner();
                pb.set_message("Reading FASTA...");
                pb.enable_steady_tick(std::time::Duration::from_millis(1));
                Some(pb)
            }
        }
    } else { None };

    for record in fasta::Reader::new(File::open(path).expect("Failed to read FASTA file")).records() {
        let record_unwrapped = record.as_ref().unwrap();
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        let sequence_bytes = record_unwrapped.seq();
        let encoded_kmers = sliding_window_encoded(
            sequence_bytes,
            *kmer_length,
            is_protein,
            &illegal_chars,
        );

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); encoded_kmers.len()];
        }

        for (i, encoded_kmer) in encoded_kmers.into_iter().enumerate() {
            if let Some(kmer) = encoded_kmer {
                transposed_kmers[i].push(kmer);
            }
            // Skip None values (invalid k-mers)
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record_unwrapped.desc() {
                [record_unwrapped.id(), desc].join(" ")
            } else {
                record_unwrapped.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    if let Some(pb) = pb { pb.finish_and_clear(); }

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    (transposed_kmers, headers, sequence_count, is_protein)
} 