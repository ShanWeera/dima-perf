use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use std::fmt;

use std::fs::File;
use std::io::{BufWriter, Write};

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
    pub distinct_variants_incidence: f64,
    pub total_variants_incidence: f64,
    pub diversity_motifs: Option<Vec<Variant>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Variant {
    pub sequence: String,
    pub count: usize,
    pub incidence: f64,
    pub motif_short: Option<String>,
    pub motif_long: Option<String>,
    #[serde(with = "crate::models::serde_hashmap_opt")]
    pub metadata: Option<HashMap<String, HashMap<String, usize>>>,
}

pub mod serde_hashmap_opt {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Serialize metadata with sorted keys for deterministic JSON output. (Fix 7.28)
    pub fn serialize<S>(
        value: &Option<HashMap<String, HashMap<String, usize>>>,
        s: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(map) => {
                let sorted: std::collections::BTreeMap<
                    &String,
                    std::collections::BTreeMap<&String, &usize>,
                > = map.iter().map(|(k, v)| (k, v.iter().collect())).collect();
                sorted.serialize(s)
            }
            None => s.serialize_none(),
        }
    }

    /// Properly handles both null JSON values and object values.
    /// Previously this always tried HashMap::deserialize which fails on null.
    #[allow(clippy::type_complexity)]
    pub fn deserialize<'de, D>(
        d: D,
    ) -> Result<Option<HashMap<String, HashMap<String, usize>>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<HashMap<String, HashMap<String, usize>>>::deserialize(d)
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_string_pretty(self) {
            Ok(s) => f.write_str(&s),
            Err(e) => write!(f, "<serialization error: {}>", e),
        }
    }
}

impl fmt::Debug for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Variant {{ seq: \"{}\", count: {}, incidence: {:.2}% }}",
            self.sequence, self.count, self.incidence
        )
    }
}

impl fmt::Display for Results {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Results {{ query: \"{}\", positions: {}, sequences: {}, k: {}, avg_entropy: {:.4} }}",
            self.query_name,
            self.results.len(),
            self.sequence_count,
            self.kmer_length,
            self.average_entropy
        )
    }
}

impl fmt::Debug for Results {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Results {{ query: \"{}\", positions: {}, sequences: {}, k: {}, avg_entropy: {:.4} }}",
            self.query_name,
            self.results.len(),
            self.sequence_count,
            self.kmer_length,
            self.average_entropy
        )
    }
}

/// Find the length of the overlap between the end of `acc` and the start of `kmer`.
///
/// For k-mers of length k at adjacent positions in a sliding window, the expected
/// overlap is exactly k-1 (one position shift). However, when adjacent positions
/// have DIFFERENT Index sequences (e.g., tied Index variants), the actual overlap
/// may differ. We return the longest valid prefix-suffix overlap found.
///
/// Returns 0 if no valid prefix-suffix overlap is found, which indicates
/// non-contiguous sequences that should start a new HCS region.
fn find_overlap_length(acc: &str, kmer: &str) -> usize {
    let kmer_len = kmer.len();
    if kmer_len == 0 || acc.is_empty() {
        return 0;
    }
    // Check overlaps from longest possible (kmer_len - 1) down to 1.
    // We cap at kmer_len - 1 because a full overlap means the k-mers are identical
    // (same position), which shouldn't happen in a correctly processed sliding window.
    let max_check = acc.len().min(kmer_len - 1);
    for overlap in (1..=max_check).rev() {
        if acc.ends_with(&kmer[..overlap]) {
            return overlap;
        }
    }
    0
}

impl Results {
    /// Compute Highly Conserved Sequences (HCS) by stitching contiguous Index k-mers.
    ///
    /// Algorithm (per PMC11596295):
    /// 1. Walk positions in order; at each position select the lexicographically
    ///    first Index variant above the threshold.
    /// 2. A position without a qualifying Index breaks the current HCS region.
    /// 3. Adjacent k-mers are stitched by their (k-1)-overlap: the non-overlapping
    ///    suffix is appended (may be >1 char if overlap < k-1).
    /// 4. Single-position Index k-mers ARE valid HCS regions (length = k).
    pub fn get_hcs(
        &self,
        path: Option<String>,
        threshold: Option<f64>,
    ) -> Result<Vec<String>, std::io::Error> {
        let mut hcs_out: Vec<String> = Vec::new();
        let mut acc = String::new();
        // Track which HCS positions have low support for the warning
        let mut current_region_low_support: Vec<usize> = Vec::new();
        let mut all_low_support_positions: Vec<(usize, Vec<usize>)> = Vec::new();

        // Defensive sort by position index. During normal analysis, positions are
        // already in order, but deserialized data (JSON/binary round-trip) may not be.
        // Sorting guarantees correct consecutive-position stitching regardless of source.
        let mut sorted_positions: Vec<&Position> = self.results.iter().collect();
        sorted_positions.sort_unstable_by_key(|p| p.position);

        // Track last qualifying position for explicit consecutive-position check
        let mut last_qualifying_position: Option<usize> = None;

        for position in sorted_positions {
            // Find qualifying Index variant at this position
            let index_kmer = position.diversity_motifs.as_ref().and_then(|motifs| {
                let support = position.support;
                let mut candidates: Vec<&Variant> = motifs
                    .iter()
                    .filter(|v| v.motif_short.as_deref() == Some("I"))
                    .filter(|v| match threshold {
                        Some(t) => {
                            // Cross-multiplication avoids floating-point rounding errors
                            // from the division in incidence = (count/support)*100.
                            // A variant with 90/100 reads should pass threshold=90.0 exactly,
                            // but (90.0/100.0)*100.0 may yield 89.99999... due to IEEE 754.
                            // Cross-multiply: count*100 >= support*threshold (exact for <2^53).
                            (v.count as f64 * 100.0) >= (support as f64 * t)
                        }
                        None => true,
                    })
                    .collect();
                // Use lexicographically first Index for determinism when ties exist
                candidates.sort_by(|a, b| a.sequence.cmp(&b.sequence));
                candidates.first().map(|v| v.sequence.as_str())
            });

            match index_kmer {
                Some(sequence) => {
                    if position.low_support.is_some() {
                        current_region_low_support.push(position.position);
                    }

                    // Positions must be consecutive (position N+1 == last + 1) for valid
                    // HCS stitching. Non-consecutive qualifying positions start a new region
                    // even if their sequences happen to share an overlap by coincidence.
                    let is_consecutive =
                        last_qualifying_position.map_or(true, |last| position.position == last + 1);

                    if acc.is_empty() || !is_consecutive {
                        // Start a new region (either first qualifying position, or gap detected)
                        if !acc.is_empty() {
                            hcs_out.push(acc);
                            if !current_region_low_support.is_empty() {
                                all_low_support_positions
                                    .push((hcs_out.len(), current_region_low_support.clone()));
                                current_region_low_support.clear();
                            }
                        }
                        acc = sequence.to_string();
                    } else {
                        let overlap_len = find_overlap_length(&acc, sequence);
                        // For k>1, adjacent k-mers share a (k-1)-character overlap.
                        // For k=1, there's no overlap between single-char k-mers,
                        // but adjacent positions are still contiguous (overlap=0 is valid).
                        let min_required_overlap = self.kmer_length.saturating_sub(1);
                        if overlap_len >= min_required_overlap {
                            acc.push_str(&sequence[overlap_len..]);
                        } else {
                            hcs_out.push(acc);
                            if !current_region_low_support.is_empty() {
                                all_low_support_positions
                                    .push((hcs_out.len(), current_region_low_support.clone()));
                                current_region_low_support.clear();
                            }
                            acc = sequence.to_string();
                        }
                    }
                    last_qualifying_position = Some(position.position);
                }
                None => {
                    if !acc.is_empty() {
                        hcs_out.push(acc);
                        if !current_region_low_support.is_empty() {
                            all_low_support_positions
                                .push((hcs_out.len(), current_region_low_support.clone()));
                            current_region_low_support.clear();
                        }
                        acc = String::new();
                    }
                }
            }
        }
        if !acc.is_empty() {
            hcs_out.push(acc);
            if !current_region_low_support.is_empty() {
                all_low_support_positions.push((hcs_out.len(), current_region_low_support.clone()));
            }
        }

        // Warn about HCS regions that include low-support positions,
        // which may be statistically unreliable despite meeting the incidence threshold
        if !all_low_support_positions.is_empty() {
            let total: usize = all_low_support_positions.iter().map(|(_, v)| v.len()).sum();
            tracing::warn!(
                low_support_positions = total,
                "HCS contains positions with low support — these regions may be statistically unreliable"
            );
        }

        if let Some(save_path) = path {
            // Atomic write: temp file + fsync + rename (same pattern as to_json)
            let final_path = std::path::Path::new(&save_path);
            let tmp_path = final_path.with_extension("hcs.tmp");
            {
                let file = File::create(&tmp_path)?;
                let mut writer = BufWriter::new(&file);
                serde_json::to_writer_pretty(&mut writer, &hcs_out)?;
                writer.flush()?;
                file.sync_all()?;
            }
            std::fs::rename(&tmp_path, final_path)?;
            Ok(hcs_out)
        } else {
            Ok(hcs_out)
        }
    }

    /// Validates that all float fields are finite (not NaN or Infinity).
    /// serde_json silently serializes NaN/Inf as `null`, causing silent data corruption
    /// in downstream tools that expect numeric values. This converts that into an
    /// actionable error message identifying the exact offending field.
    fn validate_output_floats(&self) -> Result<(), std::io::Error> {
        if !self.average_entropy.is_finite() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "average_entropy is non-finite ({}); this indicates a bug in the \
                     entropy calculation pipeline",
                    self.average_entropy
                ),
            ));
        }
        if !self.highest_entropy.entropy.is_finite() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "highest_entropy.entropy is non-finite ({})",
                    self.highest_entropy.entropy
                ),
            ));
        }
        for pos in &self.results {
            if !pos.entropy.is_finite() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "position {} has non-finite entropy ({}); cannot serialize to JSON",
                        pos.position, pos.entropy
                    ),
                ));
            }
            if !pos.distinct_variants_incidence.is_finite() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "position {} has non-finite distinct_variants_incidence ({})",
                        pos.position, pos.distinct_variants_incidence
                    ),
                ));
            }
            if !pos.total_variants_incidence.is_finite() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "position {} has non-finite total_variants_incidence ({})",
                        pos.position, pos.total_variants_incidence
                    ),
                ));
            }
            if let Some(ref motifs) = pos.diversity_motifs {
                for v in motifs {
                    if !v.incidence.is_finite() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "position {} variant '{}' has non-finite incidence ({})",
                                pos.position, v.sequence, v.incidence
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn to_json(&self, path: Option<String>) -> Result<String, std::io::Error> {
        self.validate_output_floats()?;

        if let Some(save_path) = path {
            // Atomic write: serialize to a temp file, fsync, then rename into place.
            // This prevents partial/corrupt JSON if the process crashes mid-write.
            let final_path = std::path::Path::new(&save_path);
            let tmp_path = final_path.with_extension("json.tmp");
            {
                let file = File::create(&tmp_path)?;
                let mut writer = BufWriter::new(&file);
                serde_json::to_writer_pretty(&mut writer, &self)?;
                writer.flush()?;
                file.sync_all()?;
            }
            std::fs::rename(&tmp_path, final_path)?;
            Ok(String::new())
        } else {
            serde_json::to_string_pretty(&self)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }
    }

    /// Save results in binary format for improved performance
    pub fn to_binary(
        &self,
        path: String,
        config: Option<crate::binary::BinaryFormatConfig>,
    ) -> Result<(), std::io::Error> {
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
        let variant_count = self.diversity_motifs.as_ref().map_or(0, |v| v.len());
        write!(
            f,
            "Position {{ pos: {}, entropy: {:.4}, variants: {}, low_support: {:?} }}",
            self.position, self.entropy, variant_count, self.low_support
        )
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant_count = self.diversity_motifs.as_ref().map_or(0, |v| v.len());
        write!(
            f,
            "Position {{ pos: {}, entropy: {:.4}, variants: {}, low_support: {:?} }}",
            self.position, self.entropy, variant_count, self.low_support
        )
    }
}

impl Position {
    pub fn get_minors(&self, sort: Option<String>) -> Option<Vec<Variant>> {
        let mut variant_matches = self
            .diversity_motifs
            .as_ref()?
            .iter()
            .filter(|variant| variant.motif_short.as_deref() == Some("Mi"))
            .cloned()
            .collect::<Vec<Variant>>();

        variant_matches.sort_unstable_by(|a, b| match sort.as_deref() {
            None | Some("asc") => a.count.cmp(&b.count),
            Some("desc") => b.count.cmp(&a.count),
            _ => a.count.cmp(&b.count),
        });

        Some(variant_matches)
    }
}

/// Count non-Index variant *types* and their combined read count.
///
/// Returns (distinct_count, total_non_index_reads) where:
///   - `distinct_count`: number of distinct k-mer types NOT classified as Index
///   - `total_non_index_reads`: sum of counts across those non-Index types
fn get_distinct_variant_counts(variants: &[Variant]) -> (usize, usize) {
    let mut count = 0usize;
    let mut total = 0usize;

    for variant in variants {
        if variant.motif_short.as_deref() != Some("I") {
            total += variant.count;
            count += 1;
        }
    }

    (count, total)
}

/// Populate summary statistics on a Position from its classified variants.
///
/// **Formulae (per PMC11596295, page 4):**
///
/// *distinct_variants_incidence* = (number of non-Index k-mer *types*)
///                                 / (total non-Index k-mer *reads*) × 100
///   → Measures the "type richness" of the minority population.
///     A value of 100% means every non-Index read is a unique type.
///     Example: 3 non-Index types with total reads {5, 3, 2} → 3/10 × 100 = 30%
///
/// *total_variants_incidence* = (total non-Index k-mer *reads*)
///                              / (support, i.e. total reads) × 100
///   → Measures what fraction of all reads at this position are NOT the Index.
///     Example: 10 non-Index reads out of 100 support → 10/100 × 100 = 10%
fn set_pos_obj_data(position_obj: &mut Position, variants: &[Variant], support: &usize) {
    let (distinct_variant_count, distinct_variants_total) = get_distinct_variant_counts(variants);

    let non_index_total = distinct_variants_total;
    let distinct_variants_incidence: f64 = if non_index_total > 0 {
        (distinct_variant_count as f64 / non_index_total as f64) * 100.0
    } else {
        0.0
    };

    let total_variance: f64 = if *support > 0 {
        (distinct_variants_total as f64 / *support as f64) * 100.0
    } else {
        0.0
    };

    position_obj.distinct_variants_count = distinct_variant_count;
    position_obj.distinct_variants_incidence =
        if distinct_variants_incidence.is_nan() || distinct_variants_incidence.is_infinite() {
            0.0
        } else {
            distinct_variants_incidence
        };
    position_obj.diversity_motifs = Some(variants.to_vec());
    position_obj.total_variants_incidence =
        if total_variance.is_nan() || total_variance.is_infinite() {
            0.0
        } else {
            total_variance
        };
}

impl Position {
    /// Construct a new Position, classifying variants into motif categories.
    ///
    /// Motif classification follows PMC11596295 (page 5):
    /// - **Index (I)**: highest count (count > 1). Per the paper: "in some instances,
    ///   a position may exhibit more than one index or major variant, which is observed
    ///   when two or more distinct k-mer sequences are of the same incidence."
    ///   All tied-at-max variants receive Index classification.
    /// - **Unique (U)**: count == 1 (singleton)
    /// - **Major (Ma)**: highest count among remaining (after Index/Unique assigned).
    ///   Ties are handled identically to Index — all tied variants get Major.
    /// - **Minor (Mi)**: everything else
    ///
    /// Variants are sorted deterministically: count DESC, sequence ASC for stable output.
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

        let variants_unwrapped = match variants {
            Some(v) if !v.is_empty() => v,
            _ => return position_obj,
        };

        // Find the maximum count across all variants
        let max_incidence = match variants_unwrapped.iter().max_by_key(|v| v.count) {
            Some(v) => v.count,
            None => return position_obj,
        };

        // Phase 1: Classify Index and Unique
        variants_unwrapped.iter_mut().for_each(|variant| {
            if variant.count == max_incidence && variant.count != 1 {
                variant.motif_long = Some("Index".to_owned());
                variant.motif_short = Some("I".to_string());
            } else if variant.count == 1 {
                variant.motif_long = Some("Unique".to_owned());
                variant.motif_short = Some("U".to_string());
            }
        });

        // Phase 2: Classify Major/Minor among remaining unclassified
        let pending_classification: Vec<&mut Variant> = variants_unwrapped
            .iter_mut()
            .filter(|variant| variant.motif_long.is_none())
            .collect();

        if !pending_classification.is_empty() {
            let major_count = pending_classification
                .iter()
                .map(|v| v.count)
                .max()
                .unwrap_or(0);

            for variant in pending_classification {
                if variant.count == major_count {
                    variant.motif_long = Some("Major".to_owned());
                    variant.motif_short = Some("Ma".to_string());
                } else {
                    variant.motif_long = Some("Minor".to_owned());
                    variant.motif_short = Some("Mi".to_string());
                }
            }
        }

        // Sort variants deterministically: count DESC, sequence ASC
        variants_unwrapped.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.sequence.cmp(&b.sequence))
        });

        set_pos_obj_data(&mut position_obj, variants_unwrapped.as_slice(), &support);
        position_obj
    }
}

/// Find the position with the highest entropy value from a raw entropy array.
/// Returns 1-based position index to match Position.position convention.
/// Returns position=0, entropy=0.0 for empty input (no positions).
/// NOTE: This operates on ALL positions — caller should pre-filter for reliability if needed.
#[allow(dead_code)]
pub fn highest_entropy(numbers: &[f64]) -> HighestEntropy {
    if numbers.is_empty() {
        return HighestEntropy {
            position: 0,
            entropy: 0.0,
        };
    }

    let highest_position = numbers
        .iter()
        .enumerate()
        .reduce(|acc, item| if acc.1 > item.1 { acc } else { item })
        .unwrap(); // Safe: we checked non-empty above

    // Convert 0-based index to 1-based position (matches Position.position)
    HighestEntropy {
        position: highest_position.0 + 1,
        entropy: *highest_position.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_variant(seq: &str, count: usize) -> Variant {
        Variant {
            sequence: seq.to_string(),
            count,
            incidence: 0.0,
            motif_short: None,
            motif_long: None,
            metadata: None,
        }
    }

    #[test]
    fn test_all_same_kmer_100pct_index() {
        let mut variants = vec![make_variant("AAA", 100)];
        let pos = Position::new(1, 0.0, 100, Some(&mut variants), None);
        let motifs = pos.diversity_motifs.unwrap();
        assert_eq!(motifs.len(), 1);
        assert_eq!(motifs[0].motif_short.as_deref(), Some("I"));
    }

    #[test]
    fn test_all_unique_singletons() {
        let mut variants = vec![
            make_variant("AAA", 1),
            make_variant("BBB", 1),
            make_variant("CCC", 1),
        ];
        let pos = Position::new(1, 1.58, 3, Some(&mut variants), None);
        let motifs = pos.diversity_motifs.unwrap();
        // All count==1 AND all tied at max → they get Index if count > 1 fails,
        // so they should all be Unique
        for m in &motifs {
            assert_eq!(m.motif_short.as_deref(), Some("U"));
        }
    }

    #[test]
    fn test_tied_max_counts_multiple_index() {
        let mut variants = vec![
            make_variant("AAA", 10),
            make_variant("BBB", 10),
            make_variant("CCC", 3),
        ];
        let pos = Position::new(1, 1.0, 23, Some(&mut variants), None);
        let motifs = pos.diversity_motifs.unwrap();
        let index_count = motifs
            .iter()
            .filter(|v| v.motif_short.as_deref() == Some("I"))
            .count();
        assert_eq!(index_count, 2, "Both tied-at-max variants should be Index");
        // CCC is the sole remaining non-Index variant → it's the max among pending → Major
        let remaining = motifs.iter().find(|v| v.sequence == "CCC").unwrap();
        assert_eq!(remaining.motif_short.as_deref(), Some("Ma"));
    }

    #[test]
    fn test_standard_index_major_minor_unique_mix() {
        let mut variants = vec![
            make_variant("AAA", 50),
            make_variant("BBB", 20),
            make_variant("CCC", 5),
            make_variant("DDD", 1),
        ];
        let pos = Position::new(1, 1.5, 76, Some(&mut variants), None);
        let motifs = pos.diversity_motifs.unwrap();
        let find = |seq: &str| motifs.iter().find(|v| v.sequence == seq).unwrap();
        assert_eq!(find("AAA").motif_short.as_deref(), Some("I"));
        assert_eq!(find("BBB").motif_short.as_deref(), Some("Ma"));
        assert_eq!(find("CCC").motif_short.as_deref(), Some("Mi"));
        assert_eq!(find("DDD").motif_short.as_deref(), Some("U"));
    }

    #[test]
    fn test_tied_major_variants() {
        let mut variants = vec![
            make_variant("AAA", 50),
            make_variant("BBB", 10),
            make_variant("CCC", 10),
            make_variant("DDD", 1),
        ];
        let pos = Position::new(1, 1.2, 71, Some(&mut variants), None);
        let motifs = pos.diversity_motifs.unwrap();
        let major_count = motifs
            .iter()
            .filter(|v| v.motif_short.as_deref() == Some("Ma"))
            .count();
        assert_eq!(
            major_count, 2,
            "Both tied non-Index max variants should be Major"
        );
    }

    #[test]
    fn test_empty_variants_returns_zero_stats() {
        let pos = Position::new(1, 0.0, 0, None, None);
        assert!(pos.diversity_motifs.is_none());
        assert_eq!(pos.distinct_variants_count, 0);
        assert_eq!(pos.distinct_variants_incidence, 0.0);
    }

    #[test]
    fn test_low_support_tag_preserved() {
        let pos = Position::new(1, 0.5, 10, None, Some("LS".to_string()));
        assert_eq!(pos.low_support.as_deref(), Some("LS"));
    }

    #[test]
    fn test_deterministic_sort_order() {
        let mut variants = vec![
            make_variant("CCC", 10),
            make_variant("AAA", 10),
            make_variant("BBB", 5),
        ];
        let pos = Position::new(1, 1.0, 25, Some(&mut variants), None);
        let motifs = pos.diversity_motifs.unwrap();
        // Sorted by count DESC, then sequence ASC
        assert_eq!(motifs[0].sequence, "AAA");
        assert_eq!(motifs[1].sequence, "CCC");
        assert_eq!(motifs[2].sequence, "BBB");
    }

    #[test]
    fn test_highest_entropy_empty() {
        let result = highest_entropy(&[]);
        assert_eq!(result.position, 0);
        assert_eq!(result.entropy, 0.0);
    }

    #[test]
    fn test_highest_entropy_single() {
        let result = highest_entropy(&[1.5]);
        assert_eq!(result.position, 1);
        assert_eq!(result.entropy, 1.5);
    }

    #[test]
    fn test_highest_entropy_multiple() {
        let result = highest_entropy(&[0.5, 2.0, 1.0, 0.8]);
        assert_eq!(result.position, 2); // 1-based index
        assert_eq!(result.entropy, 2.0);
    }

    #[test]
    fn test_distinct_variants_incidence_calculation() {
        let mut variants = vec![
            make_variant("AAA", 100), // Index
            make_variant("BBB", 20),  // Major
            make_variant("CCC", 10),  // Minor
            make_variant("DDD", 1),   // Unique
        ];
        let pos = Position::new(1, 1.5, 131, Some(&mut variants), None);
        // Non-Index types: BBB(20) + CCC(10) + DDD(1) = 31 total reads, 3 types
        // distinct_variants_incidence = 3/31 * 100 ≈ 9.677
        assert!((pos.distinct_variants_incidence - 9.677).abs() < 0.01);
        // total_variants_incidence = 31/131 * 100 ≈ 23.664
        assert!((pos.total_variants_incidence - 23.664).abs() < 0.01);
    }

    // ─── HCS Unit Tests ──────────────────────────────────────────────────────

    fn make_index_variant(seq: &str, count: usize, total: usize) -> Variant {
        Variant {
            sequence: seq.to_string(),
            count,
            incidence: (count as f64 / total as f64) * 100.0,
            motif_short: Some("I".to_string()),
            motif_long: Some("Index".to_string()),
            metadata: None,
        }
    }

    fn make_results_for_hcs(positions: Vec<Position>, kmer_length: usize) -> Results {
        Results {
            sequence_count: 100,
            support_threshold: 10,
            low_support_count: 0,
            query_name: "test".to_string(),
            kmer_length,
            highest_entropy: HighestEntropy {
                position: 1,
                entropy: 0.0,
            },
            average_entropy: 0.0,
            results: positions,
        }
    }

    #[test]
    fn test_hcs_basic_stitching_adjacent_positions() {
        // k=3: ABC, BCD, CDE → overlap by 2 → "ABCDE"
        let positions = vec![
            Position {
                position: 1,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("ABC", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
            Position {
                position: 2,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("BCD", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
            Position {
                position: 3,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("CDE", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
        ];
        let results = make_results_for_hcs(positions, 3);
        let hcs = results.get_hcs(None, None).unwrap();
        assert_eq!(hcs, vec!["ABCDE"]);
    }

    #[test]
    fn test_hcs_gap_splits_regions() {
        // Position 2 has no Index → splits into two HCS regions
        let positions = vec![
            Position {
                position: 1,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("ABC", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
            Position {
                position: 2,
                entropy: 1.5,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![Variant {
                    sequence: "XYZ".to_string(),
                    count: 50,
                    incidence: 50.0,
                    motif_short: Some("Ma".to_string()),
                    motif_long: Some("Major".to_string()),
                    metadata: None,
                }]),
                distinct_variants_count: 2,
                distinct_variants_incidence: 50.0,
                total_variants_incidence: 50.0,
            },
            Position {
                position: 3,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("DEF", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
        ];
        let results = make_results_for_hcs(positions, 3);
        let hcs = results.get_hcs(None, None).unwrap();
        assert_eq!(hcs, vec!["ABC", "DEF"]);
    }

    #[test]
    fn test_hcs_single_position_produces_kmer_length_region() {
        // A single qualifying position still produces a valid HCS of length k
        let positions = vec![Position {
            position: 1,
            entropy: 0.0,
            support: 100,
            low_support: None,
            diversity_motifs: Some(vec![make_index_variant("ABCDEF", 100, 100)]),
            distinct_variants_count: 1,
            distinct_variants_incidence: 0.0,
            total_variants_incidence: 0.0,
        }];
        let results = make_results_for_hcs(positions, 6);
        let hcs = results.get_hcs(None, None).unwrap();
        assert_eq!(hcs, vec!["ABCDEF"]);
    }

    #[test]
    fn test_hcs_kmer1_no_overlap_concatenation() {
        // k=1: single-char k-mers have zero overlap, adjacent ones still merge
        let positions = vec![
            Position {
                position: 1,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("A", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
            Position {
                position: 2,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("B", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
            Position {
                position: 3,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("C", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
        ];
        let results = make_results_for_hcs(positions, 1);
        let hcs = results.get_hcs(None, None).unwrap();
        assert_eq!(hcs, vec!["ABC"]);
    }

    #[test]
    fn test_hcs_empty_results_no_regions() {
        let results = make_results_for_hcs(Vec::new(), 3);
        let hcs = results.get_hcs(None, None).unwrap();
        assert!(hcs.is_empty());
    }

    #[test]
    fn test_hcs_threshold_filters_low_incidence() {
        // With threshold=90%, only position 1 (100%) qualifies, position 2 (50%) doesn't
        let positions = vec![
            Position {
                position: 1,
                entropy: 0.0,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("ABC", 100, 100)]),
                distinct_variants_count: 1,
                distinct_variants_incidence: 0.0,
                total_variants_incidence: 0.0,
            },
            Position {
                position: 2,
                entropy: 0.5,
                support: 100,
                low_support: None,
                diversity_motifs: Some(vec![make_index_variant("BCD", 50, 100)]),
                distinct_variants_count: 2,
                distinct_variants_incidence: 50.0,
                total_variants_incidence: 50.0,
            },
        ];
        let results = make_results_for_hcs(positions, 3);
        let hcs = results.get_hcs(None, Some(90.0)).unwrap();
        assert_eq!(hcs, vec!["ABC"]);
    }

    #[test]
    fn test_find_overlap_length_basic() {
        assert_eq!(find_overlap_length("ABCD", "CDE"), 2);
        assert_eq!(find_overlap_length("ABC", "BCD"), 2);
        assert_eq!(find_overlap_length("ABC", "XYZ"), 0);
        assert_eq!(find_overlap_length("", "ABC"), 0);
        assert_eq!(find_overlap_length("ABC", ""), 0);
    }

    #[test]
    fn test_find_overlap_length_single_char() {
        assert_eq!(find_overlap_length("A", "A"), 0);
        assert_eq!(find_overlap_length("AB", "B"), 0);
    }
}
