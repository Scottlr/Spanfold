use std::{cmp::Ordering, collections::BTreeSet};

use crate::{ComparisonFinality, TemporalAxis, TemporalPoint, WindowHistory};

use super::{
    Episode, EpisodeComparisonError, EpisodeComparisonPlan, EpisodeComparisonResult, EpisodeError,
    EpisodeFormationPlan, EpisodeRelation, EpisodeRelationKind, EpisodeSet, formation, metrics,
};

pub(crate) fn run(
    history: &WindowHistory,
    plan: EpisodeComparisonPlan,
) -> Result<EpisodeComparisonResult, EpisodeComparisonError> {
    let target_plan = formation_plan(&plan, true);
    let against_plan = formation_plan(&plan, false);
    let target_set = formation::run(history, target_plan).map_err(comparison_formation_error)?;
    let against_set = formation::run(history, against_plan).map_err(comparison_formation_error)?;
    ensure_disjoint(&target_set, &against_set, &plan)?;

    let mut target_edges = vec![Vec::new(); target_set.episodes().len()];
    let mut against_edges = vec![Vec::new(); against_set.episodes().len()];
    for (target_index, target) in target_set.episodes().iter().enumerate() {
        for (against_index, against) in against_set.episodes().iter().enumerate() {
            if compatible(target, against)
                && fragments_relate(target, against, plan.relation.tolerance().magnitude())
            {
                target_edges[target_index].push(against_index);
                against_edges[against_index].push(target_index);
            }
        }
    }

    let horizon = target_set.evaluation_horizon().cloned();
    let relations = components(
        &target_set,
        &against_set,
        &target_edges,
        &against_edges,
        plan.relation.time_axis(),
        plan.relation.tolerance().magnitude(),
        horizon.as_ref(),
    )?;
    let summary = super::summary::summarize_comparison(&target_set, &against_set, &relations)?;
    Ok(EpisodeComparisonResult {
        plan,
        target_episodes: target_set,
        against_episodes: against_set,
        relations,
        summary,
        evaluation_horizon: horizon,
    })
}

fn comparison_formation_error(error: EpisodeError) -> EpisodeComparisonError {
    match error {
        EpisodeError::MagnitudeOverflow => EpisodeComparisonError::MagnitudeOverflow,
        error => error.into(),
    }
}

fn formation_plan(plan: &EpisodeComparisonPlan, target: bool) -> EpisodeFormationPlan {
    let (name, selector) = if target {
        (&plan.target_name, &plan.target)
    } else {
        (&plan.against_name, &plan.against)
    };
    EpisodeFormationPlan {
        name: name.clone(),
        selector: selector.clone().with_name(name),
        scope: plan.scope.clone(),
        normalization: plan.normalization.clone(),
        formation: plan.formation.clone(),
    }
}

fn ensure_disjoint(
    target: &EpisodeSet,
    against: &EpisodeSet,
    plan: &EpisodeComparisonPlan,
) -> Result<(), EpisodeComparisonError> {
    let target_ids = target
        .episodes()
        .iter()
        .flat_map(Episode::fragments)
        .map(|fragment| fragment.record_id())
        .collect::<BTreeSet<_>>();
    if let Some(record_id) = against
        .episodes()
        .iter()
        .flat_map(Episode::fragments)
        .map(|fragment| fragment.record_id())
        .find(|record_id| target_ids.contains(record_id))
    {
        return Err(EpisodeComparisonError::SelfMembership {
            record_id: record_id.to_owned(),
            target_name: plan.target_name.clone(),
            against_name: plan.against_name.clone(),
        });
    }
    Ok(())
}

fn compatible(target: &Episode, against: &Episode) -> bool {
    target.window_name() == against.window_name()
        && target.key() == against.key()
        && target.partition() == against.partition()
        && target.time_axis() == against.time_axis()
        && (target.time_axis() != TemporalAxis::Timestamp
            || target.envelope().start().clock() == against.envelope().start().clock())
}

fn fragments_relate(target: &Episode, against: &Episode, tolerance: i64) -> bool {
    target.fragments().iter().any(|target_fragment| {
        against.fragments().iter().any(|against_fragment| {
            metrics::fragment_gap(target_fragment.range(), against_fragment.range())
                <= i128::from(tolerance)
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn components(
    target_set: &EpisodeSet,
    against_set: &EpisodeSet,
    target_edges: &[Vec<usize>],
    against_edges: &[Vec<usize>],
    axis: TemporalAxis,
    tolerance: i64,
    horizon: Option<&TemporalPoint>,
) -> Result<Vec<EpisodeRelation>, EpisodeComparisonError> {
    let mut visited_targets = vec![false; target_set.episodes().len()];
    let mut visited_against = vec![false; against_set.episodes().len()];
    let mut relations = Vec::new();
    for target_index in 0..target_set.episodes().len() {
        if !visited_targets[target_index] {
            relations.push(traverse(
                Some((true, target_index)),
                target_set,
                against_set,
                target_edges,
                against_edges,
                &mut visited_targets,
                &mut visited_against,
                axis,
                tolerance,
                horizon,
            )?);
        }
    }
    for against_index in 0..against_set.episodes().len() {
        if !visited_against[against_index] {
            relations.push(traverse(
                Some((false, against_index)),
                target_set,
                against_set,
                target_edges,
                against_edges,
                &mut visited_targets,
                &mut visited_against,
                axis,
                tolerance,
                horizon,
            )?);
        }
    }
    relations.sort_by(compare_relations);
    Ok(relations)
}

#[allow(clippy::too_many_arguments)]
fn traverse(
    start: Option<(bool, usize)>,
    target_set: &EpisodeSet,
    against_set: &EpisodeSet,
    target_edges: &[Vec<usize>],
    against_edges: &[Vec<usize>],
    visited_targets: &mut [bool],
    visited_against: &mut [bool],
    axis: TemporalAxis,
    tolerance: i64,
    horizon: Option<&TemporalPoint>,
) -> Result<EpisodeRelation, EpisodeComparisonError> {
    let mut stack = vec![start.expect("component traversal always has a start")];
    let (mut targets, mut against) = (Vec::new(), Vec::new());
    while let Some((is_target, index)) = stack.pop() {
        if is_target {
            if visited_targets[index] {
                continue;
            }
            visited_targets[index] = true;
            targets.push(target_set.episodes()[index].clone());
            for &neighbor in target_edges[index].iter().rev() {
                if !visited_against[neighbor] {
                    stack.push((false, neighbor));
                }
            }
        } else {
            if visited_against[index] {
                continue;
            }
            visited_against[index] = true;
            against.push(against_set.episodes()[index].clone());
            for &neighbor in against_edges[index].iter().rev() {
                if !visited_targets[neighbor] {
                    stack.push((true, neighbor));
                }
            }
        }
    }
    targets.sort_by(compare_episodes);
    against.sort_by(compare_episodes);
    let relation_metrics = metrics::calculate(&targets, &against, axis)?;
    let finality = finality(&targets, &against, tolerance, horizon);
    Ok(EpisodeRelation {
        kind: classify(targets.len(), against.len()),
        target_episodes: targets,
        against_episodes: against,
        metrics: relation_metrics,
        finality,
    })
}

fn classify(target_count: usize, against_count: usize) -> EpisodeRelationKind {
    match (target_count, against_count) {
        (1, 0) => EpisodeRelationKind::UnmatchedTarget,
        (0, 1) => EpisodeRelationKind::UnmatchedAgainst,
        (1, 1) => EpisodeRelationKind::OneToOne,
        (1, 2..) => EpisodeRelationKind::Split,
        (2.., 1) => EpisodeRelationKind::Merge,
        (2.., 2..) => EpisodeRelationKind::Complex,
        _ => unreachable!("graph components are non-empty and bipartite"),
    }
}

fn finality(
    targets: &[Episode],
    against: &[Episode],
    tolerance: i64,
    horizon: Option<&TemporalPoint>,
) -> ComparisonFinality {
    let episodes = targets.iter().chain(against);
    if episodes
        .clone()
        .any(|episode| episode.finality() == &ComparisonFinality::Provisional)
    {
        return ComparisonFinality::Provisional;
    }
    let Some(horizon) = horizon else {
        return ComparisonFinality::Final;
    };
    let latest_end = episodes
        .flat_map(Episode::fragments)
        .map(|fragment| fragment.range().end().magnitude())
        .max()
        .expect("component contains at least one episode");
    if horizon.magnitude() <= latest_end.saturating_add(tolerance) {
        ComparisonFinality::Provisional
    } else {
        ComparisonFinality::Final
    }
}

fn compare_relations(left: &EpisodeRelation, right: &EpisodeRelation) -> Ordering {
    first_episode(left)
        .cmp_by(first_episode(right))
        .then_with(|| left.kind.cmp(&right.kind))
        .then_with(|| left.target_episodes.len().cmp(&right.target_episodes.len()))
        .then_with(|| {
            left.against_episodes
                .len()
                .cmp(&right.against_episodes.len())
        })
}

fn first_episode(relation: &EpisodeRelation) -> EpisodeOrder<'_> {
    match (
        relation.target_episodes.first(),
        relation.against_episodes.first(),
    ) {
        (Some(target), Some(against)) => {
            let target = EpisodeOrder(target);
            let against = EpisodeOrder(against);
            target.min(against)
        }
        (Some(target), None) => EpisodeOrder(target),
        (None, Some(against)) => EpisodeOrder(against),
        (None, None) => unreachable!("relation components are non-empty"),
    }
}

fn compare_episodes(left: &Episode, right: &Episode) -> Ordering {
    EpisodeOrder(left).cmp(&EpisodeOrder(right))
}

#[derive(Clone, Copy)]
struct EpisodeOrder<'a>(&'a Episode);

impl EpisodeOrder<'_> {
    fn cmp_by(self, other: Self) -> Ordering {
        self.cmp(&other)
    }
}

impl PartialEq for EpisodeOrder<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for EpisodeOrder<'_> {}
impl PartialOrd for EpisodeOrder<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for EpisodeOrder<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self.0;
        let right = other.0;
        left.window_name()
            .cmp(right.window_name())
            .then_with(|| left.key().cmp(right.key()))
            .then_with(|| left.partition().cmp(&right.partition()))
            .then_with(|| left.time_axis().cmp(&right.time_axis()))
            .then_with(|| {
                left.envelope()
                    .start()
                    .clock()
                    .cmp(&right.envelope().start().clock())
            })
            .then_with(|| {
                left.envelope()
                    .start()
                    .magnitude()
                    .cmp(&right.envelope().start().magnitude())
            })
            .then_with(|| {
                left.envelope()
                    .end()
                    .magnitude()
                    .cmp(&right.envelope().end().magnitude())
            })
            .then_with(|| left.source().cmp(&right.source()))
            .then_with(|| left.id().cmp(right.id()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ClosedWindow, ComparisonFinality, ComparisonNormalizationPolicy, ComparisonScope,
        ComparisonSelector, EpisodeComparisonBuilder, EpisodeComparisonError, EpisodeRelationKind,
        TemporalPoint, TemporalRange, TemporalTolerance, WindowHistory, WindowRecordId,
    };

    fn closed(id: &str, start: i64, end: i64, source: &str) -> ClosedWindow {
        ClosedWindow {
            id: WindowRecordId::new(id).unwrap(),
            window_name: "State".to_owned(),
            key: "device-1".to_owned(),
            range: TemporalRange::positions(start, end).unwrap(),
            known_at: None,
            source: Some(source.to_owned()),
            partition: Some("north".to_owned()),
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        }
    }

    fn builder(history: &WindowHistory) -> EpisodeComparisonBuilder<'_> {
        history
            .compare_episodes("State comparison")
            .target("target", ComparisonSelector::for_source("target"))
            .against("against", ComparisonSelector::for_source("against"))
            .scope(ComparisonScope::window("State"))
    }

    fn timestamp_closed(id: &str, source: &str, clock: &str) -> ClosedWindow {
        let start = TemporalPoint::timestamp_ticks_with_clock(0, clock);
        let end = TemporalPoint::timestamp_ticks_with_clock(5, clock);
        ClosedWindow {
            id: WindowRecordId::new(id).unwrap(),
            window_name: "State".to_owned(),
            key: "device-1".to_owned(),
            range: TemporalRange::new(start, end).unwrap(),
            known_at: None,
            source: Some(source.to_owned()),
            partition: Some("north".to_owned()),
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        }
    }

    fn compare(
        records: impl IntoIterator<Item = ClosedWindow>,
    ) -> (WindowHistory, Vec<ClosedWindow>) {
        let records = records.into_iter().collect::<Vec<_>>();
        let history = WindowHistory::from_records(records.clone(), []).unwrap();
        (history, records)
    }

    #[test]
    fn classifies_all_directional_component_kinds() {
        let scenarios = [
            (
                vec![closed("t1", 0, 5, "target"), closed("a1", 1, 4, "against")],
                EpisodeRelationKind::OneToOne,
            ),
            (
                vec![
                    closed("t2", 0, 10, "target"),
                    closed("a2", 0, 4, "against"),
                    closed("a3", 6, 10, "against"),
                ],
                EpisodeRelationKind::Split,
            ),
            (
                vec![
                    closed("t3", 0, 4, "target"),
                    closed("t4", 6, 10, "target"),
                    closed("a4", 0, 10, "against"),
                ],
                EpisodeRelationKind::Merge,
            ),
            (
                vec![
                    closed("t5", 0, 4, "target"),
                    closed("t6", 6, 10, "target"),
                    closed("a5", 0, 6, "against"),
                    closed("a6", 7, 10, "against"),
                ],
                EpisodeRelationKind::Complex,
            ),
        ];

        for (records, expected) in scenarios {
            let (history, _) = compare(records);
            let result = builder(&history).run().unwrap();
            assert_eq!(result.relations().len(), 1);
            assert_eq!(result.relations()[0].kind(), expected);
        }

        let (history, _) = compare([closed("t7", 0, 2, "target"), closed("a7", 5, 7, "against")]);
        let kinds = builder(&history)
            .run()
            .unwrap()
            .relations()
            .iter()
            .map(|relation| relation.kind())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                EpisodeRelationKind::UnmatchedTarget,
                EpisodeRelationKind::UnmatchedAgainst,
            ]
        );
    }

    #[test]
    fn exhaustive_graph_keeps_chained_many_to_many_component() {
        let (history, _) = compare([
            closed("t1", 0, 4, "target"),
            closed("t2", 6, 10, "target"),
            closed("a1", 0, 6, "against"),
            closed("a2", 7, 10, "against"),
        ]);

        let result = builder(&history).run().unwrap();
        let relation = &result.relations()[0];
        assert_eq!(result.relations().len(), 1);
        assert_eq!(relation.kind(), EpisodeRelationKind::Complex);
        assert_eq!(relation.target_episodes().len(), 2);
        assert_eq!(relation.against_episodes().len(), 2);
    }

    #[test]
    fn actual_fragments_enforce_exact_tolerance_without_envelope_edges() {
        let (history, _) = compare([
            closed("t1", 0, 2, "target"),
            closed("t2", 8, 10, "target"),
            closed("a1", 4, 6, "against"),
        ]);
        let stitched = builder(&history)
            .stitch_gaps_up_to(TemporalTolerance::processing_positions(6).unwrap());

        let outside = stitched
            .clone()
            .relate_within(TemporalTolerance::processing_positions(1).unwrap())
            .run()
            .unwrap();
        let exact = stitched
            .relate_within(TemporalTolerance::processing_positions(2).unwrap())
            .run()
            .unwrap();

        assert_eq!(outside.target_episodes().episodes().len(), 1);
        assert_eq!(outside.relations().len(), 2);
        assert_eq!(exact.relations().len(), 1);
        assert_eq!(
            exact.relations()[0].metrics().minimum_gap_magnitude(),
            Some(2)
        );
    }

    #[test]
    fn timestamp_clock_is_a_hard_relation_boundary() {
        let history = WindowHistory::from_records(
            [
                timestamp_closed("t1", "target", "clock-a"),
                timestamp_closed("a1", "against", "clock-b"),
            ],
            [],
        )
        .unwrap();
        let result = builder(&history)
            .scope(ComparisonScope::window("State").on_event_time())
            .normalization(ComparisonNormalizationPolicy::event_time())
            .relate_within(TemporalTolerance::timestamp_ticks(0).unwrap())
            .run()
            .unwrap();

        assert_eq!(result.relations().len(), 2);
        assert!(
            result
                .relations()
                .iter()
                .all(|relation| relation.kind() != EpisodeRelationKind::OneToOne)
        );
    }

    #[test]
    fn exact_key_and_partition_are_hard_relation_boundaries() {
        let target = closed("t1", 0, 5, "target");
        let mut different_key = closed("a1", 0, 5, "against");
        different_key.key = "device-2".to_owned();
        let key_history = WindowHistory::from_records([target.clone(), different_key], []).unwrap();
        assert!(
            builder(&key_history)
                .run()
                .unwrap()
                .relations()
                .iter()
                .all(|relation| {
                    matches!(
                        relation.kind(),
                        EpisodeRelationKind::UnmatchedTarget
                            | EpisodeRelationKind::UnmatchedAgainst
                    )
                })
        );

        let mut different_partition = closed("a2", 0, 5, "against");
        different_partition.partition = Some("south".to_owned());
        let partition_history =
            WindowHistory::from_records([target, different_partition], []).unwrap();
        assert!(
            builder(&partition_history)
                .run()
                .unwrap()
                .relations()
                .iter()
                .all(|relation| {
                    matches!(
                        relation.kind(),
                        EpisodeRelationKind::UnmatchedTarget
                            | EpisodeRelationKind::UnmatchedAgainst
                    )
                })
        );
    }

    #[test]
    fn component_order_is_stable_across_history_insertion_order() {
        let records = [
            closed("t2", 20, 25, "target"),
            closed("a2", 21, 24, "against"),
            closed("t1", 0, 5, "target"),
            closed("a1", 1, 4, "against"),
        ];
        let forward = WindowHistory::from_records(records.clone(), []).unwrap();
        let reversed = WindowHistory::from_records(records.into_iter().rev(), []).unwrap();
        let identities = |history: &WindowHistory| {
            builder(history)
                .run()
                .unwrap()
                .relations()
                .iter()
                .map(|relation| {
                    (
                        relation.kind(),
                        relation.target_episodes()[0].id().as_str().to_owned(),
                        relation.against_episodes()[0].id().as_str().to_owned(),
                    )
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(identities(&forward), identities(&reversed));
    }

    #[test]
    fn metrics_union_overlapping_fragments_and_keep_directional_deltas() {
        let (history, _) = compare([
            closed("t1", 0, 5, "target"),
            closed("t2", 3, 8, "target"),
            closed("a1", 2, 6, "against"),
        ]);

        let result = builder(&history).run().unwrap();
        let metrics = result.relations()[0].metrics();
        assert_eq!(metrics.target_active_magnitude(), 8);
        assert_eq!(metrics.against_active_magnitude(), 4);
        assert_eq!(metrics.overlap_magnitude(), 4);
        assert_eq!(metrics.target_coverage_ratio(), Some(0.5));
        assert_eq!(metrics.against_coverage_ratio(), Some(1.0));
        assert_eq!(metrics.intersection_over_union(), Some(0.5));
        assert_eq!(metrics.minimum_gap_magnitude(), Some(0));
        assert_eq!(metrics.onset_delta_magnitude(), Some(2));
        assert_eq!(metrics.recovery_delta_magnitude(), Some(-2));
        assert_eq!(metrics.active_magnitude_delta(), Some(-4));
        assert_eq!(metrics.elapsed_magnitude_delta(), Some(-4));
    }

    #[test]
    fn zero_magnitude_metrics_have_null_ratios() {
        let (history, _) = compare([closed("t1", 5, 5, "target"), closed("a1", 5, 5, "against")]);
        let result = builder(&history).run().unwrap();
        let metrics = result.relations()[0].metrics();
        assert_eq!(metrics.target_coverage_ratio(), None);
        assert_eq!(metrics.against_coverage_ratio(), None);
        assert_eq!(metrics.intersection_over_union(), None);
        assert_eq!(metrics.minimum_gap_magnitude(), Some(0));
    }

    #[test]
    fn checked_component_totals_return_typed_overflow() {
        let history = WindowHistory::from_records(
            [
                closed("t1", i64::MIN, -1, "target"),
                closed("t2", 0, i64::MAX, "target-2"),
                closed("a1", 0, 0, "against"),
            ],
            [],
        )
        .unwrap();
        let error = history
            .compare_episodes("overflow")
            .target(
                "target",
                ComparisonSelector::for_sources(["target", "target-2"]),
            )
            .against("against", ComparisonSelector::for_source("against"))
            .scope(ComparisonScope::window("State"))
            .relate_within(TemporalTolerance::processing_positions(1).unwrap())
            .run()
            .unwrap_err();

        assert_eq!(error, EpisodeComparisonError::MagnitudeOverflow);
    }

    #[test]
    fn public_timing_deltas_saturate_at_extreme_positions() {
        let (history, _) = compare([
            closed("t1", i64::MIN, -1, "target"),
            closed("a1", 0, i64::MAX, "against"),
        ]);
        let result = builder(&history)
            .relate_within(TemporalTolerance::processing_positions(1).unwrap())
            .run()
            .unwrap();
        let metrics = result.relations()[0].metrics();

        assert_eq!(metrics.onset_delta_magnitude(), Some(i64::MAX));
        assert_eq!(metrics.recovery_delta_magnitude(), Some(i64::MAX));
    }

    #[test]
    fn overlapping_normalized_record_membership_is_rejected() {
        let (history, _) = compare([closed("shared", 0, 5, "target")]);
        let error = history
            .compare_episodes("self match")
            .target(
                "target-side",
                ComparisonSelector::runtime_only("target", "all", |_| true),
            )
            .against(
                "against-side",
                ComparisonSelector::runtime_only("against", "all", |_| true),
            )
            .scope(ComparisonScope::window("State"))
            .run()
            .unwrap_err();

        assert_eq!(
            error,
            EpisodeComparisonError::SelfMembership {
                record_id: "shared".to_owned(),
                target_name: "target-side".to_owned(),
                against_name: "against-side".to_owned(),
            }
        );
    }

    #[test]
    fn relation_settles_strictly_after_tolerance_boundary() {
        let (history, _) = compare([closed("t1", 0, 5, "target"), closed("a1", 0, 5, "against")]);
        let comparison =
            builder(&history).relate_within(TemporalTolerance::processing_positions(2).unwrap());

        let before = comparison
            .clone()
            .run_live(TemporalPoint::position(6))
            .unwrap();
        let at = comparison
            .clone()
            .run_live(TemporalPoint::position(7))
            .unwrap();
        let after = comparison.run_live(TemporalPoint::position(8)).unwrap();

        assert_eq!(
            before.relations()[0].finality(),
            &ComparisonFinality::Provisional
        );
        assert_eq!(
            at.relations()[0].finality(),
            &ComparisonFinality::Provisional
        );
        assert_eq!(after.relations()[0].finality(), &ComparisonFinality::Final);
        assert_eq!(
            after.plan().normalization().open_window_horizon.as_ref(),
            after.evaluation_horizon()
        );
        assert_eq!(
            after.target_episodes().evaluation_horizon(),
            after.evaluation_horizon()
        );
        assert_eq!(
            after.against_episodes().evaluation_horizon(),
            after.evaluation_horizon()
        );
    }

    #[test]
    fn duplicate_side_configuration_is_not_silently_replaced() {
        let (history, _) = compare([]);
        let error = builder(&history)
            .against("other", ComparisonSelector::for_source("other"))
            .build()
            .unwrap_err();
        assert_eq!(error, EpisodeComparisonError::DuplicateAgainst);
    }
}
