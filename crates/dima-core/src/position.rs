//! Turns the counts of one position into its result: support class, entropy estimate, sample
//! coverage, diversity motifs, incidences, per-variant metadata and, when requested,
//! similarity-weighted entropy and the normalised entropies (Wu 2025).

use crate::config::Estimator;
use crate::engine::PositionCounts;
use crate::entropy::{self, CountProfile};
use crate::error::{DimaError, Result};
use crate::model::{FieldCounts, Metadata, PositionResult, Variant};
use crate::{similarity, stats};

/// Everything needed to finalise positions, shared by all of them.
#[derive(Debug)]
pub struct Finaliser<'a> {
    /// Support threshold *T*.
    pub threshold: u64,
    /// Entropy estimator.
    pub estimator: Estimator,
    /// `--min-coverage`, if given.
    pub min_coverage: Option<f64>,
    /// `--similarity-weighted-entropy`: also compute H_sim, H_n and H_nsim.
    pub similarity_weighted_entropy: bool,
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
        let similarity = if self.similarity_weighted_entropy {
            Similarity::of(counts, &ids, &profile, estimate.observed)?
        } else {
            Similarity::default()
        };
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
            similarity_weighted_entropy: similarity.weighted,
            normalised_observed_entropy: similarity.normalised_observed,
            normalised_similarity_weighted_entropy: similarity.normalised_weighted,
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

/// Similarity-weighted entropy and the normalised entropies of one position (Wu 2025, Section
/// 2.1); all `None` when not requested.
#[derive(Debug, Default)]
struct Similarity {
    weighted: Option<f64>,
    normalised_observed: Option<f64>,
    normalised_weighted: Option<f64>,
}

impl Similarity {
    /// The three values of a position whose observed entropy is `observed` (`None` without
    /// support, where all three are `None` too). Normalisation divides by log₂ of the number of
    /// distinct k-mers, the Index included (Wu's number of unique sequences).
    fn of(
        counts: &PositionCounts,
        ids: &[u32],
        profile: &CountProfile,
        observed: Option<f64>,
    ) -> Result<Self> {
        let Some(observed) = observed else {
            return Ok(Self::default());
        };
        let weighted = similarity::similarity_weighted_entropy(
            ids.iter().map(|&id| (counts.kmer(id), counts.count(id))),
        )?
        .ok_or_else(|| DimaError::internal("a position with an observed entropy has no k-mers"))?;
        // Postcondition: 0 ≤ H_sim ≤ H. The weights are at most 1, so only rounding could
        // exceed H, by a few units in the last place (measured: at most 2.2e-16 relative).
        if !(weighted.is_finite() && weighted >= 0.0 && weighted <= observed * (1.0 + 1e-12)) {
            return Err(DimaError::internal(format!(
                "similarity-weighted entropy {weighted:?} is not a number between 0 and the \
                 observed entropy {observed:?}"
            )));
        }
        let distinct = profile.distinct();
        Ok(Self {
            weighted: Some(weighted),
            normalised_observed: entropy::normalised(observed, distinct)?,
            normalised_weighted: entropy::normalised(weighted, distinct)?,
        })
    }
}
