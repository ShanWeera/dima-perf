use hashbrown::HashMap;

// Encoding tables for amino acids/nucleotides
const PROTEIN_ENCODING: &[u8; 256] = &{
    let mut table = [255u8; 256]; // 255 = invalid
    table[b'A' as usize] = 0;
    table[b'C' as usize] = 1;
    table[b'D' as usize] = 2;
    table[b'E' as usize] = 3;
    table[b'F' as usize] = 4;
    table[b'G' as usize] = 5;
    table[b'H' as usize] = 6;
    table[b'I' as usize] = 7;
    table[b'K' as usize] = 8;
    table[b'L' as usize] = 9;
    table[b'M' as usize] = 10;
    table[b'N' as usize] = 11;
    table[b'P' as usize] = 12;
    table[b'Q' as usize] = 13;
    table[b'R' as usize] = 14;
    table[b'S' as usize] = 15;
    table[b'T' as usize] = 16;
    table[b'V' as usize] = 17;
    table[b'W' as usize] = 18;
    table[b'Y' as usize] = 19;
    table
};

const NUCLEOTIDE_ENCODING: &[u8; 256] = &{
    let mut table = [255u8; 256];
    table[b'A' as usize] = 0;
    table[b'C' as usize] = 1;
    table[b'G' as usize] = 2;
    table[b'T' as usize] = 3;
    table
};

// Decoding tables for converting back to strings
const PROTEIN_CHARS: &[u8; 20] = b"ACDEFGHIKLMNPQRSTVWY";
const NUCLEOTIDE_CHARS: &[u8; 4] = b"ACGT";

pub fn encode_kmer(kmer: &[u8], is_protein: bool) -> Option<u64> {
    let encoding_table = if is_protein { PROTEIN_ENCODING } else { NUCLEOTIDE_ENCODING };
    let base = if is_protein { 20u64 } else { 4u64 };
    
    let mut encoded = 0u64;
    for &byte in kmer {
        let code = encoding_table[byte as usize];
        if code == 255 { return None; } // Invalid character
        encoded = encoded * base + code as u64;
    }
    Some(encoded)
}

pub fn decode_kmer(encoded: u64, kmer_length: usize, is_protein: bool) -> String {
    let mut result = Vec::with_capacity(kmer_length);
    let mut remaining = encoded;
    
    if is_protein {
        let base = 20u64;
        for _ in 0..kmer_length {
            let char_idx = (remaining % base) as usize;
            result.push(PROTEIN_CHARS[char_idx]);
            remaining /= base;
        }
    } else {
        let base = 4u64;
        for _ in 0..kmer_length {
            let char_idx = (remaining % base) as usize;
            result.push(NUCLEOTIDE_CHARS[char_idx]);
            remaining /= base;
        }
    }
    
    result.reverse();
    String::from_utf8(result).unwrap()
}

pub fn sliding_window_encoded(
    sequence: &[u8],
    kmer_length: usize,
    is_protein: bool,
    illegal_chars: &[u8],
) -> Vec<Option<u64>> {
    sequence
        .windows(kmer_length)
        .map(|window| {
            // Check for illegal characters first
            if window.iter().any(|&b| illegal_chars.contains(&b)) {
                None
            } else {
                encode_kmer(window, is_protein)
            }
        })
        .collect()
}

pub fn has_overlap_end(prefix: &str, next: &str) -> bool {
    let max_overlap = prefix.len().min(next.len());
    for k in (1..=max_overlap).rev() {
        if &prefix[prefix.len() - k..] == &next[..k] {
            return true;
        }
    }
    false
}

pub fn sliding_window(
    sequence: &String,
    kmer_length: &usize,
    illegal_chars: &Vec<char>,
) -> Vec<String> {
    sequence
        .chars()
        .collect::<Vec<char>>()
        .windows(*kmer_length)
        .map(|kmer_chars| {
            let iter = kmer_chars.into_iter();
            if iter.clone().any(|f| illegal_chars.contains(f)) {
                return String::from("NA");
            }
            iter.collect()
        })
        .collect::<Vec<String>>()
}

pub fn count_kmers<'a>(kmers: &'a [Box<str>]) -> HashMap<&'a str, (usize, Vec<usize>)> {
    let mut counts: HashMap<&'a str, (usize, Vec<usize>)> = HashMap::new();
    kmers
        .iter()
        .enumerate()
        .for_each(|(idx, kmer)| {
            let entry = counts.entry(kmer).or_insert((0, vec![]));
            entry.0 += 1;
            entry.1.push(idx);
        });
    counts
}

pub fn count_kmers_encoded(kmers: &[u64]) -> HashMap<u64, (usize, Vec<usize>)> {
    let mut counts: HashMap<u64, (usize, Vec<usize>)> = HashMap::new();
    kmers
        .iter()
        .enumerate()
        .for_each(|(idx, &kmer)| {
            let entry = counts.entry(kmer).or_insert((0, vec![]));
            entry.0 += 1;
            entry.1.push(idx);
        });
    counts
}

pub fn transpose_kmers(kmers: &Vec<&Vec<String>>) -> Vec<Vec<String>> {
    assert!(!kmers.is_empty());
    let len = kmers[0].len();
    let mut iters: Vec<_> = kmers.into_iter().map(|n| n.into_iter()).collect();
    (0..len)
        .map(|_| {
            iters
                .iter_mut()
                .map(|n| n.next().unwrap().to_owned())
                .collect::<Vec<String>>()
        })
        .collect()
} 