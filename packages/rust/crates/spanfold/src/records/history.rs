use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeMap};

use super::{
    annotations::{WindowAnnotation, WindowAnnotationTarget},
    model::{
        ClosedWindow, OpenWindow, WindowHistoryImportError, WindowHistorySnapshot, WindowOverlap,
        WindowRecord, WindowRecordId, WindowResidualSegment, WindowTag,
    },
    query::WindowHistoryRefQuery,
    snapshot::snapshot_record,
};
use crate::{PrimitiveValue, TemporalAxis, TemporalPoint, TemporalRangeError};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
/// In-memory history of open and closed windows.
pub struct WindowHistory {
    pub(crate) closed: Vec<ClosedWindow>,
    pub(crate) open: Vec<OpenWindow>,
    #[serde(skip)]
    pub(crate) open_indexes: BTreeMap<WindowRecordId, usize>,
    pub(crate) annotations: Vec<WindowAnnotation>,
}

impl<'de> Deserialize<'de> for WindowHistory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawHistory {
            #[serde(default)]
            closed: Vec<ClosedWindow>,
            #[serde(default)]
            open: Vec<OpenWindow>,
            #[serde(default)]
            annotations: Vec<WindowAnnotation>,
        }

        let raw = RawHistory::deserialize(deserializer)?;
        let open_indexes = rebuild_open_indexes(&raw.open)
            .map_err(|error| serde::de::Error::custom(error.to_string()))?;
        Ok(Self {
            closed: raw.closed,
            open: raw.open,
            open_indexes,
            annotations: raw.annotations,
        })
    }
}

impl WindowHistory {
    /// Creates an empty window history.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            closed: Vec::new(),
            open: Vec::new(),
            open_indexes: BTreeMap::new(),
            annotations: Vec::new(),
        }
    }

    /// Imports materialized closed and open records into a queryable history.
    ///
    /// The import boundary performs the same open-record identity validation
    /// as history deserialization and rebuilds the live open-record index.
    /// Queries sort both record families into deterministic order regardless
    /// of their input order.
    pub fn from_records(
        closed: impl IntoIterator<Item = ClosedWindow>,
        open: impl IntoIterator<Item = OpenWindow>,
    ) -> Result<Self, WindowHistoryImportError> {
        let closed = closed.into_iter().collect::<Vec<_>>();
        let open = open.into_iter().collect::<Vec<_>>();
        let open_indexes = rebuild_open_indexes(&open)?;
        Ok(Self {
            closed,
            open,
            open_indexes,
            annotations: Vec::new(),
        })
    }

    /// Returns closed windows.
    #[must_use]
    pub fn closed_windows(&self) -> &[ClosedWindow] {
        &self.closed
    }

    /// Returns open windows.
    #[must_use]
    pub fn open_windows(&self) -> &[OpenWindow] {
        &self.open
    }

    /// Returns all annotations attached to recorded windows.
    #[must_use]
    pub fn annotations(&self) -> &[WindowAnnotation] {
        &self.annotations
    }

    /// Returns all windows in deterministic query order.
    #[must_use]
    pub fn windows(&self) -> Vec<WindowRecord> {
        let mut windows = self
            .closed
            .iter()
            .cloned()
            .map(WindowRecord::Closed)
            .chain(self.open.iter().cloned().map(WindowRecord::Open))
            .collect::<Vec<_>>();
        sort_window_records(&mut windows);
        windows
    }

    /// Starts a direct read-only query borrowing this history.
    #[must_use]
    pub fn query(&self) -> WindowHistoryRefQuery<'_> {
        WindowHistoryRefQuery::new(self)
    }

    /// Returns recorded windows for a configured window name.
    #[must_use]
    pub fn for_window(&self, window_name: &str) -> Vec<WindowRecord> {
        self.query().where_window(window_name).windows()
    }

    /// Returns windows containing a required segment value.
    #[must_use]
    pub fn with_segment(&self, name: &str, value: impl Into<PrimitiveValue>) -> Vec<WindowRecord> {
        self.query().where_segment(name, value).windows()
    }

    /// Returns windows containing a required tag value.
    #[must_use]
    pub fn with_tag(&self, name: &str, value: impl Into<PrimitiveValue>) -> Vec<WindowRecord> {
        self.query().where_tag(name, value).windows()
    }

    /// Evaluates recorded windows at an explicit horizon.
    pub fn snapshot_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<WindowHistorySnapshot, TemporalRangeError> {
        let mut records = self
            .query()
            .windows()
            .into_iter()
            .map(|window| snapshot_record(window, horizon.clone()))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| compare_window_records(&left.window, &right.window));
        Ok(WindowHistorySnapshot { horizon, records })
    }

    /// Finds overlapping closed windows within the same window/key/partition scope.
    #[must_use]
    pub fn find_overlaps(&self) -> Vec<WindowOverlap> {
        let scope_index = index_closed_windows_by_scope(self.closed.iter().enumerate());
        let mut overlaps = Vec::new();
        for (index, first) in self.closed.iter().enumerate() {
            let candidate_indexes = &scope_index[&ClosedWindowScope::from(first)];
            let later_candidate_start =
                candidate_indexes.partition_point(|candidate_index| *candidate_index <= index);
            for &second_index in &candidate_indexes[later_candidate_start..] {
                let second = &self.closed[second_index];
                if closed_windows_overlap(first, second) {
                    overlaps.push(WindowOverlap {
                        first: first.clone(),
                        second: second.clone(),
                    });
                }
            }
        }
        overlaps
    }

    /// Finds target-source closed segments not covered by other sources.
    #[must_use]
    pub fn find_residuals(&self, target_source: &str) -> Vec<WindowResidualSegment> {
        let comparison_scope_index = index_closed_windows_by_scope(
            self.closed
                .iter()
                .enumerate()
                .filter(|(_, window)| window.source.as_deref() != Some(target_source)),
        );
        let mut residuals = Vec::new();
        for target in &self.closed {
            if target.source.as_deref() != Some(target_source) {
                continue;
            }
            let mut segments = vec![(
                target.range.start().magnitude(),
                target.range.end().magnitude(),
            )];
            let candidate_indexes = comparison_scope_index
                .get(&ClosedWindowScope::from(target))
                .map_or(&[][..], Vec::as_slice);
            for &comparison_index in candidate_indexes {
                let comparison = &self.closed[comparison_index];
                if comparison.id == target.id || !closed_windows_overlap(target, comparison) {
                    continue;
                }
                segments = subtract_position_window(&segments, comparison);
            }
            residuals.extend(segments.into_iter().filter(|(start, end)| start < end).map(
                |(start_position, end_position)| WindowResidualSegment {
                    window_name: target.window_name.clone(),
                    key: target.key.clone(),
                    source: target_source.to_owned(),
                    start_position,
                    end_position,
                    partition: target.partition.clone(),
                    axis: target.range.start().axis(),
                    clock: target.range.start().clock().map(str::to_owned),
                },
            ));
        }
        residuals
    }

    /// Attaches append-only metadata to a recorded window target.
    pub fn annotate(
        &mut self,
        target: WindowAnnotationTarget,
        name: impl Into<String>,
        value: impl Into<PrimitiveValue>,
        known_at: Option<TemporalPoint>,
    ) -> WindowAnnotation {
        let name = name.into();
        let revision = 1 + self
            .annotations
            .iter()
            .filter(|annotation| annotation.target == target && annotation.name == name)
            .count();
        let annotation = WindowAnnotation {
            target,
            name,
            value: value.into(),
            known_at,
            revision,
        };
        self.annotations.push(annotation.clone());
        annotation
    }

    /// Returns annotations attached to a window target.
    #[must_use]
    pub fn annotations_for(&self, target: &WindowAnnotationTarget) -> Vec<WindowAnnotation> {
        self.annotations
            .iter()
            .filter(|annotation| &annotation.target == target)
            .cloned()
            .collect()
    }

    /// Returns annotations whose known-at point is at or before a horizon.
    #[must_use]
    pub fn annotations_known_at(
        &self,
        target: &WindowAnnotationTarget,
        horizon: TemporalPoint,
    ) -> Vec<WindowAnnotation> {
        self.annotations
            .iter()
            .filter(|annotation| {
                &annotation.target == target
                    && annotation.known_at.as_ref().is_some_and(|known_at| {
                        known_at.axis() == horizon.axis()
                            && known_at
                                .try_cmp(&horizon)
                                .is_ok_and(std::cmp::Ordering::is_le)
                    })
            })
            .cloned()
            .collect()
    }

    /// Builds a directional source matrix for one recorded window family.
    #[must_use]
    pub fn compare_sources(
        &self,
        name: &str,
        window_name: &str,
        sources: &[String],
    ) -> crate::SourceMatrixResult {
        crate::compare_sources(self, name, window_name, sources)
    }

    /// Compares parent and child window families as a hierarchy explanation.
    #[must_use]
    pub fn compare_hierarchy(
        &self,
        name: &str,
        parent_window_name: &str,
        child_window_name: &str,
    ) -> crate::HierarchyComparisonResult {
        crate::compare_hierarchy(self, name, parent_window_name, child_window_name)
    }

    pub(crate) fn push_closed(&mut self, window: ClosedWindow) {
        self.closed.push(window);
    }

    pub(crate) fn push_open(&mut self, window: OpenWindow) {
        self.open_indexes.insert(window.id.clone(), self.open.len());
        self.open.push(window);
    }

    pub(crate) fn remove_open(&mut self, id: &WindowRecordId) -> Option<OpenWindow> {
        let index = self.open_indexes.remove(id)?;
        let removed = self.open.swap_remove(index);
        if index < self.open.len() {
            self.open_indexes.insert(self.open[index].id.clone(), index);
        }
        Some(removed)
    }

    pub(crate) fn update_open_tags(&mut self, id: &WindowRecordId, tags: Vec<WindowTag>) -> bool {
        let Some(index) = self.open_indexes.get(id).copied() else {
            return false;
        };
        let Some(window) = self.open.get_mut(index) else {
            return false;
        };
        window.tags = tags;
        true
    }
}

fn rebuild_open_indexes(
    open: &[OpenWindow],
) -> Result<BTreeMap<WindowRecordId, usize>, WindowHistoryImportError> {
    let mut open_indexes = BTreeMap::new();
    for (index, window) in open.iter().enumerate() {
        if open_indexes.insert(window.id.clone(), index).is_some() {
            return Err(WindowHistoryImportError::DuplicateOpenRecordId {
                record_id: window.id.clone(),
            });
        }
    }
    Ok(open_indexes)
}

pub(crate) fn sort_window_records(windows: &mut [WindowRecord]) {
    windows.sort_by(compare_window_records);
}

pub(crate) fn compare_window_records(left: &WindowRecord, right: &WindowRecord) -> Ordering {
    left.window_name()
        .cmp(right.window_name())
        .then_with(|| left.key().cmp(right.key()))
        .then_with(|| left.source().cmp(&right.source()))
        .then_with(|| left.partition().cmp(&right.partition()))
        .then_with(|| left.start().magnitude().cmp(&right.start().magnitude()))
        .then_with(|| {
            left.end()
                .map_or(i64::MAX, |point| point.magnitude())
                .cmp(&right.end().map_or(i64::MAX, |point| point.magnitude()))
        })
        .then_with(|| left.id().cmp(right.id()))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ClosedWindowScope<'a> {
    window_name: &'a str,
    key: &'a str,
    partition: Option<&'a str>,
    axis: TemporalAxis,
    clock: Option<&'a str>,
}

impl<'a> From<&'a ClosedWindow> for ClosedWindowScope<'a> {
    fn from(window: &'a ClosedWindow) -> Self {
        let start = window.range.start_ref();
        Self {
            window_name: &window.window_name,
            key: &window.key,
            partition: window.partition.as_deref(),
            axis: start.axis(),
            clock: if start.axis() == TemporalAxis::Timestamp {
                start.clock()
            } else {
                None
            },
        }
    }
}

fn index_closed_windows_by_scope<'a>(
    windows: impl Iterator<Item = (usize, &'a ClosedWindow)>,
) -> BTreeMap<ClosedWindowScope<'a>, Vec<usize>> {
    let mut scope_index = BTreeMap::<ClosedWindowScope<'a>, Vec<usize>>::new();
    for (index, window) in windows {
        scope_index
            .entry(ClosedWindowScope::from(window))
            .or_default()
            .push(index);
    }
    scope_index
}

fn closed_windows_overlap(first: &ClosedWindow, second: &ClosedWindow) -> bool {
    matches!(
        first.range.start().try_cmp(&second.range.end()),
        Ok(Ordering::Less)
    ) && matches!(
        second.range.start().try_cmp(&first.range.end()),
        Ok(Ordering::Less)
    )
}

fn subtract_position_window(segments: &[(i64, i64)], window: &ClosedWindow) -> Vec<(i64, i64)> {
    let mut result = Vec::new();
    let window_start = window.range.start().magnitude();
    let window_end = window.range.end().magnitude();
    for (start, end) in segments {
        if window_end <= *start || window_start >= *end {
            result.push((*start, *end));
            continue;
        }
        if *start < window_start {
            result.push((*start, window_start));
        }
        if window_end < *end {
            result.push((window_end, *end));
        }
    }
    result
}
