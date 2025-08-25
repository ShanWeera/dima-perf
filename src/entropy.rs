use linreg::linear_regression_of;
use rand::seq::SliceRandom;
use hashbrown::HashMap;

pub fn shannons_entropy(kmers: &[Box<str>]) -> f64 {
    let kmer_count = kmers.len();
    let mut entropy = count_only(kmers)
        .into_iter()
        .map(|(_i, d)| d)
        .map(|count| {
            let p: f64 = count as f64 / kmer_count as f64;
            p * p.log2()
        })
        .sum::<f64>();

    if entropy == 0_f64 { return entropy; }
    entropy *= -1_f64;
    entropy
}

pub fn calculate_entropy(kmers: &[Box<str>], support_threshold: &usize) -> f64 {
    let kmer_count = kmers.len();
    if kmer_count <= 1 { return 0.0_f64; }

    let all_kmers_entropy = shannons_entropy(kmers);
    if &kmer_count < support_threshold { return all_kmers_entropy; }

    let percentage_cutoff = ((*support_threshold as f64 / kmer_count as f64) * 100_f64).ceil() as usize;
    if percentage_cutoff == 100 { return all_kmers_entropy; }

    let starting_point = if percentage_cutoff < 50 { 50 } else { (percentage_cutoff + 4) / 5 * 5 };

    let mut entropy_values: Vec<(f64, f64)> = (starting_point..100)
        .step_by(5)
        .map(|percentage| {
            let samples = (percentage * kmer_count) / 100;
            let entropy = shannons_entropy_sampled(kmers, samples);
            (1.0 / samples as f64, entropy)
        }).collect();

    entropy_values.push((1.0 / kmer_count as f64, all_kmers_entropy));

    if entropy_values.len() >= 5 {
        let (_, y) = linear_regression_of(&entropy_values).unwrap();
        if y < 0_f64 { return all_kmers_entropy; }
        y
    } else {
        all_kmers_entropy
    }
}

fn shannons_entropy_sampled(kmers: &[Box<str>], sample_size: usize) -> f64 {
    // Sample indices and count without allocating strings
    let mut rng = rand::thread_rng();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for _ in 0..sample_size {
        let chosen = kmers.choose(&mut rng).unwrap();
        *counts.entry(chosen).or_insert(0) += 1;
    }
    let mut sum = 0.0f64;
    for (_k, &c) in counts.iter() {
        let p = c as f64 / sample_size as f64;
        sum += p * p.log2();
    }
    if sum == 0.0 { 0.0 } else { -sum }
}

fn count_only<'a>(kmers: &'a [Box<str>]) -> Vec<(String, usize)> {
    let mut counts: HashMap<&'a str, usize> = HashMap::new();
    kmers.iter().for_each(|k| { *counts.entry(k).or_insert(0) += 1; });
    counts.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

// Integer k-mer versions for performance
pub fn shannons_entropy_encoded(kmers: &[u64]) -> f64 {
    let kmer_count = kmers.len();
    let mut entropy = count_only_encoded(kmers)
        .into_iter()
        .map(|(_i, d)| d)
        .map(|count| {
            let p: f64 = count as f64 / kmer_count as f64;
            p * p.log2()
        })
        .sum::<f64>();

    if entropy == 0_f64 { return entropy; }
    entropy *= -1_f64;
    entropy
}

pub fn calculate_entropy_encoded(kmers: &[u64], support_threshold: &usize) -> f64 {
    let kmer_count = kmers.len();
    if kmer_count <= 1 { return 0.0_f64; }

    let all_kmers_entropy = shannons_entropy_encoded(kmers);
    if &kmer_count < support_threshold { return all_kmers_entropy; }

    let percentage_cutoff = ((*support_threshold as f64 / kmer_count as f64) * 100_f64).ceil() as usize;
    if percentage_cutoff == 100 { return all_kmers_entropy; }

    let starting_point = if percentage_cutoff < 50 { 50 } else { (percentage_cutoff + 4) / 5 * 5 };

    let mut entropy_values: Vec<(f64, f64)> = (starting_point..100)
        .step_by(5)
        .map(|percentage| {
            let samples = (percentage * kmer_count) / 100;
            let entropy = shannons_entropy_sampled_encoded(kmers, samples);
            (1.0 / samples as f64, entropy)
        }).collect();

    entropy_values.push((1.0 / kmer_count as f64, all_kmers_entropy));

    if entropy_values.len() >= 5 {
        let (_, y) = linear_regression_of(&entropy_values).unwrap();
        if y < 0_f64 { return all_kmers_entropy; }
        y
    } else {
        all_kmers_entropy
    }
}

fn shannons_entropy_sampled_encoded(kmers: &[u64], sample_size: usize) -> f64 {
    // Sample indices and count without allocating strings
    let mut rng = rand::thread_rng();
    let mut counts: HashMap<u64, usize> = HashMap::new();
    for _ in 0..sample_size {
        let chosen = *kmers.choose(&mut rng).unwrap();
        *counts.entry(chosen).or_insert(0) += 1;
    }
    let mut sum = 0.0f64;
    for (_k, &c) in counts.iter() {
        let p = c as f64 / sample_size as f64;
        sum += p * p.log2();
    }
    if sum == 0.0 { 0.0 } else { -sum }
}

fn count_only_encoded(kmers: &[u64]) -> Vec<(u64, usize)> {
    let mut counts: HashMap<u64, usize> = HashMap::new();
    kmers.iter().for_each(|&k| { *counts.entry(k).or_insert(0) += 1; });
    counts.into_iter().collect()
} 