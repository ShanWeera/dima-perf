//! Turns the counts of one position into its result: support class, entropy estimate, sample
//! coverage, diversity motifs, incidences and per-variant metadata.

use crate::config::Estimator;
use crate::engine::PositionCounts;
use crate::entropy::{self, CountProfile};
use crate::error::Result;
use crate::model::{FieldCounts, Metadata, PositionResult, Variant};
use crate::stats;

/// Everything needed to finalise positions, shared by all of them.
#[derive(Debug)]
pub struct Finaliser<'a> {
    /// Support threshold *T*.
    pub threshold: u64,
    /// Entropy estimator.
    pub estimator: Estimator,
    /// `--min-coverage`, if given.
    pub min_coverage: Option<f64>,
    /// Stratified field names, in header order (empty without metadata).
    pub fields: &'a [String],
    /// For each header value id: (stratified field index, value).
    pub values: &'a [(usize, String)],
}

impl Finaliser<'_> {
    /// The result of position `position` (1-based).
    ///
    /// # Errors
    /// Internal errors from the estimators (non-finite values).
    pub fn finalise(&self, position: u64, counts: &PositionCounts) -> Result<PositionResult> {
        let ids = counts.ordered_ids();
        let ordered: Vec<u64> = ids.iter().map(|&id| counts.count(id)).collect();
        let support: u64 = ordered.iter().sum();
        let motifs = stats::classify_motifs(&ordered);
        let diversity = stats::diversity(&ordered, &motifs);
        let profile = CountProfile::from_counts(ordered.iter().copied())?;
        let estimate = entropy::estimate(&profile, self.estimator, self.threshold)?;
        let coverage = entropy::sample_coverage(&profile);
        let diversity_motifs = ids
            .iter()
            .zip(&ordered)
            .zip(&motifs)
            .map(|((&id, &count), &motif)| Variant {
                // Canonical bytes are upper-case ASCII letters.
                sequence: String::from_utf8_lossy(counts.kmer(id)).into_owned(),
                count,
                incidence: stats::incidence(count, support),
                motif,
                metadata: (!self.fields.is_empty()).then(|| self.metadata(counts, id)),
            })
            .collect();
        Ok(PositionResult {
            position,
            low_support: stats::support_label(support, self.threshold),
            support,
            distinct_variants_count: diversity.distinct_variants_count,
            entropy: estimate.entropy,
            observed_entropy: estimate.observed,
            entropy_method: estimate.method,
            entropy_fallback: estimate.fallback,
            distinct_variants_incidence: diversity.distinct_variants_incidence,
            total_variants_incidence: diversity.total_variants_incidence,
            sample_coverage: coverage,
            below_min_coverage: self.min_coverage.and_then(|m| coverage.map(|c| c < m)),
            diversity_motifs,
        })
    }

    fn metadata(&self, counts: &PositionCounts, id: u32) -> Metadata {
        let mut per_field: Vec<Vec<(String, u64)>> = vec![Vec::new(); self.fields.len()];
        for (value_id, n) in counts.cells(id) {
            let (field, value) = &self.values[value_id as usize];
            per_field[*field].push((value.clone(), n));
        }
        Metadata(
            per_field
                .into_iter()
                .zip(self.fields)
                .map(|(mut values, field)| {
                    values.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                    FieldCounts {
                        field: field.clone(),
                        values,
                    }
                })
                .collect(),
        )
    }
}
