use serde::{Serialize, Deserialize};
use std::fmt;
use hashbrown::HashMap;

use crate::kmer::has_overlap_end;
use std::fs::File;
use std::io::BufWriter;

#[derive(Serialize, Deserialize, Clone)]
pub struct HighestEntropy {
    pub position: usize,
    pub entropy: f64,
}

#[derive(Serialize, Deserialize)]
pub struct Results {
    pub sequence_count: usize,
    pub support_threshold: usize,
    pub low_support_count: usize,
    pub query_name: String,
    pub kmer_length: usize,
    pub highest_entropy: HighestEntropy,
    pub average_entropy: f64,
    pub results: Vec<Position>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Position {
    pub position: usize,
    pub low_support: Option<String>,
    pub entropy: f64,
    pub support: usize,
    pub distinct_variants_count: usize,
    pub distinct_variants_incidence: f32,
    pub total_variants_incidence: f32,
    pub diversity_motifs: Option<Vec<Variant>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Variant {
    pub sequence: String,
    pub count: usize,
    pub incidence: f32,
    pub motif_short: Option<String>,
    pub motif_long: Option<String>,
    #[serde(with = "crate::models::serde_hashmap_opt")]
    pub metadata: Option<HashMap<String, HashMap<String, usize>>>,
}

pub mod serde_hashmap_opt {
    use super::*;
    use serde::{Serialize, Serializer, Deserialize, Deserializer};

    pub fn serialize<S>(value: &Option<HashMap<String, HashMap<String, usize>>>, s: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        match value {
            Some(map) => map.serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<HashMap<String, HashMap<String, usize>>>, D::Error>
    where D: Deserializer<'de> {
        Ok(Some(HashMap::<String, HashMap<String, usize>>::deserialize(d)?))
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string_pretty(self).unwrap().fmt(f)
    }
}

impl fmt::Debug for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string_pretty(self).unwrap().fmt(f)
    }
}

impl fmt::Display for Results {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string_pretty(self).unwrap().fmt(f)
    }
}

impl fmt::Debug for Results {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string_pretty(self).unwrap().fmt(f)
    }
}

impl Results {
    pub fn get_hcs(&self, path: Option<String>, threshold: Option<f32>) -> Result<Vec<String>, std::io::Error> {
        let mut hcs_out: Vec<String> = Vec::new();
        let mut acc = String::new();

        for position in &self.results {
            if let Some(motifs) = &position.diversity_motifs {
                for variant in motifs.iter().filter(|v| v.motif_short.as_deref() == Some("I")) {
                    if let Some(t) = threshold {
                        if variant.incidence < t { continue; }
                    }
                    let sequence = variant.sequence.as_str();
                    if acc.is_empty() {
                        acc.push_str(sequence);
                    } else if has_overlap_end(acc.as_str(), sequence) {
                        if let Some(last) = sequence.chars().last() {
                            acc.push(last);
                        }
                    } else {
                        hcs_out.push(acc);
                        acc = sequence.to_string();
                    }
                }
            }
        }
        if !acc.is_empty() { hcs_out.push(acc); }

        if let Some(save_path) = path {
            let file = File::create(save_path)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &hcs_out)?;
            Ok(hcs_out)
        } else {
            Ok(hcs_out)
        }
    }

    pub fn to_json(&self, path: Option<String>) -> Result<String, std::io::Error> {
        if let Some(save_path) = path {
            let file = File::create(save_path)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &self)?;
            Ok(String::new())
        } else {
            Ok(serde_json::to_string_pretty(&self).unwrap())
        }
    }
    
    /// Save results in binary format for improved performance
    pub fn to_binary(&self, path: String, config: Option<crate::binary::BinaryFormatConfig>) -> Result<(), std::io::Error> {
        crate::binary::BinaryFormat::write_to_file(self, &path, config)
    }
    
    /// Load results from binary format
    pub fn from_binary(path: String) -> Result<Results, std::io::Error> {
        crate::binary::BinaryFormat::read_from_file(&path)
    }
    
    /// Compare JSON vs binary format sizes
    pub fn compare_formats(&self) -> Result<(usize, usize, f64), std::io::Error> {
        crate::binary::BinaryFormat::compare_formats(self)
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string_pretty(self).unwrap().fmt(f)
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string_pretty(self).unwrap().fmt(f)
    }
}

impl Position {
    pub fn get_minors(&self, sort: Option<String>) -> Option<Vec<Variant>> {
        let mut variant_matches = self
            .diversity_motifs
            .as_ref()?
            .iter()
            .filter(|variant| variant.motif_short.as_ref().unwrap() == "Mi")
            .cloned()
            .collect::<Vec<Variant>>();

        variant_matches.par_sort_by(|a, b| {
            if sort.as_ref().is_none() {
                a.count.cmp(&b.count)
            } else if sort.as_ref().unwrap() == "asc" {
                a.count.cmp(&b.count)
            } else if sort.as_ref().unwrap() == "desc" {
                b.count.cmp(&a.count)
            } else {
                panic!("{}", "\n\nUnrecognized sorting option. Should either be empty, or one of:\n\t- asc\n\t- desc\n\n")
            }
        });

        Some(variant_matches)
    }
}

use rayon::prelude::*;

fn get_distinct_variant_counts(variants: &[Variant]) -> (usize, usize) {
    let distinct_variants = variants
        .into_iter()
        .filter(|variant| variant.motif_short != Some('I'.to_string()));

    let mut count = 0;
    let mut total = 0;

    for variant in distinct_variants {
        total += variant.count;
        count += 1;
    }

    (count, total)
}

fn set_pos_obj_data(position_obj: &mut Position, variants: &[Variant], support: &usize) {
    let (distinct_variant_count, distinct_variants_total) = get_distinct_variant_counts(variants);
    let index_count = support - distinct_variants_total;
    let distinct_variants_incidence = (distinct_variant_count as f32 / (*support - index_count) as f32) * 100_f32;
    let total_variance = (distinct_variants_total as f32 / *support as f32) * 100_f32;

    position_obj.distinct_variants_count = distinct_variant_count;
    position_obj.distinct_variants_incidence = if distinct_variants_incidence.is_nan() { 0.0 } else { distinct_variants_incidence };
    position_obj.diversity_motifs = Some(variants.to_vec());
    position_obj.total_variants_incidence = if total_variance.is_nan() { 0.0 } else { total_variance };
}

impl Position {
    pub fn new(
        position: usize,
        entropy: f64,
        support: usize,
        variants: Option<&mut Vec<Variant>>,
        low_support: Option<String>,
    ) -> Self {
        let mut position_obj = Self {
            position,
            support,
            entropy,
            diversity_motifs: None,
            distinct_variants_count: 0,
            total_variants_incidence: 0.0,
            distinct_variants_incidence: 0.0,
            low_support,
        };

        if variants.is_none() {
            return position_obj;
        }

        let variants_unwrapped = variants.unwrap();

        let mut max_incidence = variants_unwrapped
            .iter()
            .reduce(|a, b| if a.count < b.count { b } else { a })
            .unwrap()
            .count;

        variants_unwrapped.iter_mut().for_each(|variant| {
            if variant.count == max_incidence {
                if variant.count != 1 {
                    variant.motif_long = Some("Index".to_owned());
                    variant.motif_short = Some("I".to_string());
                }
            }

            if variant.count == 1 {
                variant.motif_long = Some("Unique".to_owned());
                variant.motif_short = Some("U".to_string());
            }
        });

        let pending_classification = &mut variants_unwrapped
            .iter_mut()
            .filter(|variant| variant.motif_long == None)
            .collect::<Vec<&mut Variant>>();

        if pending_classification.is_empty() {
            set_pos_obj_data(&mut position_obj, variants_unwrapped.as_slice(), &support);
            return position_obj;
        }

        max_incidence = pending_classification
            .iter()
            .reduce(|a, b| if a.count < b.count { b } else { a })
            .unwrap()
            .count;

        pending_classification.iter_mut().for_each(|variant| {
            if variant.count == max_incidence {
                variant.motif_long = Some("Major".parse().unwrap());
                variant.motif_short = Some("Ma".parse().unwrap());
            } else {
                variant.motif_long = Some("Minor".parse().unwrap());
                variant.motif_short = Some("Mi".parse().unwrap());
            }
        });

        set_pos_obj_data(&mut position_obj, variants_unwrapped.as_slice(), &support);
        position_obj
    }
}

pub fn highest_entropy(numbers: &[f64]) -> HighestEntropy {
    let highest_position = numbers
        .iter()
        .enumerate()
        .reduce(|acc, item| if acc.1 > item.1 { acc } else { item })
        .unwrap();

    HighestEntropy { position: highest_position.0, entropy: *highest_position.1 }
} 