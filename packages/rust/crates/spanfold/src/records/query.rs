use std::cmp::Ordering;

use super::{
    history::{WindowHistory, compare_window_records, sort_window_records},
    model::{
        ClosedWindow, OpenWindow, WindowGroupKind, WindowGroupSummary, WindowHistorySnapshot,
        WindowRecord, WindowRecordId, WindowSegment, WindowSnapshotFinality, WindowSnapshotRecord,
        WindowTag,
    },
    snapshot::snapshot_record,
    summary::{SummaryError, summarize_by_segment, summarize_by_tag, summarize_snapshot_records},
};
use crate::{PrimitiveValue, TemporalPoint, TemporalRangeError};

#[derive(Clone, Copy, Debug)]
enum WindowRef<'a> {
    Closed(&'a ClosedWindow),
    Open(&'a OpenWindow),
}

impl<'a> WindowRef<'a> {
    fn owned(self) -> WindowRecord {
        match self {
            Self::Closed(window) => WindowRecord::Closed(window.clone()),
            Self::Open(window) => WindowRecord::Open(window.clone()),
        }
    }

    fn window_name(self) -> &'a str {
        match self {
            Self::Closed(window) => &window.window_name,
            Self::Open(window) => &window.window_name,
        }
    }

    fn key(self) -> &'a str {
        match self {
            Self::Closed(window) => &window.key,
            Self::Open(window) => &window.key,
        }
    }

    fn source(self) -> Option<&'a str> {
        match self {
            Self::Closed(window) => window.source.as_deref(),
            Self::Open(window) => window.source.as_deref(),
        }
    }

    fn partition(self) -> Option<&'a str> {
        match self {
            Self::Closed(window) => window.partition.as_deref(),
            Self::Open(window) => window.partition.as_deref(),
        }
    }

    fn segments(self) -> &'a [WindowSegment] {
        match self {
            Self::Closed(window) => &window.segments,
            Self::Open(window) => &window.segments,
        }
    }

    fn tags(self) -> &'a [WindowTag] {
        match self {
            Self::Closed(window) => &window.tags,
            Self::Open(window) => &window.tags,
        }
    }

    fn is_closed(self) -> bool {
        matches!(self, Self::Closed(_))
    }

    fn start(self) -> &'a TemporalPoint {
        match self {
            Self::Closed(window) => window.range.start_ref(),
            Self::Open(window) => &window.start,
        }
    }

    fn end(self) -> Option<&'a TemporalPoint> {
        match self {
            Self::Closed(window) => Some(window.range.end_ref()),
            Self::Open(_) => None,
        }
    }

    fn id(self) -> &'a WindowRecordId {
        match self {
            Self::Closed(window) => &window.id,
            Self::Open(window) => &window.id,
        }
    }
}

fn compare_window_refs(left: WindowRef<'_>, right: WindowRef<'_>) -> Ordering {
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

#[derive(Clone, Debug, Default, PartialEq)]
struct WindowQuerySpec {
    window_names: Vec<String>,
    keys: Vec<String>,
    sources: Vec<String>,
    partitions: Vec<String>,
    segments: Vec<(String, PrimitiveValue)>,
    tags: Vec<(String, PrimitiveValue)>,
}

impl WindowQuerySpec {
    fn matches(&self, record: &impl WindowQueryRecord) -> bool {
        self.window_names
            .iter()
            .all(|window_name| record.query_window_name() == window_name)
            && self.keys.iter().all(|key| record.query_key() == key)
            && self
                .sources
                .iter()
                .all(|source| record.query_source() == Some(source.as_str()))
            && self
                .partitions
                .iter()
                .all(|partition| record.query_partition() == Some(partition.as_str()))
            && self.segments.iter().all(|(name, value)| {
                record
                    .query_segments()
                    .iter()
                    .any(|segment| segment.name == *name && segment.value == *value)
            })
            && self.tags.iter().all(|(name, value)| {
                record
                    .query_tags()
                    .iter()
                    .any(|tag| tag.name == *name && tag.value == *value)
            })
    }
}

trait WindowQueryRecord {
    fn query_window_name(&self) -> &str;
    fn query_key(&self) -> &str;
    fn query_source(&self) -> Option<&str>;
    fn query_partition(&self) -> Option<&str>;
    fn query_segments(&self) -> &[WindowSegment];
    fn query_tags(&self) -> &[WindowTag];
}

impl WindowQueryRecord for WindowRef<'_> {
    fn query_window_name(&self) -> &str {
        (*self).window_name()
    }

    fn query_key(&self) -> &str {
        (*self).key()
    }

    fn query_source(&self) -> Option<&str> {
        (*self).source()
    }

    fn query_partition(&self) -> Option<&str> {
        (*self).partition()
    }

    fn query_segments(&self) -> &[WindowSegment] {
        (*self).segments()
    }

    fn query_tags(&self) -> &[WindowTag] {
        (*self).tags()
    }
}

impl WindowQueryRecord for WindowRecord {
    fn query_window_name(&self) -> &str {
        self.window_name()
    }

    fn query_key(&self) -> &str {
        self.key()
    }

    fn query_source(&self) -> Option<&str> {
        self.source()
    }

    fn query_partition(&self) -> Option<&str> {
        self.partition()
    }

    fn query_segments(&self) -> &[WindowSegment] {
        self.segments()
    }

    fn query_tags(&self) -> &[WindowTag] {
        self.tags()
    }
}

impl WindowQueryRecord for WindowSnapshotRecord {
    fn query_window_name(&self) -> &str {
        self.window.window_name()
    }

    fn query_key(&self) -> &str {
        self.window.key()
    }

    fn query_source(&self) -> Option<&str> {
        self.window.source()
    }

    fn query_partition(&self) -> Option<&str> {
        self.window.partition()
    }

    fn query_segments(&self) -> &[WindowSegment] {
        self.window.segments()
    }

    fn query_tags(&self) -> &[WindowTag] {
        self.window.tags()
    }
}

/// Fluent direct query API borrowing recorded windows.
#[derive(Clone)]
pub struct WindowHistoryRefQuery<'a> {
    windows: Vec<WindowRef<'a>>,
    spec: WindowQuerySpec,
}

impl<'a> WindowHistoryRefQuery<'a> {
    pub(crate) fn new(history: &'a WindowHistory) -> Self {
        let mut windows = history
            .closed
            .iter()
            .map(WindowRef::Closed)
            .chain(history.open.iter().map(WindowRef::Open))
            .collect::<Vec<_>>();
        windows.sort_by(|left, right| compare_window_refs(*left, *right));
        Self {
            windows,
            spec: WindowQuerySpec::default(),
        }
    }

    /// Filters by configured window name.
    #[must_use]
    pub fn where_window(mut self, window_name: &str) -> Self {
        self.spec.window_names.push(window_name.to_owned());
        self
    }

    /// Filters by logical key.
    #[must_use]
    pub fn where_key(mut self, key: &str) -> Self {
        self.spec.keys.push(key.to_owned());
        self
    }

    /// Filters by source/lane.
    #[must_use]
    pub fn where_source(mut self, source: &str) -> Self {
        self.spec.sources.push(source.to_owned());
        self
    }

    /// Filters by partition.
    #[must_use]
    pub fn where_partition(mut self, partition: &str) -> Self {
        self.spec.partitions.push(partition.to_owned());
        self
    }

    /// Filters by segment value.
    #[must_use]
    pub fn where_segment(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.spec.segments.push((name.to_owned(), value.into()));
        self
    }

    /// Filters by tag value.
    #[must_use]
    pub fn where_tag(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.spec.tags.push((name.to_owned(), value.into()));
        self
    }

    /// Filters to closed windows.
    #[must_use]
    pub fn closed(mut self) -> Self {
        self.windows.retain(|window| window.is_closed());
        self
    }

    /// Filters to open windows.
    #[must_use]
    pub fn open(mut self) -> Self {
        self.windows.retain(|window| !window.is_closed());
        self
    }

    /// Materializes matching windows only at the result boundary.
    #[must_use]
    pub fn windows(&self) -> Vec<WindowRecord> {
        self.windows
            .iter()
            .filter(|window| self.spec.matches(*window))
            .copied()
            .map(WindowRef::owned)
            .collect()
    }

    /// Materializes matching closed windows.
    #[must_use]
    pub fn closed_windows(&self) -> Vec<ClosedWindow> {
        self.windows
            .iter()
            .filter(|window| self.spec.matches(*window))
            .filter_map(|window| match window {
                WindowRef::Closed(window) => Some((*window).clone()),
                WindowRef::Open(_) => None,
            })
            .collect()
    }

    /// Materializes matching open windows.
    #[must_use]
    pub fn open_windows(&self) -> Vec<OpenWindow> {
        self.windows
            .iter()
            .filter(|window| self.spec.matches(*window))
            .filter_map(|window| match window {
                WindowRef::Closed(_) => None,
                WindowRef::Open(window) => Some((*window).clone()),
            })
            .collect()
    }

    /// Returns the latest matching window.
    #[must_use]
    pub fn latest(&self) -> Option<WindowRecord> {
        self.windows
            .iter()
            .rfind(|window| self.spec.matches(*window))
            .copied()
            .map(WindowRef::owned)
    }

    /// Summarizes matching windows by segment.
    pub fn summarize_by_segment(
        &self,
        name: &str,
    ) -> Result<Vec<WindowGroupSummary>, SummaryError> {
        summarize_by_segment(self.windows(), name)
    }

    /// Summarizes matching windows by tag.
    pub fn summarize_by_tag(&self, name: &str) -> Result<Vec<WindowGroupSummary>, SummaryError> {
        summarize_by_tag(self.windows(), name)
    }
}

impl std::fmt::Debug for WindowHistoryRefQuery<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let windows = self
            .windows
            .iter()
            .filter(|window| self.spec.matches(*window))
            .collect::<Vec<_>>();
        formatter
            .debug_struct("WindowHistoryRefQuery")
            .field("windows", &windows)
            .finish()
    }
}

/// Fluent direct query API for owned window records.
#[derive(Clone)]
pub struct WindowHistoryQuery {
    windows: Vec<WindowRecord>,
    spec: WindowQuerySpec,
}

impl WindowHistoryQuery {
    /// Creates a query over window records.
    #[must_use]
    pub fn new(mut windows: Vec<WindowRecord>) -> Self {
        sort_window_records(&mut windows);
        Self {
            windows,
            spec: WindowQuerySpec::default(),
        }
    }

    /// Filters by configured window name.
    #[must_use]
    pub fn where_window(mut self, window_name: &str) -> Self {
        self.spec.window_names.push(window_name.to_owned());
        self
    }

    /// Filters by logical key.
    #[must_use]
    pub fn where_key(mut self, key: &str) -> Self {
        self.spec.keys.push(key.to_owned());
        self
    }

    /// Filters by source/lane.
    #[must_use]
    pub fn where_source(mut self, source: &str) -> Self {
        self.spec.sources.push(source.to_owned());
        self
    }

    /// Filters by partition.
    #[must_use]
    pub fn where_partition(mut self, partition: &str) -> Self {
        self.spec.partitions.push(partition.to_owned());
        self
    }

    /// Filters by segment value.
    #[must_use]
    pub fn where_segment(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.spec.segments.push((name.to_owned(), value.into()));
        self
    }

    /// Filters by tag value.
    #[must_use]
    pub fn where_tag(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.spec.tags.push((name.to_owned(), value.into()));
        self
    }

    /// Filters to closed windows.
    #[must_use]
    pub fn closed(mut self) -> Self {
        self.windows.retain(WindowRecord::is_closed);
        self
    }

    /// Filters to open windows.
    #[must_use]
    pub fn open(mut self) -> Self {
        self.windows.retain(|window| !window.is_closed());
        self
    }

    /// Materializes matching windows.
    #[must_use]
    pub fn windows(&self) -> Vec<WindowRecord> {
        self.windows
            .iter()
            .filter(|window| self.spec.matches(*window))
            .cloned()
            .collect()
    }

    /// Materializes matching closed windows.
    #[must_use]
    pub fn closed_windows(&self) -> Vec<ClosedWindow> {
        self.windows
            .iter()
            .filter(|window| self.spec.matches(*window))
            .filter_map(|window| match window {
                WindowRecord::Closed(window) => Some(window.clone()),
                WindowRecord::Open(_) => None,
            })
            .collect()
    }

    /// Materializes matching open windows.
    #[must_use]
    pub fn open_windows(&self) -> Vec<OpenWindow> {
        self.windows
            .iter()
            .filter(|window| self.spec.matches(*window))
            .filter_map(|window| match window {
                WindowRecord::Closed(_) => None,
                WindowRecord::Open(window) => Some(window.clone()),
            })
            .collect()
    }

    /// Returns the latest matching window.
    #[must_use]
    pub fn latest(&self) -> Option<WindowRecord> {
        self.windows
            .iter()
            .rfind(|window| self.spec.matches(*window))
            .cloned()
    }

    /// Materializes matching snapshot records at a horizon.
    pub fn windows_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<Vec<WindowSnapshotRecord>, TemporalRangeError> {
        Ok(self.snapshot_at(horizon)?.records)
    }

    /// Materializes final matching snapshot records at a horizon.
    pub fn closed_windows_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<Vec<WindowSnapshotRecord>, TemporalRangeError> {
        Ok(self.snapshot_at(horizon)?.query().closed_windows())
    }

    /// Materializes provisional matching snapshot records at a horizon.
    pub fn open_windows_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<Vec<WindowSnapshotRecord>, TemporalRangeError> {
        Ok(self.snapshot_at(horizon)?.query().open_windows())
    }

    /// Returns the latest matching snapshot record at a horizon.
    pub fn latest_window_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<Option<WindowSnapshotRecord>, TemporalRangeError> {
        Ok(self.snapshot_at(horizon)?.query().latest_window())
    }

    /// Summarizes matching windows by segment.
    pub fn summarize_by_segment(
        &self,
        name: &str,
    ) -> Result<Vec<WindowGroupSummary>, SummaryError> {
        summarize_by_segment(self.windows(), name)
    }

    /// Summarizes matching windows by tag.
    pub fn summarize_by_tag(&self, name: &str) -> Result<Vec<WindowGroupSummary>, SummaryError> {
        summarize_by_tag(self.windows(), name)
    }

    fn snapshot_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<WindowHistorySnapshot, TemporalRangeError> {
        let mut records = self
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
}

impl std::fmt::Debug for WindowHistoryQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WindowHistoryQuery")
            .field("windows", &self.windows())
            .finish()
    }
}

impl PartialEq for WindowHistoryQuery {
    fn eq(&self, other: &Self) -> bool {
        self.windows() == other.windows()
    }
}

impl WindowHistorySnapshot {
    /// Starts a query over snapshot records.
    #[must_use]
    pub fn query(&self) -> WindowSnapshotQuery {
        WindowSnapshotQuery::new(self.horizon.clone(), self.records.clone())
    }
}

/// Fluent direct query API for window snapshot records.
#[derive(Clone)]
pub struct WindowSnapshotQuery {
    horizon: TemporalPoint,
    records: Vec<WindowSnapshotRecord>,
    spec: WindowQuerySpec,
}

impl WindowSnapshotQuery {
    /// Creates a snapshot query.
    #[must_use]
    pub fn new(horizon: TemporalPoint, mut records: Vec<WindowSnapshotRecord>) -> Self {
        records.sort_by(|left, right| compare_window_records(&left.window, &right.window));
        Self {
            horizon,
            records,
            spec: WindowQuerySpec::default(),
        }
    }

    /// Filters by configured window name.
    #[must_use]
    pub fn where_window(mut self, window_name: &str) -> Self {
        self.spec.window_names.push(window_name.to_owned());
        self
    }

    /// Filters by logical key.
    #[must_use]
    pub fn where_key(mut self, key: &str) -> Self {
        self.spec.keys.push(key.to_owned());
        self
    }

    /// Filters by source/lane.
    #[must_use]
    pub fn where_source(mut self, source: &str) -> Self {
        self.spec.sources.push(source.to_owned());
        self
    }

    /// Filters by partition.
    #[must_use]
    pub fn where_partition(mut self, partition: &str) -> Self {
        self.spec.partitions.push(partition.to_owned());
        self
    }

    /// Filters by segment value.
    #[must_use]
    pub fn where_segment(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.spec.segments.push((name.to_owned(), value.into()));
        self
    }

    /// Filters by tag value.
    #[must_use]
    pub fn where_tag(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.spec.tags.push((name.to_owned(), value.into()));
        self
    }

    /// Materializes all matching snapshot records.
    #[must_use]
    pub fn windows(&self) -> Vec<WindowSnapshotRecord> {
        self.records
            .iter()
            .filter(|record| self.spec.matches(*record))
            .cloned()
            .collect()
    }

    /// Materializes final snapshot records.
    #[must_use]
    pub fn closed_windows(&self) -> Vec<WindowSnapshotRecord> {
        self.records
            .iter()
            .filter(|record| self.spec.matches(*record))
            .filter(|record| record.finality == WindowSnapshotFinality::Final)
            .cloned()
            .collect()
    }

    /// Materializes provisional snapshot records.
    #[must_use]
    pub fn open_windows(&self) -> Vec<WindowSnapshotRecord> {
        self.records
            .iter()
            .filter(|record| self.spec.matches(*record))
            .filter(|record| record.finality == WindowSnapshotFinality::Provisional)
            .cloned()
            .collect()
    }

    /// Returns the latest matching snapshot record.
    #[must_use]
    pub fn latest_window(&self) -> Option<WindowSnapshotRecord> {
        self.records
            .iter()
            .rfind(|record| self.spec.matches(*record))
            .cloned()
    }

    /// Summarizes matching snapshot records by segment.
    pub fn summarize_by_segment(
        &self,
        name: &str,
    ) -> Result<Vec<WindowGroupSummary>, SummaryError> {
        summarize_snapshot_records(&self.windows(), WindowGroupKind::Segment, name)
    }

    /// Summarizes matching snapshot records by tag.
    pub fn summarize_by_tag(&self, name: &str) -> Result<Vec<WindowGroupSummary>, SummaryError> {
        summarize_snapshot_records(&self.windows(), WindowGroupKind::Tag, name)
    }
}

impl std::fmt::Debug for WindowSnapshotQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WindowSnapshotQuery")
            .field("horizon", &self.horizon)
            .field("records", &self.windows())
            .finish()
    }
}

impl PartialEq for WindowSnapshotQuery {
    fn eq(&self, other: &Self) -> bool {
        self.horizon == other.horizon && self.windows() == other.windows()
    }
}
