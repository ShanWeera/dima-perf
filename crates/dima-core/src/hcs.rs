//! Historically conserved sequences (HCS; Tharanga et al. 2025; D12, D44, D86).
//!
//! A position *qualifies* when its Index k-mer reaches the threshold incidence, compared
//! exactly (count × 100 ≥ threshold × support in integers). Above 50% the Index variant is
//! unique, so a qualifying position has exactly one conserved k-mer. Consecutive qualifying
//! k-mers are joined into one region when they overlap or abut (as in Abd Raman et al. 2020) and
//! agree on every overlapping column; a disagreement starts a new region marked
//! `split_from_previous`.
//!
//! **Low support (D86).** Like the paper and DiMA v5.0.9, any position may qualify. Each region
//! lists the positions in its span with support ≤ T and is *flagged* when there are any — the
//! practice of Abd Raman et al. 2020 (conserved-sequence tables with low-support positions
//! asterisked) and Hu et al. 2013 — and reports the exact 95% Clopper–Pearson lower bound of
//! its members' conservation (Clopper & Pearson 1934): a k-mer shared by 3 of 3 sequences is
//! "100% conserved" but only bounded below by 29.2%. With `eligible_only`, only positions
//! with support > T qualify and NS/LS/ELS positions end regions (the analysis set of Hu et al.
//! 2013; cf. Gblocks, Castresana 2000).

use crate::config::Percent;
use crate::diagnostics::{Code, Diagnostic, Tally, agree, plural};
use crate::error::{DimaError, Result};
use crate::model::{
    HcsConfidence, HcsEligibility, HcsMember, HcsParameters, HcsRegion, HcsResults, HcsSource,
    Motif, Reference, Results, SCHEMA_VERSION, ToolInfo,
};
use crate::numeric::clopper_pearson_lower;

/// Confidence level of the reported lower bounds.
pub const CONFIDENCE_LEVEL: f64 = 0.95;

/// Citation keys of the HCS method.
pub const REFERENCES: &[&str] = &["tharanga2025", "raman2020", "hu2013", "clopper1934"];
/// Also cited with `eligible_only`: low-support columns end blocks, as in Gblocks.
pub const ELIGIBLE_ONLY_REFERENCE: &str = "castresana2000";

/// HCS parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HcsParams {
    /// Minimum Index incidence, in (50, 100].
    pub threshold: Percent,
    /// Only positions with support above the threshold may qualify.
    pub eligible_only: bool,
}

impl Default for HcsParams {
    fn default() -> Self {
        Self {
            threshold: crate::config::DEFAULT_HCS_THRESHOLD,
            eligible_only: false,
        }
    }
}

/// Exact two-sided 95% Clopper–Pearson lower bound of `count / support`.
#[must_use]
pub fn lower_bound_95(count: u64, support: u64) -> f64 {
    clopper_pearson_lower(count, support, (1.0 - CONFIDENCE_LEVEL) / 2.0)
}

struct Open {
    start: u64,
    end: u64,
    sequence: String,
    split_from_previous: bool,
    members: Vec<HcsMember>,
}

/// Computes the conserved regions of a results set.
///
/// # Errors
/// [`DimaError::InputData`] if the results' positions are not 1, 2, … in order, or the
/// threshold is outside (50, 100].
pub fn compute(results: &Results, params: &HcsParams) -> Result<HcsResults> {
    params.threshold.check_hcs_range()?;
    let positions = &results.results;
    if positions
        .iter()
        .enumerate()
        .any(|(i, p)| p.position != i as u64 + 1)
    {
        return Err(DimaError::input_data(
            None,
            "the results' positions are not consecutive from 1",
        )
        .with_hint("compute conserved regions from a complete DiMA results file"));
    }
    let k = results.kmer_length;
    let t = results.support_threshold;
    let low = |pos: u64| positions[(pos - 1) as usize].support <= t;
    let mut regions: Vec<Open> = Vec::new();
    for p in positions {
        if params.eligible_only && p.support <= t {
            continue;
        }
        let Some(index) = p.diversity_motifs.iter().find(|v| v.motif == Motif::Index) else {
            continue;
        };
        if !params.threshold.is_met_by(index.count, p.support) {
            continue;
        }
        let member = HcsMember {
            position: p.position,
            kmer: index.sequence.clone(),
            count: index.count,
            support: p.support,
            incidence: index.incidence,
            conservation_lower_bound_95: lower_bound_95(index.count, p.support),
        };
        let mut split = false;
        if let Some(r) = regions.last_mut() {
            let last = r.members.last().map_or(r.start, |m| m.position);
            let adjacent = p.position <= r.end + 1;
            if adjacent {
                let overlap = (r.end + 1 - p.position) as usize;
                let tail = &r.sequence[r.sequence.len() - overlap..];
                let agrees = tail.as_bytes() == &member.kmer.as_bytes()[..overlap];
                let unbroken = !params.eligible_only || !(last + 1..p.position).any(low);
                if agrees && unbroken {
                    r.sequence.push_str(&member.kmer[overlap..]);
                    r.end = p.position + k - 1;
                    r.members.push(member);
                    continue;
                }
                // Set whenever an overlapping k-mer disagrees, whatever else ends the region.
                split = !agrees;
            }
        }
        regions.push(Open {
            start: p.position,
            end: p.position + k - 1,
            sequence: member.kmer.clone(),
            split_from_previous: split,
            members: vec![member],
        });
    }
    let regions: Vec<HcsRegion> = regions
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let first = r.members[0].position;
            let last = r.members.last().map_or(first, |m| m.position);
            let low_support_positions: Vec<u64> = (first..=last).filter(|&q| low(q)).collect();
            HcsRegion {
                id: i as u64 + 1,
                start_column: r.start,
                end_column: r.end,
                length: r.end - r.start + 1,
                sequence: r.sequence,
                min_incidence: r
                    .members
                    .iter()
                    .map(|m| m.incidence)
                    .fold(f64::INFINITY, f64::min),
                min_support: r.members.iter().map(|m| m.support).min().unwrap_or(0),
                conservation_lower_bound_95: r
                    .members
                    .iter()
                    .map(|m| m.conservation_lower_bound_95)
                    .fold(f64::INFINITY, f64::min),
                flagged: !low_support_positions.is_empty(),
                low_support_positions,
                split_from_previous: r.split_from_previous,
                members: r.members,
            }
        })
        .collect();
    let flagged: Vec<&HcsRegion> = regions.iter().filter(|r| r.flagged).collect();
    let mut diagnostics = Vec::new();
    if !flagged.is_empty() {
        let mut tally = Tally::default();
        for r in &flagged {
            tally.add(1, || {
                format!(
                    "region {} (columns {} to {}): min support {}, lower bound {:.1}%",
                    r.id,
                    r.start_column,
                    r.end_column,
                    r.min_support,
                    r.conservation_lower_bound_95 * 100.0
                )
            });
        }
        diagnostics.push(
            Diagnostic::new(
                Code::HcsLowSupportRegions,
                format!(
                    "{} of {} {} positions with support at or below the threshold ({t}); {} \
                     conservation rests on few sequences",
                    flagged.len(),
                    plural(
                        regions.len() as u64,
                        "conserved region",
                        "conserved regions"
                    ),
                    agree(flagged.len() as u64, "includes", "include"),
                    agree(flagged.len() as u64, "its", "their")
                ),
                tally,
            )
            .with_hint(
                "see each region's conservation_lower_bound_95, or use --hcs-eligible-only to \
                 build regions from positions with support above the threshold only",
            ),
        );
    }
    tracing::debug!(
        regions = regions.len(),
        flagged = flagged.len(),
        eligible_only = params.eligible_only,
        "conserved sequences computed"
    );
    Ok(HcsResults {
        schema_version: SCHEMA_VERSION.to_owned(),
        tool: ToolInfo {
            name: "dima".to_owned(),
            version: crate::citations::SOFTWARE_VERSION.to_owned(),
        },
        source: HcsSource {
            query_name: results.query_name.clone(),
            kmer_length: k,
            support_threshold: t,
            sequence_count: results.sequence_count,
            input_checksum: results.run.input.checksum.clone(),
            tool_version: results.run.tool.version.clone(),
            results_schema_version: results.schema_version.clone(),
        },
        parameters: HcsParameters {
            threshold_percent: params.threshold,
            merge_rule: "overlap_or_adjacent".to_owned(),
            eligibility: if params.eligible_only {
                HcsEligibility::SupportGtThreshold
            } else {
                HcsEligibility::AllPositions
            },
            confidence: HcsConfidence {
                method: "clopper_pearson".to_owned(),
                level: CONFIDENCE_LEVEL,
            },
        },
        region_count: regions.len() as u64,
        flagged_region_count: flagged.len() as u64,
        regions,
        diagnostics,
        references: REFERENCES
            .iter()
            .chain(params.eligible_only.then_some(&ELIGIBLE_ONLY_REFERENCE))
            .map(|k| Reference::of(k))
            .collect(),
    })
}
