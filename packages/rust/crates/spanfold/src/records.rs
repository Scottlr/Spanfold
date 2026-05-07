use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::{
    ComparisonFinality, PrimitiveValue, TemporalAxis, TemporalPoint, TemporalRange,
    TemporalRangeError,
};

/// Deterministic window record identifier.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct WindowRecordId(String);

impl WindowRecordId {
    /// Creates a new record identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Analytical segment captured with a window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowSegment {
    /// Segment name.
    pub name: String,
    /// Segment value.
    pub value: PrimitiveValue,
    /// Optional parent segment name.
    pub parent_name: Option<String>,
}

impl WindowSegment {
    /// Creates a segment.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            parent_name: None,
        }
    }

    /// Sets the parent segment name.
    #[must_use]
    pub fn with_parent(mut self, parent_name: impl Into<String>) -> Self {
        self.parent_name = Some(parent_name.into());
        self
    }
}

/// Descriptive tag captured with a window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowTag {
    /// Tag name.
    pub name: String,
    /// Tag value.
    pub value: PrimitiveValue,
}

impl WindowTag {
    /// Creates a tag.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Describes why a recorded window boundary was emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WindowBoundaryReason {
    /// The active predicate changed from true to false.
    ActivePredicateEnded,
    /// The window remained active but one or more segment values changed.
    SegmentChanged,
}

/// Describes one segment value change that caused a window boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowBoundaryChange {
    /// Segment dimension name.
    pub segment_name: String,
    /// Previous segment value, or `None` when the segment was added.
    pub previous_value: Option<PrimitiveValue>,
    /// Current segment value, or `None` when the segment was removed.
    pub current_value: Option<PrimitiveValue>,
}

/// Closed state window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClosedWindow {
    /// Window record ID.
    pub id: WindowRecordId,
    /// Window family name.
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Window temporal range.
    pub range: TemporalRange,
    /// Availability point used for known-at filtering, when explicitly supplied.
    pub known_at: Option<TemporalPoint>,
    /// Optional source/lane.
    pub source: Option<String>,
    /// Optional partition.
    pub partition: Option<String>,
    /// Captured segments.
    pub segments: Vec<WindowSegment>,
    /// Captured tags.
    pub tags: Vec<WindowTag>,
    /// Reason this window closed, when known.
    pub boundary_reason: Option<WindowBoundaryReason>,
    /// Segment value changes that caused this boundary.
    pub boundary_changes: Vec<WindowBoundaryChange>,
}

/// Open state window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenWindow {
    /// Window record ID.
    pub id: WindowRecordId,
    /// Window family name.
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Window start point.
    pub start: TemporalPoint,
    /// Availability point used for known-at filtering, when explicitly supplied.
    pub known_at: Option<TemporalPoint>,
    /// Optional source/lane.
    pub source: Option<String>,
    /// Optional partition.
    pub partition: Option<String>,
    /// Captured segments.
    pub segments: Vec<WindowSegment>,
    /// Captured tags.
    pub tags: Vec<WindowTag>,
}

/// A recorded open or closed window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WindowRecord {
    /// Closed/final window record.
    Closed(ClosedWindow),
    /// Open/provisional window record.
    Open(OpenWindow),
}

impl WindowRecord {
    /// Returns the window record ID.
    #[must_use]
    pub fn id(&self) -> &WindowRecordId {
        match self {
            Self::Closed(window) => &window.id,
            Self::Open(window) => &window.id,
        }
    }

    /// Returns the configured window name.
    #[must_use]
    pub fn window_name(&self) -> &str {
        match self {
            Self::Closed(window) => &window.window_name,
            Self::Open(window) => &window.window_name,
        }
    }

    /// Returns the logical window key.
    #[must_use]
    pub fn key(&self) -> &str {
        match self {
            Self::Closed(window) => &window.key,
            Self::Open(window) => &window.key,
        }
    }

    /// Returns the optional source/lane.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        match self {
            Self::Closed(window) => window.source.as_deref(),
            Self::Open(window) => window.source.as_deref(),
        }
    }

    /// Returns the optional partition.
    #[must_use]
    pub fn partition(&self) -> Option<&str> {
        match self {
            Self::Closed(window) => window.partition.as_deref(),
            Self::Open(window) => window.partition.as_deref(),
        }
    }

    /// Returns captured segments.
    #[must_use]
    pub fn segments(&self) -> &[WindowSegment] {
        match self {
            Self::Closed(window) => &window.segments,
            Self::Open(window) => &window.segments,
        }
    }

    /// Returns captured tags.
    #[must_use]
    pub fn tags(&self) -> &[WindowTag] {
        match self {
            Self::Closed(window) => &window.tags,
            Self::Open(window) => &window.tags,
        }
    }

    /// Returns whether this record is closed/final.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Closed(_))
    }

    /// Returns the start point.
    #[must_use]
    pub fn start(&self) -> TemporalPoint {
        match self {
            Self::Closed(window) => window.range.start(),
            Self::Open(window) => window.start,
        }
    }

    /// Returns the end point when closed.
    #[must_use]
    pub fn end(&self) -> Option<TemporalPoint> {
        match self {
            Self::Closed(window) => Some(window.range.end()),
            Self::Open(_) => None,
        }
    }
}

/// A snapshot view of one window at an explicit horizon.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowSnapshotRecord {
    /// Source window.
    pub window: WindowRecord,
    /// Visible range at the snapshot horizon.
    pub range: TemporalRange,
    /// Whether the visible range is final or provisional.
    pub finality: ComparisonFinality,
}

/// Snapshot of a window history at one temporal horizon.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowHistorySnapshot {
    /// Evaluation horizon.
    pub horizon: TemporalPoint,
    /// Snapshot records.
    pub records: Vec<WindowSnapshotRecord>,
}

/// Grouping dimension used by summary helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WindowGroupKind {
    /// Segment grouping.
    Segment,
    /// Tag grouping.
    Tag,
}

/// Summary for one segment or tag value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowGroupSummary {
    /// Grouping kind.
    pub group_kind: WindowGroupKind,
    /// Segment or tag name.
    pub name: String,
    /// Segment or tag value.
    pub value: PrimitiveValue,
    /// Number of records in the group.
    pub record_count: usize,
    /// Number of final records.
    pub final_count: usize,
    /// Number of provisional records.
    pub provisional_count: usize,
    /// Number of records with measured processing-position length.
    pub measured_position_count: usize,
    /// Total processing-position length.
    pub total_position_length: i64,
}

/// Closed-window overlap pair from direct history analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowOverlap {
    /// First closed window.
    pub first: ClosedWindow,
    /// Second closed window.
    pub second: ClosedWindow,
}

/// Target-only closed-window residual segment from direct history analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowResidualSegment {
    /// Window family name.
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Target source.
    pub source: String,
    /// Inclusive start position.
    pub start_position: i64,
    /// Exclusive end position.
    pub end_position: i64,
    /// Optional partition.
    pub partition: Option<String>,
}

/// Stable target identity for window annotations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WindowAnnotationTarget {
    /// Window family name.
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Window start position.
    pub start_position: i64,
    /// Optional source/lane.
    pub source: Option<String>,
    /// Optional partition.
    pub partition: Option<String>,
}

impl WindowAnnotationTarget {
    /// Creates a target from a recorded window.
    #[must_use]
    pub fn from_window(window: &WindowRecord) -> Self {
        Self {
            window_name: window.window_name().to_owned(),
            key: window.key().to_owned(),
            start_position: window.start().magnitude(),
            source: window.source().map(str::to_owned),
            partition: window.partition().map(str::to_owned),
        }
    }

    /// Creates a target from a closed window.
    #[must_use]
    pub fn from_closed(window: &ClosedWindow) -> Self {
        Self::from_window(&WindowRecord::Closed(window.clone()))
    }

    /// Creates a target from an open window.
    #[must_use]
    pub fn from_open(window: &OpenWindow) -> Self {
        Self::from_window(&WindowRecord::Open(window.clone()))
    }
}

/// Append-only metadata attached to a recorded window target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowAnnotation {
    /// Stable annotation target.
    pub target: WindowAnnotationTarget,
    /// Annotation name.
    pub name: String,
    /// Annotation value.
    pub value: PrimitiveValue,
    /// Availability point for known-at filtering.
    pub known_at: Option<TemporalPoint>,
    /// Revision number for repeated names on the same target.
    pub revision: usize,
}

/// In-memory history of open and closed windows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowHistory {
    closed: Vec<ClosedWindow>,
    open: Vec<OpenWindow>,
    annotations: Vec<WindowAnnotation>,
}

impl WindowHistory {
    /// Creates an empty window history.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            closed: Vec::new(),
            open: Vec::new(),
            annotations: Vec::new(),
        }
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

    /// Starts a direct read-only query over recorded windows.
    #[must_use]
    pub fn query(&self) -> WindowHistoryQuery {
        WindowHistoryQuery::new(self.windows())
    }

    /// Returns recorded windows for a configured window name.
    #[must_use]
    pub fn for_window(&self, window_name: &str) -> Vec<WindowRecord> {
        self.query().where_window(window_name).all()
    }

    /// Returns windows containing a required segment value.
    #[must_use]
    pub fn with_segment(&self, name: &str, value: impl Into<PrimitiveValue>) -> Vec<WindowRecord> {
        self.query().where_segment(name, value).all()
    }

    /// Returns windows containing a required tag value.
    #[must_use]
    pub fn with_tag(&self, name: &str, value: impl Into<PrimitiveValue>) -> Vec<WindowRecord> {
        self.query().where_tag(name, value).all()
    }

    /// Evaluates recorded windows at an explicit horizon.
    pub fn snapshot_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<WindowHistorySnapshot, TemporalRangeError> {
        let mut records = self
            .windows()
            .into_iter()
            .map(|window| snapshot_record(window, horizon))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| window_sort_key(&record.window));
        Ok(WindowHistorySnapshot { horizon, records })
    }

    /// Finds overlapping closed windows within the same window/key/partition scope.
    #[must_use]
    pub fn find_overlaps(&self) -> Vec<WindowOverlap> {
        let mut overlaps = Vec::new();
        for (index, first) in self.closed.iter().enumerate() {
            for second in &self.closed[index + 1..] {
                if same_closed_scope(first, second) && closed_windows_overlap(first, second) {
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
        let mut residuals = Vec::new();
        for target in &self.closed {
            if target.source.as_deref() != Some(target_source) {
                continue;
            }
            let mut segments = vec![(
                target.range.start().magnitude(),
                target.range.end().magnitude(),
            )];
            for comparison in &self.closed {
                if comparison.id == target.id
                    || comparison.source.as_deref() == Some(target_source)
                    || !same_closed_scope(target, comparison)
                    || !closed_windows_overlap(target, comparison)
                {
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
                    && annotation.known_at.is_some_and(|known_at| {
                        known_at.axis() == horizon.axis() && known_at <= horizon
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
        self.open.push(window);
    }

    pub(crate) fn open_windows_mut(&mut self) -> &mut Vec<OpenWindow> {
        &mut self.open
    }
}

/// Fluent direct query API for recorded windows.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowHistoryQuery {
    windows: Vec<WindowRecord>,
}

impl WindowHistoryQuery {
    /// Creates a query over window records.
    #[must_use]
    pub fn new(mut windows: Vec<WindowRecord>) -> Self {
        sort_window_records(&mut windows);
        Self { windows }
    }

    /// Filters by configured window name.
    #[must_use]
    pub fn where_window(mut self, window_name: &str) -> Self {
        self.windows
            .retain(|window| window.window_name() == window_name);
        self
    }

    /// Alias for [`where_window`](Self::where_window).
    #[must_use]
    pub fn window(self, window_name: &str) -> Self {
        self.where_window(window_name)
    }

    /// Filters by logical key.
    #[must_use]
    pub fn where_key(mut self, key: &str) -> Self {
        self.windows.retain(|window| window.key() == key);
        self
    }

    /// Alias for [`where_key`](Self::where_key).
    #[must_use]
    pub fn key(self, key: &str) -> Self {
        self.where_key(key)
    }

    /// Filters by source/lane.
    #[must_use]
    pub fn where_source(mut self, source: &str) -> Self {
        self.windows
            .retain(|window| window.source() == Some(source));
        self
    }

    /// Alias for [`where_source`](Self::where_source).
    #[must_use]
    pub fn source(self, source: &str) -> Self {
        self.where_source(source)
    }

    /// Alias for filtering by source/lane.
    #[must_use]
    pub fn where_lane(self, lane: &str) -> Self {
        self.where_source(lane)
    }

    /// Alias for [`where_lane`](Self::where_lane).
    #[must_use]
    pub fn lane(self, lane: &str) -> Self {
        self.where_lane(lane)
    }

    /// Filters by partition.
    #[must_use]
    pub fn where_partition(mut self, partition: &str) -> Self {
        self.windows
            .retain(|window| window.partition() == Some(partition));
        self
    }

    /// Alias for [`where_partition`](Self::where_partition).
    #[must_use]
    pub fn partition(self, partition: &str) -> Self {
        self.where_partition(partition)
    }

    /// Filters by segment value.
    #[must_use]
    pub fn where_segment(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        let value = value.into();
        self.windows.retain(|window| {
            window
                .segments()
                .iter()
                .any(|segment| segment.name == name && segment.value == value)
        });
        self
    }

    /// Alias for [`where_segment`](Self::where_segment).
    #[must_use]
    pub fn segment(self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.where_segment(name, value)
    }

    /// Filters by tag value.
    #[must_use]
    pub fn where_tag(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        let value = value.into();
        self.windows.retain(|window| {
            window
                .tags()
                .iter()
                .any(|tag| tag.name == name && tag.value == value)
        });
        self
    }

    /// Alias for [`where_tag`](Self::where_tag).
    #[must_use]
    pub fn tag(self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.where_tag(name, value)
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
        self.windows.clone()
    }

    /// Alias for [`windows`](Self::windows).
    #[must_use]
    pub fn all(&self) -> Vec<WindowRecord> {
        self.windows()
    }

    /// Materializes matching closed windows.
    #[must_use]
    pub fn closed_windows(&self) -> Vec<ClosedWindow> {
        self.windows
            .iter()
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
            .filter_map(|window| match window {
                WindowRecord::Closed(_) => None,
                WindowRecord::Open(window) => Some(window.clone()),
            })
            .collect()
    }

    /// Returns the latest matching window.
    #[must_use]
    pub fn latest(&self) -> Option<WindowRecord> {
        self.windows.last().cloned()
    }

    /// Alias for [`latest`](Self::latest).
    #[must_use]
    pub fn latest_window(&self) -> Option<WindowRecord> {
        self.latest()
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
    pub fn summarize_by_segment(&self, name: &str) -> Result<Vec<WindowGroupSummary>, String> {
        summarize_by_segment(self.windows.clone(), name)
    }

    /// Summarizes matching windows by tag.
    pub fn summarize_by_tag(&self, name: &str) -> Result<Vec<WindowGroupSummary>, String> {
        summarize_by_tag(self.windows.clone(), name)
    }

    fn snapshot_at(
        &self,
        horizon: TemporalPoint,
    ) -> Result<WindowHistorySnapshot, TemporalRangeError> {
        let mut records = self
            .windows
            .iter()
            .cloned()
            .map(|window| snapshot_record(window, horizon))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| window_sort_key(&record.window));
        Ok(WindowHistorySnapshot { horizon, records })
    }
}

impl WindowHistorySnapshot {
    /// Starts a query over snapshot records.
    #[must_use]
    pub fn query(&self) -> WindowSnapshotQuery {
        WindowSnapshotQuery::new(self.horizon, self.records.clone())
    }
}

/// Fluent direct query API for window snapshot records.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowSnapshotQuery {
    horizon: TemporalPoint,
    records: Vec<WindowSnapshotRecord>,
}

impl WindowSnapshotQuery {
    /// Creates a snapshot query.
    #[must_use]
    pub fn new(horizon: TemporalPoint, mut records: Vec<WindowSnapshotRecord>) -> Self {
        records.sort_by_key(|record| window_sort_key(&record.window));
        Self { horizon, records }
    }

    /// Filters by configured window name.
    #[must_use]
    pub fn where_window(mut self, window_name: &str) -> Self {
        self.records
            .retain(|record| record.window.window_name() == window_name);
        self
    }

    /// Alias for [`where_window`](Self::where_window).
    #[must_use]
    pub fn window(self, window_name: &str) -> Self {
        self.where_window(window_name)
    }

    /// Filters by logical key.
    #[must_use]
    pub fn where_key(mut self, key: &str) -> Self {
        self.records.retain(|record| record.window.key() == key);
        self
    }

    /// Alias for [`where_key`](Self::where_key).
    #[must_use]
    pub fn key(self, key: &str) -> Self {
        self.where_key(key)
    }

    /// Filters by source/lane.
    #[must_use]
    pub fn where_source(mut self, source: &str) -> Self {
        self.records
            .retain(|record| record.window.source() == Some(source));
        self
    }

    /// Alias for [`where_source`](Self::where_source).
    #[must_use]
    pub fn source(self, source: &str) -> Self {
        self.where_source(source)
    }

    /// Alias for filtering by source/lane.
    #[must_use]
    pub fn where_lane(self, lane: &str) -> Self {
        self.where_source(lane)
    }

    /// Alias for [`where_lane`](Self::where_lane).
    #[must_use]
    pub fn lane(self, lane: &str) -> Self {
        self.where_lane(lane)
    }

    /// Filters by partition.
    #[must_use]
    pub fn where_partition(mut self, partition: &str) -> Self {
        self.records
            .retain(|record| record.window.partition() == Some(partition));
        self
    }

    /// Alias for [`where_partition`](Self::where_partition).
    #[must_use]
    pub fn partition(self, partition: &str) -> Self {
        self.where_partition(partition)
    }

    /// Filters by segment value.
    #[must_use]
    pub fn where_segment(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        let value = value.into();
        self.records.retain(|record| {
            record
                .window
                .segments()
                .iter()
                .any(|segment| segment.name == name && segment.value == value)
        });
        self
    }

    /// Alias for [`where_segment`](Self::where_segment).
    #[must_use]
    pub fn segment(self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.where_segment(name, value)
    }

    /// Filters by tag value.
    #[must_use]
    pub fn where_tag(mut self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        let value = value.into();
        self.records.retain(|record| {
            record
                .window
                .tags()
                .iter()
                .any(|tag| tag.name == name && tag.value == value)
        });
        self
    }

    /// Alias for [`where_tag`](Self::where_tag).
    #[must_use]
    pub fn tag(self, name: &str, value: impl Into<PrimitiveValue>) -> Self {
        self.where_tag(name, value)
    }

    /// Materializes all matching snapshot records.
    #[must_use]
    pub fn windows(&self) -> Vec<WindowSnapshotRecord> {
        self.records.clone()
    }

    /// Alias for [`windows`](Self::windows).
    #[must_use]
    pub fn all(&self) -> Vec<WindowSnapshotRecord> {
        self.windows()
    }

    /// Materializes final snapshot records.
    #[must_use]
    pub fn closed_windows(&self) -> Vec<WindowSnapshotRecord> {
        self.records
            .iter()
            .filter(|record| record.finality == ComparisonFinality::Final)
            .cloned()
            .collect()
    }

    /// Materializes provisional snapshot records.
    #[must_use]
    pub fn open_windows(&self) -> Vec<WindowSnapshotRecord> {
        self.records
            .iter()
            .filter(|record| record.finality == ComparisonFinality::Provisional)
            .cloned()
            .collect()
    }

    /// Returns the latest matching snapshot record.
    #[must_use]
    pub fn latest_window(&self) -> Option<WindowSnapshotRecord> {
        self.records.last().cloned()
    }

    /// Alias for [`latest_window`](Self::latest_window).
    #[must_use]
    pub fn latest(&self) -> Option<WindowSnapshotRecord> {
        self.latest_window()
    }

    /// Summarizes matching snapshot records by segment.
    pub fn summarize_by_segment(&self, name: &str) -> Result<Vec<WindowGroupSummary>, String> {
        summarize_snapshot_records(&self.records, WindowGroupKind::Segment, name)
    }

    /// Summarizes matching snapshot records by tag.
    pub fn summarize_by_tag(&self, name: &str) -> Result<Vec<WindowGroupSummary>, String> {
        summarize_snapshot_records(&self.records, WindowGroupKind::Tag, name)
    }
}

/// Summarizes recorded windows by segment.
pub fn summarize_by_segment(
    windows: impl IntoIterator<Item = WindowRecord>,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, String> {
    summarize_windows(windows, WindowGroupKind::Segment, name)
}

/// Summarizes recorded windows by tag.
pub fn summarize_by_tag(
    windows: impl IntoIterator<Item = WindowRecord>,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, String> {
    summarize_windows(windows, WindowGroupKind::Tag, name)
}

#[derive(Clone, Debug)]
struct SummaryAccumulator {
    group_kind: WindowGroupKind,
    name: String,
    value: PrimitiveValue,
    record_count: usize,
    final_count: usize,
    provisional_count: usize,
    measured_position_count: usize,
    total_position_length: i64,
}

impl SummaryAccumulator {
    fn new(group_kind: WindowGroupKind, name: &str, value: PrimitiveValue) -> Self {
        Self {
            group_kind,
            name: name.to_owned(),
            value,
            record_count: 0,
            final_count: 0,
            provisional_count: 0,
            measured_position_count: 0,
            total_position_length: 0,
        }
    }

    fn add_window(&mut self, window: &WindowRecord) {
        self.record_count += 1;
        match window {
            WindowRecord::Closed(closed) => {
                self.final_count += 1;
                if closed.range.start().axis() == TemporalAxis::ProcessingPosition {
                    self.measured_position_count += 1;
                    self.total_position_length += closed.range.magnitude();
                }
            }
            WindowRecord::Open(_) => {
                self.provisional_count += 1;
            }
        }
    }

    fn add_snapshot(&mut self, record: &WindowSnapshotRecord) {
        self.record_count += 1;
        match record.finality {
            ComparisonFinality::Final => self.final_count += 1,
            ComparisonFinality::Provisional
            | ComparisonFinality::Revised
            | ComparisonFinality::Retracted => self.provisional_count += 1,
        }
        if record.range.start().axis() == TemporalAxis::ProcessingPosition {
            self.measured_position_count += 1;
            self.total_position_length += record.range.magnitude();
        }
    }

    fn into_summary(self) -> WindowGroupSummary {
        WindowGroupSummary {
            group_kind: self.group_kind,
            name: self.name,
            value: self.value,
            record_count: self.record_count,
            final_count: self.final_count,
            provisional_count: self.provisional_count,
            measured_position_count: self.measured_position_count,
            total_position_length: self.total_position_length,
        }
    }
}

fn summarize_windows(
    windows: impl IntoIterator<Item = WindowRecord>,
    group_kind: WindowGroupKind,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, String> {
    validate_summary_name(name)?;
    let mut groups = BTreeMap::<String, SummaryAccumulator>::new();
    for window in windows {
        for value in metadata_values(&window, group_kind, name) {
            groups
                .entry(primitive_sort_key(&value))
                .or_insert_with(|| SummaryAccumulator::new(group_kind, name, value.clone()))
                .add_window(&window);
        }
    }
    Ok(groups
        .into_values()
        .map(SummaryAccumulator::into_summary)
        .collect())
}

fn summarize_snapshot_records(
    records: &[WindowSnapshotRecord],
    group_kind: WindowGroupKind,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, String> {
    validate_summary_name(name)?;
    let mut groups = BTreeMap::<String, SummaryAccumulator>::new();
    for record in records {
        for value in metadata_values(&record.window, group_kind, name) {
            groups
                .entry(primitive_sort_key(&value))
                .or_insert_with(|| SummaryAccumulator::new(group_kind, name, value.clone()))
                .add_snapshot(record);
        }
    }
    Ok(groups
        .into_values()
        .map(SummaryAccumulator::into_summary)
        .collect())
}

fn validate_summary_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Summary dimension name cannot be empty.".to_owned());
    }
    Ok(())
}

fn metadata_values(
    window: &WindowRecord,
    group_kind: WindowGroupKind,
    name: &str,
) -> Vec<PrimitiveValue> {
    let mut values = Vec::new();
    match group_kind {
        WindowGroupKind::Segment => {
            for segment in window.segments() {
                if segment.name == name && !values.contains(&segment.value) {
                    values.push(segment.value.clone());
                }
            }
        }
        WindowGroupKind::Tag => {
            for tag in window.tags() {
                if tag.name == name && !values.contains(&tag.value) {
                    values.push(tag.value.clone());
                }
            }
        }
    }
    values
}

fn snapshot_record(
    window: WindowRecord,
    horizon: TemporalPoint,
) -> Result<Option<WindowSnapshotRecord>, TemporalRangeError> {
    if window.start().axis() != horizon.axis() || window.start() > horizon {
        return Ok(None);
    }

    match window {
        WindowRecord::Closed(closed) => {
            if closed.range.end() <= horizon {
                Ok(Some(WindowSnapshotRecord {
                    range: closed.range,
                    window: WindowRecord::Closed(closed),
                    finality: ComparisonFinality::Final,
                }))
            } else {
                let range = TemporalRange::new(closed.range.start(), horizon)?;
                Ok(Some(WindowSnapshotRecord {
                    window: WindowRecord::Closed(closed),
                    range,
                    finality: ComparisonFinality::Provisional,
                }))
            }
        }
        WindowRecord::Open(open) => {
            let range = TemporalRange::new(open.start, horizon)?;
            Ok(Some(WindowSnapshotRecord {
                window: WindowRecord::Open(open),
                range,
                finality: ComparisonFinality::Provisional,
            }))
        }
    }
}

fn sort_window_records(windows: &mut [WindowRecord]) {
    windows.sort_by_key(window_sort_key);
}

fn window_sort_key(window: &WindowRecord) -> (String, String, String, String, i64, i64, String) {
    (
        window.window_name().to_owned(),
        window.key().to_owned(),
        window.source().unwrap_or("<null>").to_owned(),
        window.partition().unwrap_or("<null>").to_owned(),
        window.start().magnitude(),
        window.end().map_or(i64::MAX, |point| point.magnitude()),
        window.id().as_str().to_owned(),
    )
}

fn primitive_sort_key(value: &PrimitiveValue) -> String {
    match value {
        PrimitiveValue::String(value) => format!("string:{value}"),
        PrimitiveValue::Integer(value) => format!("integer:{value:020}"),
        PrimitiveValue::Float(value) => format!("float:{value:?}"),
        PrimitiveValue::Bool(value) => format!("bool:{value}"),
        PrimitiveValue::Null => "null:".to_owned(),
    }
}

fn same_closed_scope(first: &ClosedWindow, second: &ClosedWindow) -> bool {
    first.window_name == second.window_name
        && first.key == second.key
        && first.partition == second.partition
}

fn closed_windows_overlap(first: &ClosedWindow, second: &ClosedWindow) -> bool {
    first.range.start().magnitude() < second.range.end().magnitude()
        && second.range.start().magnitude() < first.range.end().magnitude()
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

/// Fixture-oriented builder for compact histories.
#[derive(Clone, Debug, Default)]
pub struct WindowHistoryFixture {
    history: WindowHistory,
    next_record_id: u64,
}

impl WindowHistoryFixture {
    /// Creates a fixture builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            history: WindowHistory::new(),
            next_record_id: 0,
        }
    }

    /// Adds a closed processing-position window.
    pub fn closed_window(
        mut self,
        window_name: impl Into<String>,
        key: impl Into<String>,
        start_position: i64,
        end_position: i64,
        configure: impl FnOnce(WindowHistoryFixtureWindow) -> WindowHistoryFixtureWindow,
    ) -> Result<Self, TemporalRangeError> {
        let metadata = configure(WindowHistoryFixtureWindow::default());
        let id = self.next_id();
        let range = TemporalRange::positions(start_position, end_position)?;
        self.history.push_closed(ClosedWindow {
            id,
            window_name: window_name.into(),
            key: key.into(),
            range,
            known_at: metadata.known_at,
            source: metadata.source,
            partition: metadata.partition,
            segments: metadata.segments,
            tags: metadata.tags,
            boundary_reason: None,
            boundary_changes: Vec::new(),
        });
        Ok(self)
    }

    /// Adds an open processing-position window.
    #[must_use]
    pub fn open_window(
        mut self,
        window_name: impl Into<String>,
        key: impl Into<String>,
        start_position: i64,
        configure: impl FnOnce(WindowHistoryFixtureWindow) -> WindowHistoryFixtureWindow,
    ) -> Self {
        let metadata = configure(WindowHistoryFixtureWindow::default());
        let id = self.next_id();
        self.history.push_open(OpenWindow {
            id,
            window_name: window_name.into(),
            key: key.into(),
            start: TemporalPoint::position(start_position),
            known_at: metadata.known_at,
            source: metadata.source,
            partition: metadata.partition,
            segments: metadata.segments,
            tags: metadata.tags,
        });
        self
    }

    /// Builds the history.
    #[must_use]
    pub fn build(self) -> WindowHistory {
        self.history
    }

    fn next_id(&mut self) -> WindowRecordId {
        let id = WindowRecordId::new(format!("window-{:04}", self.next_record_id));
        self.next_record_id += 1;
        id
    }
}

/// Metadata builder for one fixture window.
#[derive(Clone, Debug, Default)]
pub struct WindowHistoryFixtureWindow {
    known_at: Option<TemporalPoint>,
    source: Option<String>,
    partition: Option<String>,
    segments: Vec<WindowSegment>,
    tags: Vec<WindowTag>,
}

impl WindowHistoryFixtureWindow {
    /// Sets the source/lane.
    #[must_use]
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets the known-at processing position for the window.
    #[must_use]
    pub fn known_at_position(mut self, position: i64) -> Self {
        self.known_at = Some(TemporalPoint::position(position));
        self
    }

    /// Sets the partition.
    #[must_use]
    pub fn partition(mut self, partition: impl Into<String>) -> Self {
        self.partition = Some(partition.into());
        self
    }

    /// Adds a segment.
    #[must_use]
    pub fn segment(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        self.segments.push(WindowSegment::new(name, value));
        self
    }

    /// Adds a segment with parent metadata.
    #[must_use]
    pub fn child_segment(
        mut self,
        name: impl Into<String>,
        value: impl Into<PrimitiveValue>,
        parent_name: impl Into<String>,
    ) -> Self {
        self.segments
            .push(WindowSegment::new(name, value).with_parent(parent_name));
        self
    }

    /// Adds a tag.
    #[must_use]
    pub fn tag(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        self.tags.push(WindowTag::new(name, value));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_builder_creates_closed_windows_with_metadata() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
                    .partition("fleet-a")
                    .segment("lifecycle", "Incident")
                    .child_segment("stage", "Escalated", "lifecycle")
                    .tag("fleet", "critical")
            })
            .expect("valid fixture window")
            .build();

        let window = &history.closed_windows()[0];
        assert_eq!(window.id.as_str(), "window-0000");
        assert_eq!(window.source.as_deref(), Some("provider-a"));
        assert_eq!(window.partition.as_deref(), Some("fleet-a"));
        assert_eq!(window.segments.len(), 2);
        assert_eq!(window.tags.len(), 1);
        assert_eq!(window.range.magnitude(), 4);
    }

    #[test]
    fn fixture_builder_creates_open_windows() {
        let history = WindowHistoryFixture::new()
            .open_window("DeviceOffline", "device-1", 10, |w| w.source("provider-a"))
            .build();

        assert_eq!(history.open_windows().len(), 1);
        assert_eq!(history.open_windows()[0].start, TemporalPoint::position(10));
    }

    #[test]
    fn direct_history_query_filters_and_aliases() {
        let history = segmented_history();

        let rows = history
            .query()
            .where_window("DeviceOffline")
            .where_key("device-1")
            .where_lane("provider-a")
            .where_partition("p1")
            .where_segment("lifecycle", "Incident")
            .where_tag("fleet", "warehouse")
            .closed()
            .all();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source(), Some("provider-a"));
        assert_eq!(
            rows[0].segments()[0].value,
            PrimitiveValue::from("Incident")
        );

        let latest = history
            .query()
            .window("DeviceOffline")
            .lane("provider-a")
            .latest()
            .expect("latest window");
        assert_eq!(latest.key(), "device-2");
    }

    #[test]
    fn snapshot_records_include_final_and_provisional_ranges() {
        let history = segmented_history();
        let snapshot = history
            .snapshot_at(TemporalPoint::position(6))
            .expect("snapshot");
        let rows = snapshot
            .query()
            .where_window("DeviceOffline")
            .where_lane("provider-a")
            .all();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].finality, ComparisonFinality::Final);
        assert_eq!(rows[1].finality, ComparisonFinality::Provisional);
        assert_eq!(rows[1].range.magnitude(), 3);
    }

    #[test]
    fn summaries_group_recorded_and_snapshot_windows() {
        let history = segmented_history();

        let summaries = history
            .query()
            .where_window("DeviceOffline")
            .summarize_by_segment("lifecycle")
            .expect("segment summaries");
        let incident = summaries
            .iter()
            .find(|summary| summary.value == PrimitiveValue::from("Incident"))
            .expect("incident summary");
        assert_eq!(incident.group_kind, WindowGroupKind::Segment);
        assert_eq!(incident.record_count, 2);
        assert_eq!(incident.final_count, 1);
        assert_eq!(incident.provisional_count, 1);
        assert_eq!(incident.measured_position_count, 1);
        assert_eq!(incident.total_position_length, 1);

        let snapshot_summaries = history
            .snapshot_at(TemporalPoint::position(6))
            .expect("snapshot")
            .query()
            .where_window("DeviceOffline")
            .summarize_by_segment("lifecycle")
            .expect("snapshot summaries");
        let snapshot_incident = snapshot_summaries
            .iter()
            .find(|summary| summary.value == PrimitiveValue::from("Incident"))
            .expect("snapshot incident summary");
        assert_eq!(snapshot_incident.measured_position_count, 2);
        assert_eq!(snapshot_incident.total_position_length, 4);

        assert!(history.query().summarize_by_segment("").is_err());
    }

    #[test]
    fn direct_overlap_and_residual_helpers_match_query_surface() {
        let history = WindowHistoryFixture::new()
            .closed_window("SelectionSuspension", "selection-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("SelectionSuspension", "selection-1", 3, 6, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();

        let overlap = history.find_overlaps().remove(0);
        assert_eq!(overlap.first.source.as_deref(), Some("provider-a"));
        assert_eq!(overlap.second.source.as_deref(), Some("provider-b"));

        let residual = history.find_residuals("provider-a").remove(0);
        assert_eq!(residual.start_position, 1);
        assert_eq!(residual.end_position, 3);
    }

    #[test]
    fn annotations_append_revisions_and_filter_by_known_at() {
        let mut history = WindowHistoryFixture::new()
            .open_window("DeviceOffline", "device-1", 1, |w| w.source("lane-a"))
            .build();
        let open = history.query().open_windows()[0].clone();
        let target = WindowAnnotationTarget::from_open(&open);

        let first = history.annotate(target.clone(), "classification", "initial", None);
        let second = history.annotate(
            target.clone(),
            "classification",
            "revised",
            Some(TemporalPoint::position(5)),
        );
        history.annotate(
            target.clone(),
            "classification",
            "future",
            Some(TemporalPoint::position(8)),
        );
        history.annotate(
            target.clone(),
            "timestamp-note",
            "different-axis",
            Some(TemporalPoint::timestamp_ticks(10)),
        );

        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 2);
        assert_eq!(history.annotations_for(&target).len(), 4);

        let known = history.annotations_known_at(&target, TemporalPoint::position(6));
        assert_eq!(known, vec![second]);
    }

    fn segmented_history() -> WindowHistory {
        WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 2, |w| {
                w.source("provider-a")
                    .partition("p1")
                    .segment("lifecycle", "Incident")
                    .tag("fleet", "warehouse")
            })
            .expect("closed provider-a")
            .open_window("DeviceOffline", "device-2", 3, |w| {
                w.source("provider-a")
                    .partition("p1")
                    .segment("lifecycle", "Incident")
                    .tag("fleet", "warehouse")
            })
            .closed_window("DeviceOffline", "device-3", 4, 5, |w| {
                w.source("provider-b")
                    .partition("p1")
                    .segment("lifecycle", "Normal")
                    .tag("fleet", "warehouse")
            })
            .expect("closed provider-b")
            .build()
    }
}
