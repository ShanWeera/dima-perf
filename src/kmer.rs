use hashbrown::HashMap;

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