use crate::{ComparisonFinality, TemporalAxis};

use super::{
    Episode, EpisodeComparisonError, EpisodeError, EpisodeFormationPlan, EpisodeRelation,
    EpisodeRelationKind, EpisodeSet,
};

/// Deterministic descriptive statistics for signed temporal magnitudes.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeDistributionSummary {
    count: usize,
    minimum: Option<i64>,
    mean: Option<f64>,
    median: Option<f64>,
    percentile_95: Option<i64>,
    maximum: Option<i64>,
}

impl EpisodeDistributionSummary {
    /// Returns the number of observations.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }
    /// Returns the minimum, or `None` when empty.
    #[must_use]
    pub const fn minimum(&self) -> Option<i64> {
        self.minimum
    }
    /// Returns the arithmetic mean, or `None` when empty.
    #[must_use]
    pub const fn mean(&self) -> Option<f64> {
        self.mean
    }
    /// Returns the median, or `None` when empty.
    #[must_use]
    pub const fn median(&self) -> Option<f64> {
        self.median
    }
    /// Returns the nearest-rank 95th percentile, or `None` when empty.
    #[must_use]
    pub const fn percentile_95(&self) -> Option<i64> {
        self.percentile_95
    }
    /// Returns the maximum, or `None` when empty.
    #[must_use]
    pub const fn maximum(&self) -> Option<i64> {
        self.maximum
    }
}

/// Materialized neutral measures for one episode set.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeSetSummary {
    time_axis: TemporalAxis,
    episode_count: usize,
    final_episode_count: usize,
    provisional_episode_count: usize,
    fragment_count: usize,
    multi_fragment_episode_count: usize,
    multi_fragment_episode_rate: Option<f64>,
    mean_fragments_per_episode: Option<f64>,
    maximum_fragments_per_episode: usize,
    total_active_magnitude: i64,
    total_elapsed_magnitude: i64,
    total_internal_gap_magnitude: i64,
    active_magnitude_distribution: EpisodeDistributionSummary,
    elapsed_magnitude_distribution: EpisodeDistributionSummary,
    internal_gap_magnitude_distribution: EpisodeDistributionSummary,
}

macro_rules! getter {
    ($name:ident, $field:ident, $ty:ty) => {
        #[doc = concat!("Returns `", stringify!($field), "`.")]
        #[must_use]
        pub const fn $name(&self) -> $ty {
            self.$field
        }
    };
}

impl EpisodeSetSummary {
    getter!(time_axis, time_axis, TemporalAxis);
    getter!(episode_count, episode_count, usize);
    getter!(final_episode_count, final_episode_count, usize);
    getter!(provisional_episode_count, provisional_episode_count, usize);
    getter!(fragment_count, fragment_count, usize);
    getter!(
        multi_fragment_episode_count,
        multi_fragment_episode_count,
        usize
    );
    getter!(
        multi_fragment_episode_rate,
        multi_fragment_episode_rate,
        Option<f64>
    );
    getter!(
        mean_fragments_per_episode,
        mean_fragments_per_episode,
        Option<f64>
    );
    getter!(
        maximum_fragments_per_episode,
        maximum_fragments_per_episode,
        usize
    );
    getter!(total_active_magnitude, total_active_magnitude, i64);
    getter!(total_elapsed_magnitude, total_elapsed_magnitude, i64);
    getter!(
        total_internal_gap_magnitude,
        total_internal_gap_magnitude,
        i64
    );
    /// Returns the active-magnitude distribution.
    #[must_use]
    pub const fn active_magnitude_distribution(&self) -> &EpisodeDistributionSummary {
        &self.active_magnitude_distribution
    }
    /// Returns the elapsed-magnitude distribution.
    #[must_use]
    pub const fn elapsed_magnitude_distribution(&self) -> &EpisodeDistributionSummary {
        &self.elapsed_magnitude_distribution
    }
    /// Returns the internal-gap-magnitude distribution.
    #[must_use]
    pub const fn internal_gap_magnitude_distribution(&self) -> &EpisodeDistributionSummary {
        &self.internal_gap_magnitude_distribution
    }
}

/// Materialized graph-safe neutral measures for an episode comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeComparisonSummary {
    time_axis: TemporalAxis,
    target_episode_count: usize,
    against_episode_count: usize,
    matched_target_episode_count: usize,
    matched_against_episode_count: usize,
    unmatched_target_episode_count: usize,
    unmatched_against_episode_count: usize,
    one_to_one_relation_count: usize,
    split_relation_count: usize,
    merge_relation_count: usize,
    complex_relation_count: usize,
    split_target_episode_count: usize,
    merged_against_episode_count: usize,
    episode_count_bias: i64,
    active_magnitude_bias: i64,
    target_match_rate: Option<f64>,
    against_match_rate: Option<f64>,
    split_target_rate: Option<f64>,
    merge_against_rate: Option<f64>,
    total_overlap_magnitude: i64,
    target_coverage_ratio: Option<f64>,
    against_coverage_ratio: Option<f64>,
    intersection_over_union: Option<f64>,
    onset_delta_distribution: EpisodeDistributionSummary,
    recovery_delta_distribution: EpisodeDistributionSummary,
    active_magnitude_delta_distribution: EpisodeDistributionSummary,
    elapsed_magnitude_delta_distribution: EpisodeDistributionSummary,
}

impl EpisodeComparisonSummary {
    getter!(time_axis, time_axis, TemporalAxis);
    getter!(target_episode_count, target_episode_count, usize);
    getter!(against_episode_count, against_episode_count, usize);
    getter!(
        matched_target_episode_count,
        matched_target_episode_count,
        usize
    );
    getter!(
        matched_against_episode_count,
        matched_against_episode_count,
        usize
    );
    getter!(
        unmatched_target_episode_count,
        unmatched_target_episode_count,
        usize
    );
    getter!(
        unmatched_against_episode_count,
        unmatched_against_episode_count,
        usize
    );
    getter!(one_to_one_relation_count, one_to_one_relation_count, usize);
    getter!(split_relation_count, split_relation_count, usize);
    getter!(merge_relation_count, merge_relation_count, usize);
    getter!(complex_relation_count, complex_relation_count, usize);
    getter!(
        split_target_episode_count,
        split_target_episode_count,
        usize
    );
    getter!(
        merged_against_episode_count,
        merged_against_episode_count,
        usize
    );
    getter!(episode_count_bias, episode_count_bias, i64);
    getter!(active_magnitude_bias, active_magnitude_bias, i64);
    getter!(target_match_rate, target_match_rate, Option<f64>);
    getter!(against_match_rate, against_match_rate, Option<f64>);
    getter!(split_target_rate, split_target_rate, Option<f64>);
    getter!(merge_against_rate, merge_against_rate, Option<f64>);
    getter!(total_overlap_magnitude, total_overlap_magnitude, i64);
    getter!(target_coverage_ratio, target_coverage_ratio, Option<f64>);
    getter!(against_coverage_ratio, against_coverage_ratio, Option<f64>);
    getter!(
        intersection_over_union,
        intersection_over_union,
        Option<f64>
    );
    /// Returns one-to-one onset deltas.
    #[must_use]
    pub const fn onset_delta_distribution(&self) -> &EpisodeDistributionSummary {
        &self.onset_delta_distribution
    }
    /// Returns one-to-one recovery deltas.
    #[must_use]
    pub const fn recovery_delta_distribution(&self) -> &EpisodeDistributionSummary {
        &self.recovery_delta_distribution
    }
    /// Returns one-to-one active-magnitude deltas.
    #[must_use]
    pub const fn active_magnitude_delta_distribution(&self) -> &EpisodeDistributionSummary {
        &self.active_magnitude_delta_distribution
    }
    /// Returns one-to-one elapsed-magnitude deltas.
    #[must_use]
    pub const fn elapsed_magnitude_delta_distribution(&self) -> &EpisodeDistributionSummary {
        &self.elapsed_magnitude_delta_distribution
    }
}

/// Explicit directional interpretation of target episodes as references.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeReferenceScorecard {
    pub(crate) reference_episode_count: usize,
    pub(crate) detected_reference_episode_count: usize,
    pub(crate) missed_reference_episode_count: usize,
    pub(crate) detection_episode_count: usize,
    pub(crate) matched_detection_episode_count: usize,
    pub(crate) unexpected_detection_episode_count: usize,
    pub(crate) recall: Option<f64>,
    pub(crate) precision: Option<f64>,
    pub(crate) f1_score: Option<f64>,
}

impl EpisodeReferenceScorecard {
    getter!(reference_episode_count, reference_episode_count, usize);
    getter!(
        detected_reference_episode_count,
        detected_reference_episode_count,
        usize
    );
    getter!(
        missed_reference_episode_count,
        missed_reference_episode_count,
        usize
    );
    getter!(detection_episode_count, detection_episode_count, usize);
    getter!(
        matched_detection_episode_count,
        matched_detection_episode_count,
        usize
    );
    getter!(
        unexpected_detection_episode_count,
        unexpected_detection_episode_count,
        usize
    );
    getter!(recall, recall, Option<f64>);
    getter!(precision, precision, Option<f64>);
    getter!(f1_score, f1_score, Option<f64>);
}

pub(crate) fn summarize_set(
    plan: &EpisodeFormationPlan,
    episodes: &[Episode],
) -> Result<EpisodeSetSummary, EpisodeError> {
    let mut final_count = 0usize;
    let mut provisional_count = 0usize;
    let mut fragment_count = 0usize;
    let mut multi_fragment_count = 0usize;
    let mut maximum_fragments = 0usize;
    let mut total_active = 0i64;
    let mut total_elapsed = 0i64;
    let mut total_gap = 0i64;
    let mut active = Vec::with_capacity(episodes.len());
    let mut elapsed = Vec::with_capacity(episodes.len());
    let mut gaps = Vec::with_capacity(episodes.len());
    for episode in episodes {
        debug_assert_eq!(episode.time_axis(), plan.formation().time_axis());
        match episode.finality() {
            &ComparisonFinality::Final => {
                final_count = final_count
                    .checked_add(1)
                    .ok_or(EpisodeError::MagnitudeOverflow)?
            }
            _ => {
                provisional_count = provisional_count
                    .checked_add(1)
                    .ok_or(EpisodeError::MagnitudeOverflow)?
            }
        }
        let fragments = episode.fragments().len();
        fragment_count = fragment_count
            .checked_add(fragments)
            .ok_or(EpisodeError::MagnitudeOverflow)?;
        if fragments > 1 {
            multi_fragment_count = multi_fragment_count
                .checked_add(1)
                .ok_or(EpisodeError::MagnitudeOverflow)?;
        }
        maximum_fragments = maximum_fragments.max(fragments);
        total_active = total_active
            .checked_add(episode.active_magnitude())
            .ok_or(EpisodeError::MagnitudeOverflow)?;
        total_elapsed = total_elapsed
            .checked_add(episode.elapsed_magnitude())
            .ok_or(EpisodeError::MagnitudeOverflow)?;
        total_gap = total_gap
            .checked_add(episode.internal_gap_magnitude())
            .ok_or(EpisodeError::MagnitudeOverflow)?;
        active.push(episode.active_magnitude());
        elapsed.push(episode.elapsed_magnitude());
        gaps.push(episode.internal_gap_magnitude());
    }
    Ok(EpisodeSetSummary {
        time_axis: plan.formation().time_axis(),
        episode_count: episodes.len(),
        final_episode_count: final_count,
        provisional_episode_count: provisional_count,
        fragment_count,
        multi_fragment_episode_count: multi_fragment_count,
        multi_fragment_episode_rate: rate(multi_fragment_count, episodes.len()),
        mean_fragments_per_episode: (!episodes.is_empty())
            .then(|| fragment_count as f64 / episodes.len() as f64),
        maximum_fragments_per_episode: maximum_fragments,
        total_active_magnitude: total_active,
        total_elapsed_magnitude: total_elapsed,
        total_internal_gap_magnitude: total_gap,
        active_magnitude_distribution: describe(&active),
        elapsed_magnitude_distribution: describe(&elapsed),
        internal_gap_magnitude_distribution: describe(&gaps),
    })
}

pub(crate) fn summarize_comparison(
    target: &EpisodeSet,
    against: &EpisodeSet,
    relations: &[EpisodeRelation],
) -> Result<EpisodeComparisonSummary, EpisodeComparisonError> {
    let mut matched_target = 0usize;
    let mut matched_against = 0usize;
    let mut unmatched_target = 0usize;
    let mut unmatched_against = 0usize;
    let mut one = 0usize;
    let mut split = 0usize;
    let mut merge = 0usize;
    let mut complex = 0usize;
    let mut split_targets = 0usize;
    let mut merged_against = 0usize;
    let mut overlap = 0i64;
    let mut target_coverage = 0i64;
    let mut against_coverage = 0i64;
    let mut onset = Vec::new();
    let mut recovery = Vec::new();
    let mut active_delta = Vec::new();
    let mut elapsed_delta = Vec::new();
    for relation in relations {
        if !relation.target_episodes().is_empty() && !relation.against_episodes().is_empty() {
            matched_target = matched_target
                .checked_add(relation.target_episodes().len())
                .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
            matched_against = matched_against
                .checked_add(relation.against_episodes().len())
                .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
        }
        match relation.kind() {
            EpisodeRelationKind::OneToOne => {
                one = one
                    .checked_add(1)
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
                if let Some(v) = relation.metrics().onset_delta_magnitude() {
                    onset.push(v)
                }
                if let Some(v) = relation.metrics().recovery_delta_magnitude() {
                    recovery.push(v)
                }
                if let Some(v) = relation.metrics().active_magnitude_delta() {
                    active_delta.push(v)
                }
                if let Some(v) = relation.metrics().elapsed_magnitude_delta() {
                    elapsed_delta.push(v)
                }
            }
            EpisodeRelationKind::Split => {
                split = split
                    .checked_add(1)
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
                split_targets = split_targets
                    .checked_add(relation.target_episodes().len())
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
            }
            EpisodeRelationKind::Merge => {
                merge = merge
                    .checked_add(1)
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
                merged_against = merged_against
                    .checked_add(relation.against_episodes().len())
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
            }
            EpisodeRelationKind::Complex => {
                complex = complex
                    .checked_add(1)
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
            }
            EpisodeRelationKind::UnmatchedTarget => {
                unmatched_target = unmatched_target
                    .checked_add(relation.target_episodes().len())
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?
            }
            EpisodeRelationKind::UnmatchedAgainst => {
                unmatched_against = unmatched_against
                    .checked_add(relation.against_episodes().len())
                    .ok_or(EpisodeComparisonError::MagnitudeOverflow)?
            }
        }
        overlap = overlap
            .checked_add(relation.metrics().overlap_magnitude())
            .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
        target_coverage = target_coverage
            .checked_add(relation.metrics().target_active_magnitude())
            .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
        against_coverage = against_coverage
            .checked_add(relation.metrics().against_active_magnitude())
            .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
    }
    let union = target_coverage as f64 + against_coverage as f64 - overlap as f64;
    Ok(EpisodeComparisonSummary {
        time_axis: target.summary().time_axis(),
        target_episode_count: target.episodes().len(),
        against_episode_count: against.episodes().len(),
        matched_target_episode_count: matched_target,
        matched_against_episode_count: matched_against,
        unmatched_target_episode_count: unmatched_target,
        unmatched_against_episode_count: unmatched_against,
        one_to_one_relation_count: one,
        split_relation_count: split,
        merge_relation_count: merge,
        complex_relation_count: complex,
        split_target_episode_count: split_targets,
        merged_against_episode_count: merged_against,
        episode_count_bias: signed_bias(against.episodes().len(), target.episodes().len()),
        active_magnitude_bias: against
            .summary()
            .total_active_magnitude()
            .saturating_sub(target.summary().total_active_magnitude()),
        target_match_rate: rate(matched_target, target.episodes().len()),
        against_match_rate: rate(matched_against, against.episodes().len()),
        split_target_rate: rate(split_targets, target.episodes().len()),
        merge_against_rate: rate(merged_against, against.episodes().len()),
        total_overlap_magnitude: overlap,
        target_coverage_ratio: ratio(overlap, target_coverage),
        against_coverage_ratio: ratio(overlap, against_coverage),
        intersection_over_union: (union != 0.0).then(|| overlap as f64 / union),
        onset_delta_distribution: describe(&onset),
        recovery_delta_distribution: describe(&recovery),
        active_magnitude_delta_distribution: describe(&active_delta),
        elapsed_magnitude_delta_distribution: describe(&elapsed_delta),
    })
}

pub(crate) fn describe(values: &[i64]) -> EpisodeDistributionSummary {
    if values.is_empty() {
        return EpisodeDistributionSummary {
            count: 0,
            minimum: None,
            mean: None,
            median: None,
            percentile_95: None,
            maximum: None,
        };
    }
    let mut ordered = values.to_vec();
    ordered.sort_unstable();
    let mean = ordered
        .iter()
        .enumerate()
        .fold(0.0, |mean, (index, value)| {
            mean + (*value as f64 - mean) / (index + 1) as f64
        });
    let middle = ordered.len() / 2;
    let median = if ordered.len() % 2 == 1 {
        ordered[middle] as f64
    } else {
        ordered[middle - 1] as f64 / 2.0 + ordered[middle] as f64 / 2.0
    };
    let p95 = ordered.len() - ordered.len() / 20 - 1;
    EpisodeDistributionSummary {
        count: ordered.len(),
        minimum: ordered.first().copied(),
        mean: Some(mean),
        median: Some(median),
        percentile_95: Some(ordered[p95]),
        maximum: ordered.last().copied(),
    }
}

fn rate(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}
fn ratio(numerator: i64, denominator: i64) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}
fn signed_bias(against: usize, target: usize) -> i64 {
    if against >= target {
        i64::try_from(against - target).unwrap_or(i64::MAX)
    } else {
        i64::try_from(target - against).map_or(i64::MIN, std::ops::Neg::neg)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ClosedWindow, ComparisonScope, ComparisonSelector, TemporalAxis, TemporalRange,
        TemporalTolerance, WindowHistory, WindowRecordId,
    };

    use super::describe;

    fn closed(id: &str, start: i64, end: i64, source: &str, key: &str) -> ClosedWindow {
        ClosedWindow {
            id: WindowRecordId::new(id).unwrap(),
            window_name: "State".to_owned(),
            key: key.to_owned(),
            range: TemporalRange::positions(start, end).unwrap(),
            known_at: None,
            source: Some(source.to_owned()),
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        }
    }

    fn compare(records: Vec<ClosedWindow>) -> crate::EpisodeComparisonResult {
        let history = WindowHistory::from_records(records, []).unwrap();
        history
            .compare_episodes("comparison")
            .target("target", ComparisonSelector::for_source("target"))
            .against("against", ComparisonSelector::for_source("against"))
            .scope(ComparisonScope::window("State"))
            .run()
            .unwrap()
    }

    #[test]
    fn set_summary_materializes_empty_and_nonempty_null_zero_rules() {
        let empty_history = WindowHistory::from_records([], []).unwrap();
        let empty = empty_history
            .form_episodes("empty")
            .from(ComparisonSelector::for_source("target"))
            .scope(ComparisonScope::window("State"))
            .run()
            .unwrap();
        assert_eq!(
            empty.summary().time_axis(),
            TemporalAxis::ProcessingPosition
        );
        assert_eq!(empty.summary().episode_count(), 0);
        assert_eq!(empty.summary().multi_fragment_episode_rate(), None);
        assert_eq!(empty.summary().mean_fragments_per_episode(), None);
        assert_eq!(
            empty.summary().active_magnitude_distribution().median(),
            None
        );

        let history = WindowHistory::from_records(
            [
                closed("t1", 0, 2, "target", "device"),
                closed("t2", 4, 6, "target", "device"),
            ],
            [],
        )
        .unwrap();
        let set = history
            .form_episodes("set")
            .from(ComparisonSelector::for_source("target"))
            .scope(ComparisonScope::window("State"))
            .stitch_gaps_up_to(TemporalTolerance::processing_positions(2).unwrap())
            .run()
            .unwrap();
        assert_eq!(set.summary().episode_count(), 1);
        assert_eq!(set.summary().final_episode_count(), 1);
        assert_eq!(set.summary().fragment_count(), 2);
        assert_eq!(set.summary().multi_fragment_episode_rate(), Some(1.0));
        assert_eq!(set.summary().total_active_magnitude(), 4);
        assert_eq!(set.summary().total_elapsed_magnitude(), 6);
        assert_eq!(set.summary().total_internal_gap_magnitude(), 2);
    }

    #[test]
    fn live_set_summary_counts_final_and_provisional_episodes() {
        let history = WindowHistory::from_records(
            [
                closed("settled", 0, 2, "target", "settled"),
                closed("unsettled", 7, 9, "target", "unsettled"),
            ],
            [],
        )
        .unwrap();
        let set = history
            .form_episodes("live set")
            .from(ComparisonSelector::for_source("target"))
            .scope(ComparisonScope::window("State"))
            .stitch_gaps_up_to(TemporalTolerance::processing_positions(2).unwrap())
            .run_live(crate::TemporalPoint::position(10))
            .unwrap();

        assert_eq!(set.summary().episode_count(), 2);
        assert_eq!(set.summary().final_episode_count(), 1);
        assert_eq!(set.summary().provisional_episode_count(), 1);
    }

    #[test]
    fn distribution_handles_odd_even_extremes_and_nearest_rank_p95() {
        let odd = describe(&[-2, 0, 4, 8, 100]);
        let even = describe(&[i64::MIN, i64::MAX]);
        let twenty = describe(&(1..=20).collect::<Vec<_>>());
        assert_eq!(
            (odd.count(), odd.mean(), odd.median()),
            (5, Some(22.0), Some(4.0))
        );
        assert_eq!(odd.percentile_95(), Some(100));
        assert_eq!((even.mean(), even.median()), (Some(0.0), Some(0.0)));
        assert_eq!(even.percentile_95(), Some(i64::MAX));
        assert_eq!(twenty.percentile_95(), Some(19));
    }

    #[test]
    fn comparison_counts_component_members_once_and_only_distributes_one_to_one_deltas() {
        let result = compare(vec![
            closed("t1", 0, 5, "target", "one"),
            closed("a1", 1, 4, "against", "one"),
            closed("t2", 0, 10, "target", "split"),
            closed("a2", 0, 4, "against", "split"),
            closed("a3", 6, 10, "against", "split"),
            closed("t3", 0, 4, "target", "merge"),
            closed("t4", 6, 10, "target", "merge"),
            closed("a4", 0, 10, "against", "merge"),
            closed("t5", 0, 4, "target", "complex"),
            closed("t6", 6, 10, "target", "complex"),
            closed("a5", 0, 6, "against", "complex"),
            closed("a6", 7, 10, "against", "complex"),
            closed("t7", 0, 2, "target", "unmatched-target"),
            closed("a7", 0, 2, "against", "unmatched-against"),
        ]);
        let summary = result.summary();
        assert_eq!(
            (
                summary.target_episode_count(),
                summary.against_episode_count()
            ),
            (7, 7)
        );
        assert_eq!(
            (
                summary.matched_target_episode_count(),
                summary.matched_against_episode_count()
            ),
            (6, 6)
        );
        assert_eq!(
            (
                summary.unmatched_target_episode_count(),
                summary.unmatched_against_episode_count()
            ),
            (1, 1)
        );
        assert_eq!(
            (
                summary.one_to_one_relation_count(),
                summary.split_relation_count(),
                summary.merge_relation_count(),
                summary.complex_relation_count()
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(summary.onset_delta_distribution().count(), 1);
        assert_eq!(summary.recovery_delta_distribution().count(), 1);
    }

    #[test]
    fn signed_delta_distributions_are_exact_and_exclude_split_components() {
        let result = compare(vec![
            closed("t1", 2, 6, "target", "early-against"),
            closed("a1", 0, 4, "against", "early-against"),
            closed("t2", 10, 15, "target", "late-against"),
            closed("a2", 13, 18, "against", "late-against"),
            closed("t3", 20, 30, "target", "split"),
            closed("a3", 20, 24, "against", "split"),
            closed("a4", 26, 30, "against", "split"),
        ]);
        let summary = result.summary();
        let onset = summary.onset_delta_distribution();
        let recovery = summary.recovery_delta_distribution();

        assert_eq!(summary.one_to_one_relation_count(), 2);
        assert_eq!(summary.split_relation_count(), 1);
        assert_eq!(
            (
                onset.count(),
                onset.minimum(),
                onset.mean(),
                onset.median(),
                onset.percentile_95(),
                onset.maximum()
            ),
            (2, Some(-2), Some(0.5), Some(0.5), Some(3), Some(3))
        );
        assert_eq!(
            (
                recovery.count(),
                recovery.minimum(),
                recovery.mean(),
                recovery.median(),
                recovery.percentile_95(),
                recovery.maximum()
            ),
            (2, Some(-2), Some(0.5), Some(0.5), Some(3), Some(3))
        );
        assert_eq!(
            (
                summary.active_magnitude_delta_distribution().count(),
                summary.active_magnitude_delta_distribution().minimum(),
                summary.active_magnitude_delta_distribution().maximum()
            ),
            (2, Some(0), Some(0))
        );
        assert_eq!(
            (
                summary.elapsed_magnitude_delta_distribution().count(),
                summary.elapsed_magnitude_delta_distribution().minimum(),
                summary.elapsed_magnitude_delta_distribution().maximum()
            ),
            (2, Some(0), Some(0))
        );
    }

    #[test]
    fn empty_and_disconnected_comparisons_distinguish_null_from_zero() {
        let empty = compare(Vec::new());
        assert_eq!(empty.summary().target_match_rate(), None);
        assert_eq!(empty.summary().target_coverage_ratio(), None);
        assert_eq!(empty.summary().intersection_over_union(), None);

        let disconnected = compare(vec![
            closed("t", 0, 2, "target", "target-key"),
            closed("a", 5, 7, "against", "against-key"),
        ]);
        assert_eq!(disconnected.summary().target_match_rate(), Some(0.0));
        assert_eq!(disconnected.summary().against_match_rate(), Some(0.0));
        assert_eq!(disconnected.summary().target_coverage_ratio(), Some(0.0));
        assert_eq!(disconnected.summary().intersection_over_union(), Some(0.0));
    }

    #[test]
    fn zero_magnitude_comparison_has_null_coverage_and_intersection_over_union() {
        let result = compare(vec![
            closed("t", 4, 4, "target", "device"),
            closed("a", 4, 4, "against", "device"),
        ]);

        assert_eq!(result.summary().total_overlap_magnitude(), 0);
        assert_eq!(result.summary().target_coverage_ratio(), None);
        assert_eq!(result.summary().against_coverage_ratio(), None);
        assert_eq!(result.summary().intersection_over_union(), None);
    }

    #[test]
    fn comparison_coverage_and_bias_use_component_unions_and_set_totals() {
        let result = compare(vec![
            closed("t", 0, 8, "target", "device"),
            closed("a", 2, 6, "against", "device"),
        ]);
        let summary = result.summary();
        assert_eq!(summary.episode_count_bias(), 0);
        assert_eq!(summary.active_magnitude_bias(), -4);
        assert_eq!(summary.total_overlap_magnitude(), 4);
        assert_eq!(summary.target_coverage_ratio(), Some(0.5));
        assert_eq!(summary.against_coverage_ratio(), Some(1.0));
        assert_eq!(summary.intersection_over_union(), Some(0.5));
        assert_eq!(summary.target_match_rate(), Some(1.0));
        assert_eq!(summary.against_match_rate(), Some(1.0));
    }

    #[test]
    fn multi_fragment_episode_is_not_a_split_component() {
        let history = WindowHistory::from_records(
            [
                closed("t", 0, 10, "target", "device"),
                closed("a1", 0, 4, "against", "device"),
                closed("a2", 6, 10, "against", "device"),
            ],
            [],
        )
        .unwrap();
        let result = history
            .compare_episodes("comparison")
            .target("target", ComparisonSelector::for_source("target"))
            .against("against", ComparisonSelector::for_source("against"))
            .scope(ComparisonScope::window("State"))
            .stitch_gaps_up_to(TemporalTolerance::processing_positions(2).unwrap())
            .run()
            .unwrap();

        assert_eq!(
            result
                .against_episodes()
                .summary()
                .multi_fragment_episode_count(),
            1
        );
        assert_eq!(result.summary().one_to_one_relation_count(), 1);
        assert_eq!(result.summary().split_relation_count(), 0);
        assert_eq!(result.summary().split_target_rate(), Some(0.0));
    }
}
