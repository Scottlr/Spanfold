//! Comparator-family row derivation and transition matching.

use super::*;

pub(super) fn build_overlap_rows(aligned: &AlignedComparison) -> Vec<OverlapRow> {
    let mut rows = Vec::new();
    for segment in &aligned.segments {
        if segment.target_record_ids.is_empty() || !segment.against_is_active {
            continue;
        }

        rows.push(OverlapRow {
            window_name: segment.window_name.clone(),
            key: segment.key.clone(),
            partition: segment.partition.clone(),
            range: segment.range.clone(),
            target_record_ids: segment.target_record_ids.clone(),
            against_record_ids: segment.against_record_ids.clone(),
        });
    }
    rows
}

pub(super) fn build_residual_rows(aligned: &AlignedComparison) -> Vec<ResidualRow> {
    let mut rows = Vec::new();
    for segment in &aligned.segments {
        if segment.target_record_ids.is_empty() || segment.against_is_active {
            continue;
        }

        rows.push(ResidualRow {
            window_name: segment.window_name.clone(),
            key: segment.key.clone(),
            partition: segment.partition.clone(),
            range: segment.range.clone(),
            target_record_ids: segment.target_record_ids.clone(),
        });
    }
    rows
}

pub(super) fn build_missing_rows(aligned: &AlignedComparison) -> Vec<MissingRow> {
    let mut rows = Vec::new();
    for segment in &aligned.segments {
        if !segment.target_record_ids.is_empty() || !segment.against_is_active {
            continue;
        }

        rows.push(MissingRow {
            window_name: segment.window_name.clone(),
            key: segment.key.clone(),
            partition: segment.partition.clone(),
            range: segment.range.clone(),
            against_record_ids: segment.against_record_ids.clone(),
        });
    }
    rows
}

pub(super) fn build_coverage_rows(aligned: &AlignedComparison) -> Vec<CoverageRow> {
    let mut rows = Vec::new();
    for segment in &aligned.segments {
        if segment.target_record_ids.is_empty() {
            continue;
        }

        let target_magnitude = segment.range.end - segment.range.start;
        rows.push(CoverageRow {
            window_name: segment.window_name.clone(),
            key: segment.key.clone(),
            partition: segment.partition.clone(),
            range: segment.range.clone(),
            target_magnitude,
            covered_magnitude: if segment.against_is_active {
                target_magnitude
            } else {
                0
            },
            target_record_ids: segment.target_record_ids.clone(),
            against_record_ids: segment.against_record_ids.clone(),
        });
    }
    rows
}

pub(super) fn build_gap_rows(aligned: &AlignedComparison) -> Vec<GapRow> {
    let mut groups: BTreeMap<GroupKey, Vec<&AlignedSegmentArtifact>> = BTreeMap::new();
    for segment in &aligned.segments {
        groups
            .entry((
                segment.window_name.clone(),
                segment.key.clone(),
                segment.partition.clone(),
                segment.range.axis,
                segment.range.clock.clone(),
            ))
            .or_default()
            .push(segment);
    }

    let mut rows = Vec::new();
    for mut segments in groups.into_values() {
        segments.retain(|segment| {
            !segment.target_record_ids.is_empty() || !segment.against_record_ids.is_empty()
        });
        segments.sort_by(|left, right| {
            left.range
                .start
                .cmp(&right.range.start)
                .then_with(|| left.range.end.cmp(&right.range.end))
                .then_with(|| left.target_record_ids.cmp(&right.target_record_ids))
                .then_with(|| left.against_record_ids.cmp(&right.against_record_ids))
        });

        for pair in segments.windows(2) {
            let [current, next] = pair else {
                unreachable!("windows(2) always yields two segments");
            };
            if current.range.end >= next.range.start {
                continue;
            }

            let mut boundary_record_ids = BTreeSet::new();
            boundary_record_ids.extend(current.target_record_ids.iter().cloned());
            boundary_record_ids.extend(current.against_record_ids.iter().cloned());
            boundary_record_ids.extend(next.target_record_ids.iter().cloned());
            boundary_record_ids.extend(next.against_record_ids.iter().cloned());

            rows.push(GapRow {
                window_name: current.window_name.clone(),
                key: current.key.clone(),
                partition: current.partition.clone(),
                range: RowRange {
                    start: current.range.end,
                    end: next.range.start,
                    axis: current.range.axis,
                    clock: current.range.clock.clone(),
                },
                boundary_record_ids: boundary_record_ids.into_iter().collect(),
            });
        }
    }
    rows
}

pub(super) fn build_symmetric_difference_rows(
    aligned: &AlignedComparison,
) -> Vec<SymmetricDifferenceRow> {
    let mut rows = Vec::new();
    for segment in &aligned.segments {
        let has_target = !segment.target_record_ids.is_empty();
        let has_against = segment.against_is_active;
        if has_target == has_against {
            continue;
        }

        rows.push(SymmetricDifferenceRow {
            window_name: segment.window_name.clone(),
            key: segment.key.clone(),
            partition: segment.partition.clone(),
            range: segment.range.clone(),
            side: if has_target {
                ComparisonSide::Target
            } else {
                ComparisonSide::Against
            },
            target_record_ids: segment.target_record_ids.clone(),
            against_record_ids: segment.against_record_ids.clone(),
        });
    }
    rows
}

pub(super) fn build_containment_rows(
    aligned: &AlignedComparison,
    prepared: &PreparedComparison,
) -> Vec<ContainmentRow> {
    let mut rows = Vec::new();
    let target_ranges = target_ranges_by_record_id(prepared);
    for segment in &aligned.segments {
        if segment.target_record_ids.is_empty() {
            continue;
        }

        if segment.against_is_active {
            rows.push(ContainmentRow {
                window_name: segment.window_name.clone(),
                key: segment.key.clone(),
                partition: segment.partition.clone(),
                range: segment.range.clone(),
                status: ContainmentStatus::Contained,
                target_record_ids: segment.target_record_ids.clone(),
                container_record_ids: segment.against_record_ids.clone(),
            });
            continue;
        }

        for target_record_id in &segment.target_record_ids {
            rows.push(ContainmentRow {
                window_name: segment.window_name.clone(),
                key: segment.key.clone(),
                partition: segment.partition.clone(),
                range: segment.range.clone(),
                status: classify_uncontained_segment(
                    target_ranges.get(target_record_id.as_str()),
                    (segment.range.start, segment.range.end),
                ),
                target_record_ids: vec![target_record_id.clone()],
                container_record_ids: Vec::new(),
            });
        }
    }
    rows
}

pub(super) fn target_ranges_by_record_id(
    prepared: &PreparedComparison,
) -> BTreeMap<&str, (i64, i64)> {
    let mut ranges = BTreeMap::new();
    for window in &prepared.normalized_windows {
        if window.side == ComparisonSide::Target {
            ranges.insert(
                window.record_id.as_str(),
                (
                    window.range.start().magnitude(),
                    window.range.end().magnitude(),
                ),
            );
        }
    }
    ranges
}

pub(super) fn classify_uncontained_segment(
    target_range: Option<&(i64, i64)>,
    segment_range: (i64, i64),
) -> ContainmentStatus {
    let Some(&(target_start, target_end)) = target_range else {
        return ContainmentStatus::NotContained;
    };

    if segment_range.0 == target_start {
        return ContainmentStatus::LeftOverhang;
    }
    if segment_range.1 == target_end {
        return ContainmentStatus::RightOverhang;
    }
    ContainmentStatus::NotContained
}

pub(super) fn build_lead_lag_rows(
    groups: &BTreeMap<GroupKey, GroupWindows<'_>>,
    transition: LeadLagTransition,
    axis: TemporalAxis,
    tolerance_magnitude: i64,
) -> (Vec<LeadLagRow>, LeadLagSummary) {
    let mut rows = Vec::new();
    for ((window_name, key, partition, _group_axis, _clock), (targets, againsts)) in groups {
        let mut comparison_points: Vec<TransitionPoint<'_>> = againsts
            .iter()
            .filter(|against| against.start.axis() == axis)
            .map(|against| TransitionPoint {
                record_id: against.record_id,
                point: if transition == LeadLagTransition::Start {
                    against.start.clone()
                } else {
                    against.end.clone()
                },
            })
            .collect();
        comparison_points.sort_by(|left, right| {
            left.point
                .try_cmp(&right.point)
                .expect("lead-lag groups share a temporal domain")
                .then_with(|| left.record_id.cmp(right.record_id))
        });

        for target in targets {
            if target.start.axis() != axis {
                continue;
            }
            let target_point = if transition == LeadLagTransition::Start {
                target.start.clone()
            } else {
                target.end.clone()
            };

            if comparison_points.is_empty() {
                rows.push(LeadLagRow {
                    window_name: window_name.clone(),
                    key: key.clone(),
                    partition: partition.clone(),
                    transition: transition.clone(),
                    axis,
                    target_point: row_point_from_temporal_point(&target_point),
                    comparison_point: None,
                    delta_magnitude: None,
                    tolerance_magnitude,
                    is_within_tolerance: false,
                    direction: LeadLagDirection::MissingComparison,
                    target_record_id: target.record_id.to_owned(),
                    comparison_record_id: None,
                });
                continue;
            }

            let nearest = find_nearest_transition(&comparison_points, &target_point);
            let delta = delta_magnitude(&target_point, &nearest.point);
            rows.push(LeadLagRow {
                window_name: window_name.clone(),
                key: key.clone(),
                partition: partition.clone(),
                transition: transition.clone(),
                axis,
                target_point: row_point_from_temporal_point(&target_point),
                comparison_point: Some(row_point_from_temporal_point(&nearest.point)),
                delta_magnitude: Some(delta),
                tolerance_magnitude,
                is_within_tolerance: absolute_delta_magnitude(&target_point, &nearest.point)
                    <= tolerance_magnitude,
                direction: direction_for_delta(delta),
                target_record_id: target.record_id.to_owned(),
                comparison_record_id: Some(nearest.record_id.to_owned()),
            });
        }
    }

    let mut summary = LeadLagSummary {
        transition,
        axis,
        tolerance_magnitude,
        row_count: rows.len(),
        target_lead_count: 0,
        target_lag_count: 0,
        equal_count: 0,
        missing_comparison_count: 0,
        outside_tolerance_count: 0,
        minimum_delta_magnitude: None,
        maximum_delta_magnitude: None,
    };
    for row in &rows {
        if !row.is_within_tolerance {
            summary.outside_tolerance_count += 1;
        }
        match row.direction {
            LeadLagDirection::TargetLeads => summary.target_lead_count += 1,
            LeadLagDirection::TargetLags => summary.target_lag_count += 1,
            LeadLagDirection::Equal => summary.equal_count += 1,
            LeadLagDirection::MissingComparison => summary.missing_comparison_count += 1,
        }
        if let Some(delta) = row.delta_magnitude {
            summary.minimum_delta_magnitude = Some(
                summary
                    .minimum_delta_magnitude
                    .map_or(delta, |current| current.min(delta)),
            );
            summary.maximum_delta_magnitude = Some(
                summary
                    .maximum_delta_magnitude
                    .map_or(delta, |current| current.max(delta)),
            );
        }
    }

    (rows, summary)
}

pub(super) fn find_nearest_transition<'a>(
    candidates: &'a [TransitionPoint<'a>],
    target_point: &crate::TemporalPoint,
) -> TransitionPoint<'a> {
    let insertion = candidates.partition_point(|candidate| {
        candidate
            .point
            .try_cmp(target_point)
            .is_ok_and(std::cmp::Ordering::is_lt)
    });
    let mut options = Vec::with_capacity(2);
    if let Some(candidate) = candidates.get(insertion) {
        options.push(candidate);
    }
    if insertion > 0 {
        options.push(&candidates[insertion - 1]);
    }
    options
        .into_iter()
        .min_by(|left, right| {
            absolute_delta_magnitude(target_point, &left.point)
                .cmp(&absolute_delta_magnitude(target_point, &right.point))
                .then_with(|| left.record_id.cmp(right.record_id))
        })
        .expect("nearest transition requires a non-empty candidate list")
        .clone()
}

pub(super) fn delta_magnitude(
    target_point: &crate::TemporalPoint,
    comparison_point: &crate::TemporalPoint,
) -> i64 {
    debug_assert!(target_point.is_compatible_with(comparison_point));
    let delta = i128::from(target_point.magnitude()) - i128::from(comparison_point.magnitude());
    delta.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

pub(super) fn absolute_delta_magnitude(
    target_point: &crate::TemporalPoint,
    comparison_point: &crate::TemporalPoint,
) -> i64 {
    debug_assert!(target_point.is_compatible_with(comparison_point));
    let delta = i128::from(target_point.magnitude()) - i128::from(comparison_point.magnitude());
    delta.abs().min(i128::from(i64::MAX)) as i64
}

pub(super) fn direction_for_delta(delta: i64) -> LeadLagDirection {
    if delta < 0 {
        LeadLagDirection::TargetLeads
    } else if delta > 0 {
        LeadLagDirection::TargetLags
    } else {
        LeadLagDirection::Equal
    }
}

pub(super) fn build_as_of_rows(
    groups: &BTreeMap<GroupKey, GroupWindows<'_>>,
    direction: AsOfDirection,
    axis: TemporalAxis,
    tolerance_magnitude: i64,
) -> (Vec<AsOfRow>, Vec<ComparisonDiagnostic>) {
    let mut rows = Vec::new();
    let mut diagnostics = Vec::new();
    for ((window_name, key, partition, _group_axis, _clock), (targets, againsts)) in groups {
        let mut candidates: Vec<TransitionPoint<'_>> = againsts
            .iter()
            .filter(|against| against.start.axis() == axis)
            .map(|against| TransitionPoint {
                record_id: against.record_id,
                point: against.start.clone(),
            })
            .collect();
        candidates.sort_by(|left, right| {
            left.point
                .try_cmp(&right.point)
                .expect("as-of groups share a temporal domain")
                .then_with(|| left.record_id.cmp(right.record_id))
        });

        for target in targets {
            if target.start.axis() != axis {
                continue;
            }
            let target_point = target.start.clone();
            let target_point_row = row_point_from_temporal_point(&target_point);

            if candidates.is_empty() {
                rows.push(AsOfRow {
                    window_name: window_name.clone(),
                    key: key.clone(),
                    partition: partition.clone(),
                    axis,
                    direction: direction.clone(),
                    target_point: target_point_row,
                    matched_point: None,
                    distance_magnitude: None,
                    tolerance_magnitude,
                    status: AsOfMatchStatus::NoMatch,
                    target_record_id: target.record_id.to_owned(),
                    matched_record_id: None,
                });
                continue;
            }

            let (best, ambiguous, future_rejected) =
                find_as_of_candidate(&candidates, &target_point, &direction);
            let Some(best) = best else {
                rows.push(AsOfRow {
                    window_name: window_name.clone(),
                    key: key.clone(),
                    partition: partition.clone(),
                    axis,
                    direction: direction.clone(),
                    target_point: target_point_row,
                    matched_point: None,
                    distance_magnitude: future_rejected
                        .as_ref()
                        .map(|item| absolute_delta_magnitude(&target_point, &item.point)),
                    tolerance_magnitude,
                    status: if future_rejected.is_some() {
                        AsOfMatchStatus::FutureRejected
                    } else {
                        AsOfMatchStatus::NoMatch
                    },
                    target_record_id: target.record_id.to_owned(),
                    matched_record_id: None,
                });
                continue;
            };

            let distance = absolute_delta_magnitude(&target_point, &best.point);
            if distance > tolerance_magnitude {
                rows.push(AsOfRow {
                    window_name: window_name.clone(),
                    key: key.clone(),
                    partition: partition.clone(),
                    axis,
                    direction: direction.clone(),
                    target_point: target_point_row,
                    matched_point: None,
                    distance_magnitude: Some(distance),
                    tolerance_magnitude,
                    status: AsOfMatchStatus::NoMatch,
                    target_record_id: target.record_id.to_owned(),
                    matched_record_id: None,
                });
                continue;
            }

            if ambiguous {
                diagnostics.push(ComparisonDiagnostic {
                    code: "AmbiguousAsOfMatch".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                });
            }

            rows.push(AsOfRow {
                window_name: window_name.clone(),
                key: key.clone(),
                partition: partition.clone(),
                axis,
                direction: direction.clone(),
                target_point: target_point_row,
                matched_point: Some(row_point_from_temporal_point(&best.point)),
                distance_magnitude: Some(distance),
                tolerance_magnitude,
                status: if ambiguous {
                    AsOfMatchStatus::Ambiguous
                } else if distance == 0 {
                    AsOfMatchStatus::Exact
                } else {
                    AsOfMatchStatus::Matched
                },
                target_record_id: target.record_id.to_owned(),
                matched_record_id: Some(best.record_id.to_owned()),
            });
        }
    }

    (rows, diagnostics)
}

pub(super) fn find_as_of_candidate<'a>(
    candidates: &'a [TransitionPoint<'a>],
    target_point: &crate::TemporalPoint,
    direction: &AsOfDirection,
) -> (
    Option<TransitionPoint<'a>>,
    bool,
    Option<TransitionPoint<'a>>,
) {
    let lower_bound = candidates.partition_point(|candidate| {
        candidate
            .point
            .try_cmp(target_point)
            .is_ok_and(Ordering::is_lt)
    });

    let mut indexes = Vec::new();
    let mut future_rejected = None;
    match direction {
        AsOfDirection::Previous => {
            if lower_bound == 0 {
                future_rejected = candidates.first().cloned();
            } else {
                add_equal_point_run(candidates, lower_bound - 1, &mut indexes);
            }
        }
        AsOfDirection::Next => {
            if lower_bound < candidates.len() {
                add_equal_point_run(candidates, lower_bound, &mut indexes);
            }
        }
        AsOfDirection::Nearest => {
            if lower_bound > 0 {
                add_equal_point_run(candidates, lower_bound - 1, &mut indexes);
            }
            if lower_bound < candidates.len() {
                add_equal_point_run(candidates, lower_bound, &mut indexes);
            }
        }
    }

    let mut best = None;
    let mut best_distance = None;
    let mut ambiguous = false;
    for index in indexes {
        let candidate = &candidates[index];
        let distance = absolute_delta_magnitude(target_point, &candidate.point);
        if best_distance.is_none_or(|current| distance < current) {
            best = Some(candidate.clone());
            best_distance = Some(distance);
            ambiguous = false;
        } else if Some(distance) == best_distance {
            ambiguous = true;
            if best
                .as_ref()
                .is_some_and(|current| candidate.record_id < current.record_id)
            {
                best = Some(candidate.clone());
            }
        }
    }

    (best, ambiguous, future_rejected)
}

fn add_equal_point_run(
    candidates: &[TransitionPoint<'_>],
    center: usize,
    indexes: &mut Vec<usize>,
) {
    let point = &candidates[center].point;
    let mut first = center;
    while first > 0
        && candidates[first - 1]
            .point
            .try_cmp(point)
            .is_ok_and(Ordering::is_eq)
    {
        first -= 1;
    }
    let mut last = center;
    while last + 1 < candidates.len()
        && candidates[last + 1]
            .point
            .try_cmp(point)
            .is_ok_and(Ordering::is_eq)
    {
        last += 1;
    }
    indexes.extend(first..=last);
}
