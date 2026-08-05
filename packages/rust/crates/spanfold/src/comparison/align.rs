//! Grouping and endpoint-sweep alignment for prepared comparison windows.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use serde::Serialize;

use crate::{TemporalAxis, TemporalPoint};

use super::prepare::PreparedComparison;
use super::rows::{ComparisonSide, RowPoint, RowRange};
use super::selector::AgainstSelection;

/// Aligned segment artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AlignedSegmentArtifact {
    /// Deterministic segment identifier.
    #[serde(rename = "segmentId")]
    pub segment_id: String,
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Aligned range.
    pub range: RowRange,
    /// Target record IDs covering the range.
    #[serde(rename = "targetRecordIds")]
    pub target_record_ids: Vec<String>,
    /// Comparison record IDs covering the range.
    #[serde(rename = "againstRecordIds")]
    pub against_record_ids: Vec<String>,
    /// Whether the comparison side was active after selector evaluation.
    #[serde(rename = "againstIsActive")]
    pub against_is_active: bool,
    /// Sources active on the comparison side during the aligned segment.
    #[serde(rename = "againstActiveSources")]
    pub against_active_sources: Vec<String>,
}

/// Aligned comparison artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AlignedComparison {
    /// Deterministic aligned segments.
    pub segments: Vec<AlignedSegmentArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct SegmentRef<'a> {
    pub(super) start: TemporalPoint,
    pub(super) end: TemporalPoint,
    pub(super) record_id: &'a str,
    pub(super) record_ids: Vec<String>,
    pub(super) source: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AlignedSegment {
    start: i64,
    end: i64,
    axis: TemporalAxis,
    clock: Option<String>,
    target_record_ids: Vec<String>,
    against_record_ids: Vec<String>,
    against_is_active: bool,
    against_active_sources: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TransitionPoint<'a> {
    pub(super) record_id: &'a str,
    pub(super) point: TemporalPoint,
}

pub(super) type GroupKey = (String, String, Option<String>, TemporalAxis, Option<String>);
pub(super) type GroupWindows<'a> = (Vec<SegmentRef<'a>>, Vec<SegmentRef<'a>>);

pub(super) fn align_internal(prepared: &PreparedComparison) -> AlignedComparison {
    let groups = group_normalized_windows(prepared);
    align_grouped(prepared, &groups)
}

pub(super) fn align_grouped(
    prepared: &PreparedComparison,
    groups: &BTreeMap<GroupKey, GroupWindows<'_>>,
) -> AlignedComparison {
    let mut segments = Vec::new();
    for ((window_name, key, partition, axis, clock), (targets, againsts)) in groups {
        let against = prepared.plan.against_for_alignment();
        for segment in aligned_segments(targets.as_slice(), againsts.as_slice(), &against) {
            segments.push(AlignedSegmentArtifact {
                segment_id: format!("segment[{}]", segments.len()),
                window_name: window_name.clone(),
                key: key.clone(),
                partition: partition.clone(),
                range: RowRange {
                    start: segment.start,
                    end: segment.end,
                    axis: *axis,
                    clock: clock.clone(),
                },
                target_record_ids: segment.target_record_ids,
                against_record_ids: segment.against_record_ids,
                against_is_active: segment.against_is_active,
                against_active_sources: segment.against_active_sources,
            });
        }
    }
    AlignedComparison { segments }
}

pub(super) fn group_normalized_windows(
    prepared: &PreparedComparison,
) -> BTreeMap<GroupKey, GroupWindows<'_>> {
    let mut groups: BTreeMap<GroupKey, GroupWindows<'_>> = BTreeMap::new();
    for normalized in &prepared.normalized_windows {
        let group = groups
            .entry((
                normalized.window.window_name.clone(),
                normalized.window.key.clone(),
                normalized.window.partition.clone(),
                normalized.range.start().axis(),
                normalized.range.start().clock().map(str::to_owned),
            ))
            .or_default();
        let segment = SegmentRef {
            start: normalized.range.start(),
            end: normalized.range.end(),
            record_id: normalized.record_id.as_str(),
            record_ids: normalized.record_ids.clone(),
            source: normalized.window.source.as_deref(),
        };
        match normalized.side {
            ComparisonSide::Target => group.0.push(segment),
            ComparisonSide::Against => group.1.push(segment),
        }
    }
    groups
}

pub(super) fn row_point_from_temporal_point(point: &TemporalPoint) -> RowPoint {
    RowPoint {
        axis: point.axis(),
        magnitude: point.magnitude(),
        clock: point.clock().map(str::to_owned),
    }
}

fn aligned_segments(
    targets: &[SegmentRef<'_>],
    againsts: &[SegmentRef<'_>],
    against_selection: &AgainstSelection,
) -> Vec<AlignedSegment> {
    let mut points = Vec::with_capacity((targets.len() + againsts.len()) * 2);
    for item in targets {
        points.push(item.start.clone());
        points.push(item.end.clone());
    }
    for item in againsts {
        points.push(item.start.clone());
        points.push(item.end.clone());
    }

    points.sort_by(|left, right| {
        left.try_cmp(right)
            .expect("alignment groups share a temporal domain")
    });
    points.dedup();

    let mut target_starts = targets
        .iter()
        .enumerate()
        .map(|(index, item)| (item.start.clone(), index))
        .collect::<Vec<_>>();
    let mut target_ends = targets
        .iter()
        .enumerate()
        .map(|(index, item)| (item.end.clone(), index))
        .collect::<Vec<_>>();
    let mut against_starts = againsts
        .iter()
        .enumerate()
        .map(|(index, item)| (item.start.clone(), index))
        .collect::<Vec<_>>();
    let mut against_ends = againsts
        .iter()
        .enumerate()
        .map(|(index, item)| (item.end.clone(), index))
        .collect::<Vec<_>>();
    for events in [
        &mut target_starts,
        &mut target_ends,
        &mut against_starts,
        &mut against_ends,
    ] {
        events.sort_by(|left, right| {
            left.0
                .try_cmp(&right.0)
                .expect("alignment groups share a temporal domain")
        });
    }
    let mut active_targets = BTreeSet::new();
    let mut active_againsts = BTreeSet::new();
    let mut active_against_source_counts = BTreeMap::new();
    let mut target_start_index = 0;
    let mut target_end_index = 0;
    let mut against_start_index = 0;
    let mut against_end_index = 0;
    let mut segments = Vec::new();
    for pair in points.windows(2) {
        let start = pair[0].clone();
        let end = pair[1].clone();
        if !matches!(start.try_cmp(&end), Ok(Ordering::Less)) {
            continue;
        }

        while target_end_index < target_ends.len()
            && matches!(
                target_ends[target_end_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            active_targets.remove(&target_ends[target_end_index].1);
            target_end_index += 1;
        }
        while against_end_index < against_ends.len()
            && matches!(
                against_ends[against_end_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            let index = against_ends[against_end_index].1;
            active_againsts.remove(&index);
            if let Some(source) = againsts[index].source
                && let Some(count) = active_against_source_counts.get_mut(source)
            {
                *count -= 1;
                if *count == 0 {
                    active_against_source_counts.remove(source);
                }
            }
            against_end_index += 1;
        }
        while target_start_index < target_starts.len()
            && matches!(
                target_starts[target_start_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            active_targets.insert(target_starts[target_start_index].1);
            target_start_index += 1;
        }
        while against_start_index < against_starts.len()
            && matches!(
                against_starts[against_start_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            let index = against_starts[against_start_index].1;
            active_againsts.insert(index);
            if let Some(source) = againsts[index].source {
                *active_against_source_counts.entry(source).or_insert(0) += 1;
            }
            against_start_index += 1;
        }

        let mut target_record_ids = Vec::new();
        let mut against_record_ids = Vec::new();
        for index in &active_targets {
            target_record_ids.extend(targets[*index].record_ids.iter().cloned());
        }
        for index in &active_againsts {
            against_record_ids.extend(againsts[*index].record_ids.iter().cloned());
        }

        let active_sources = active_against_source_counts
            .keys()
            .map(|source| (*source).to_owned())
            .collect::<Vec<_>>();

        let against_is_active = match against_selection {
            AgainstSelection::Sources(_) => !active_sources.is_empty(),
            AgainstSelection::Cohort {
                sources, activity, ..
            } => activity.is_active(active_sources.len(), sources.len()),
        };

        segments.push(AlignedSegment {
            start: start.magnitude(),
            end: end.magnitude(),
            axis: start.axis(),
            clock: start.clock().map(str::to_owned),
            target_record_ids,
            against_record_ids,
            against_is_active,
            against_active_sources: active_sources,
        });
    }
    segments
}
