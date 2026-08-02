use crate::TemporalAxis;

use super::{Episode, EpisodeComparisonError, EpisodeRelationMetrics};

#[derive(Clone, Copy, Debug)]
struct Interval {
    start: i64,
    end: i64,
}

pub(crate) fn calculate(
    targets: &[Episode],
    against: &[Episode],
    time_axis: TemporalAxis,
) -> Result<EpisodeRelationMetrics, EpisodeComparisonError> {
    let target_union = build_union(targets);
    let against_union = build_union(against);
    let target_active = total_magnitude(&target_union)?;
    let against_active = total_magnitude(&against_union)?;
    let overlap = intersection_magnitude(&target_union, &against_union)?;
    let active_union = i128::from(target_active) + i128::from(against_active) - i128::from(overlap);
    let has_both = !targets.is_empty() && !against.is_empty();

    Ok(EpisodeRelationMetrics {
        time_axis,
        target_active_magnitude: target_active,
        against_active_magnitude: against_active,
        overlap_magnitude: overlap,
        target_coverage_ratio: ratio(overlap, target_active),
        against_coverage_ratio: ratio(overlap, against_active),
        intersection_over_union: (active_union != 0)
            .then_some(overlap as f64 / active_union as f64),
        minimum_gap_magnitude: has_both.then(|| minimum_gap(&target_union, &against_union)),
        onset_delta_magnitude: has_both
            .then(|| earliest_start(against).saturating_sub(earliest_start(targets))),
        recovery_delta_magnitude: has_both
            .then(|| latest_end(against).saturating_sub(latest_end(targets))),
        active_magnitude_delta: has_both.then(|| against_active.saturating_sub(target_active)),
        elapsed_magnitude_delta: has_both
            .then(|| component_elapsed(against).saturating_sub(component_elapsed(targets))),
    })
}

fn build_union(episodes: &[Episode]) -> Vec<Interval> {
    let mut intervals = episodes
        .iter()
        .flat_map(Episode::fragments)
        .map(|fragment| Interval {
            start: fragment.range().start().magnitude(),
            end: fragment.range().end().magnitude(),
        })
        .collect::<Vec<_>>();
    intervals.sort_by_key(|interval| (interval.start, interval.end));
    let mut union = Vec::<Interval>::new();
    for interval in intervals {
        if let Some(last) = union.last_mut()
            && interval.start <= last.end
        {
            last.end = last.end.max(interval.end);
        } else {
            union.push(interval);
        }
    }
    union
}

fn total_magnitude(intervals: &[Interval]) -> Result<i64, EpisodeComparisonError> {
    let total = intervals
        .iter()
        .try_fold(0_i128, |total, interval| {
            total.checked_add(i128::from(interval.end) - i128::from(interval.start))
        })
        .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
    i64::try_from(total).map_err(|_| EpisodeComparisonError::MagnitudeOverflow)
}

fn intersection_magnitude(
    target: &[Interval],
    against: &[Interval],
) -> Result<i64, EpisodeComparisonError> {
    let (mut target_index, mut against_index, mut total) = (0, 0, 0_i128);
    while target_index < target.len() && against_index < against.len() {
        let start = target[target_index].start.max(against[against_index].start);
        let end = target[target_index].end.min(against[against_index].end);
        if end > start {
            total = total
                .checked_add(i128::from(end) - i128::from(start))
                .ok_or(EpisodeComparisonError::MagnitudeOverflow)?;
        }
        if target[target_index].end <= against[against_index].end {
            target_index += 1;
        } else {
            against_index += 1;
        }
    }
    i64::try_from(total).map_err(|_| EpisodeComparisonError::MagnitudeOverflow)
}

fn ratio(numerator: i64, denominator: i64) -> Option<f64> {
    (denominator != 0).then_some(numerator as f64 / denominator as f64)
}

fn minimum_gap(target: &[Interval], against: &[Interval]) -> i64 {
    let mut minimum = i64::MAX;
    for left in target {
        for right in against {
            let gap = gap(*left, *right);
            if gap == 0 {
                return 0;
            }
            minimum = minimum.min(gap);
        }
    }
    minimum
}

pub(crate) fn fragment_gap(left: &crate::TemporalRange, right: &crate::TemporalRange) -> i128 {
    let left = Interval {
        start: left.start().magnitude(),
        end: left.end().magnitude(),
    };
    let right = Interval {
        start: right.start().magnitude(),
        end: right.end().magnitude(),
    };
    gap_i128(left, right)
}

fn gap(left: Interval, right: Interval) -> i64 {
    i64::try_from(gap_i128(left, right)).unwrap_or(i64::MAX)
}

fn gap_i128(left: Interval, right: Interval) -> i128 {
    if left.end < right.start {
        i128::from(right.start) - i128::from(left.end)
    } else if right.end < left.start {
        i128::from(left.start) - i128::from(right.end)
    } else {
        0
    }
}

fn earliest_start(episodes: &[Episode]) -> i64 {
    episodes
        .iter()
        .flat_map(Episode::fragments)
        .map(|fragment| fragment.range().start().magnitude())
        .min()
        .expect("matched component side is non-empty")
}

fn latest_end(episodes: &[Episode]) -> i64 {
    episodes
        .iter()
        .flat_map(Episode::fragments)
        .map(|fragment| fragment.range().end().magnitude())
        .max()
        .expect("matched component side is non-empty")
}

fn component_elapsed(episodes: &[Episode]) -> i64 {
    latest_end(episodes).saturating_sub(earliest_start(episodes))
}
