use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use crate::{
    ComparisonFinality, PrimitiveValue, TemporalAxis, TemporalPoint, TemporalRange,
    TemporalRangeError,
};

/// Deterministic window record identifier.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize)]
pub struct WindowRecordId(String);

impl WindowRecordId {
    /// Creates a new record identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, WindowMetadataError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(WindowMetadataError::EmptyRecordId);
        }
        Ok(Self(value))
    }

    pub(crate) fn generated(value: String) -> Self {
        debug_assert!(!value.trim().is_empty());
        Self(value)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WindowRecordId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WindowRecordId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(serde::de::Error::custom("window record id cannot be empty"));
        }
        Ok(Self(value))
    }
}

/// Analytical segment captured with a window.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowSegment {
    /// Segment name.
    pub(crate) name: String,
    /// Segment value.
    pub(crate) value: PrimitiveValue,
    /// Optional parent segment name.
    pub(crate) parent_name: Option<String>,
}

impl WindowSegment {
    /// Creates a segment.
    pub fn new(
        name: impl Into<String>,
        value: impl Into<PrimitiveValue>,
    ) -> Result<Self, WindowMetadataError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(WindowMetadataError::EmptySegmentName);
        }
        Ok(Self {
            name,
            value: value.into(),
            parent_name: None,
        })
    }

    /// Sets the parent segment name.
    pub fn with_parent(
        mut self,
        parent_name: impl Into<String>,
    ) -> Result<Self, WindowMetadataError> {
        let parent_name = parent_name.into();
        if parent_name.trim().is_empty() {
            return Err(WindowMetadataError::EmptyParentSegmentName);
        }
        self.parent_name = Some(parent_name);
        Ok(self)
    }

    /// Returns the segment name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the segment value.
    #[must_use]
    pub const fn value(&self) -> &PrimitiveValue {
        &self.value
    }

    /// Returns the optional parent segment name.
    #[must_use]
    pub fn parent_name(&self) -> Option<&str> {
        self.parent_name.as_deref()
    }
}

impl<'de> Deserialize<'de> for WindowSegment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawSegment {
            name: String,
            value: PrimitiveValue,
            parent_name: Option<String>,
        }
        let raw = RawSegment::deserialize(deserializer)?;
        if raw.name.trim().is_empty()
            || raw
                .parent_name
                .as_deref()
                .is_some_and(|parent| parent.trim().is_empty())
        {
            return Err(serde::de::Error::custom("segment names cannot be empty"));
        }
        Ok(Self {
            name: raw.name,
            value: raw.value,
            parent_name: raw.parent_name,
        })
    }
}

/// Descriptive tag captured with a window.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowTag {
    /// Tag name.
    pub(crate) name: String,
    /// Tag value.
    pub(crate) value: PrimitiveValue,
}

impl WindowTag {
    /// Creates a tag.
    pub fn new(
        name: impl Into<String>,
        value: impl Into<PrimitiveValue>,
    ) -> Result<Self, WindowMetadataError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(WindowMetadataError::EmptyTagName);
        }
        Ok(Self {
            name,
            value: value.into(),
        })
    }

    /// Returns the tag name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the tag value.
    #[must_use]
    pub const fn value(&self) -> &PrimitiveValue {
        &self.value
    }
}

/// Public window metadata construction error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WindowMetadataError {
    /// Record identifiers cannot be blank.
    #[error("window record id cannot be empty")]
    EmptyRecordId,
    /// Segment names cannot be blank.
    #[error("segment name cannot be empty")]
    EmptySegmentName,
    /// Parent segment names cannot be blank.
    #[error("parent segment name cannot be empty")]
    EmptyParentSegmentName,
    /// Tag names cannot be blank.
    #[error("tag name cannot be empty")]
    EmptyTagName,
}

/// Fixture-history construction error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WindowHistoryFixtureError {
    /// A fixture range was invalid.
    #[error(transparent)]
    Temporal(#[from] TemporalRangeError),
    /// Fixture metadata was invalid.
    #[error(transparent)]
    Metadata(#[from] WindowMetadataError),
}

/// Error returned when importing materialized window history.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WindowHistoryImportError {
    /// The materialized open-window list contains the same record ID twice.
    #[error("duplicate open window record id '{record_id}'")]
    DuplicateOpenRecordId {
        /// The repeated open-window record ID.
        record_id: WindowRecordId,
    },
}

impl<'de> Deserialize<'de> for WindowTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawTag {
            name: String,
            value: PrimitiveValue,
        }
        let raw = RawTag::deserialize(deserializer)?;
        if raw.name.trim().is_empty() {
            return Err(serde::de::Error::custom("tag names cannot be empty"));
        }
        Ok(Self {
            name: raw.name,
            value: raw.value,
        })
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
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowBoundaryChange {
    /// Segment dimension name.
    pub segment_name: String,
    /// Previous segment value, or `None` when the segment was added.
    pub previous_value: Option<PrimitiveValue>,
    /// Current segment value, or `None` when the segment was removed.
    pub current_value: Option<PrimitiveValue>,
}

impl<'de> Deserialize<'de> for WindowBoundaryChange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawBoundaryChange {
            segment_name: String,
            previous_value: Option<PrimitiveValue>,
            current_value: Option<PrimitiveValue>,
        }
        let raw = RawBoundaryChange::deserialize(deserializer)?;
        if raw.segment_name.trim().is_empty() {
            return Err(serde::de::Error::custom(
                "boundary segment names cannot be empty",
            ));
        }
        Ok(Self {
            segment_name: raw.segment_name,
            previous_value: raw.previous_value,
            current_value: raw.current_value,
        })
    }
}

/// Closed state window.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize)]
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
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize)]
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

struct WindowPayload<'a> {
    window_name: &'a str,
    key: &'a str,
    source: Option<&'a str>,
    partition: Option<&'a str>,
    start: &'a TemporalPoint,
    known_at: Option<&'a TemporalPoint>,
    segments: &'a [WindowSegment],
    tags: &'a [WindowTag],
    boundary_changes: &'a [WindowBoundaryChange],
}

fn validate_window_payload(payload: WindowPayload<'_>) -> Result<(), &'static str> {
    if payload.window_name.trim().is_empty() {
        return Err("window name cannot be empty");
    }
    if payload.key.trim().is_empty() {
        return Err("window key cannot be empty");
    }
    if payload.source.is_some_and(|value| value.trim().is_empty()) {
        return Err("source cannot be empty");
    }
    if payload
        .partition
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("partition cannot be empty");
    }
    if let Some(known_at) = payload.known_at
        && !payload.start.is_compatible_with(known_at)
    {
        return Err("known-at point must share the window temporal domain");
    }
    let mut segment_names = BTreeSet::new();
    for segment in payload.segments {
        if !segment_names.insert(segment.name.as_str()) {
            return Err("segment names must be unique within a window");
        }
        if let Some(parent) = segment.parent_name.as_deref()
            && !segment_names.contains(parent)
        {
            return Err("segment parent must precede and reference a captured segment");
        }
    }
    let mut tag_names = BTreeSet::new();
    for tag in payload.tags {
        if !tag_names.insert(tag.name.as_str()) {
            return Err("tag names must be unique within a window");
        }
    }
    if payload
        .boundary_changes
        .iter()
        .any(|change| change.segment_name.trim().is_empty())
    {
        return Err("boundary segment names cannot be empty");
    }
    Ok(())
}

impl<'de> Deserialize<'de> for ClosedWindow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawClosedWindow {
            id: WindowRecordId,
            window_name: String,
            key: String,
            range: TemporalRange,
            known_at: Option<TemporalPoint>,
            source: Option<String>,
            partition: Option<String>,
            segments: Vec<WindowSegment>,
            tags: Vec<WindowTag>,
            boundary_reason: Option<WindowBoundaryReason>,
            boundary_changes: Vec<WindowBoundaryChange>,
        }
        let raw = RawClosedWindow::deserialize(deserializer)?;
        let start = raw.range.start_ref();
        validate_window_payload(WindowPayload {
            window_name: &raw.window_name,
            key: &raw.key,
            source: raw.source.as_deref(),
            partition: raw.partition.as_deref(),
            start,
            known_at: raw.known_at.as_ref(),
            segments: &raw.segments,
            tags: &raw.tags,
            boundary_changes: &raw.boundary_changes,
        })
        .map_err(serde::de::Error::custom)?;
        Ok(Self {
            id: raw.id,
            window_name: raw.window_name,
            key: raw.key,
            range: raw.range,
            known_at: raw.known_at,
            source: raw.source,
            partition: raw.partition,
            segments: raw.segments,
            tags: raw.tags,
            boundary_reason: raw.boundary_reason,
            boundary_changes: raw.boundary_changes,
        })
    }
}

impl<'de> Deserialize<'de> for OpenWindow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawOpenWindow {
            id: WindowRecordId,
            window_name: String,
            key: String,
            start: TemporalPoint,
            known_at: Option<TemporalPoint>,
            source: Option<String>,
            partition: Option<String>,
            segments: Vec<WindowSegment>,
            tags: Vec<WindowTag>,
        }
        let raw = RawOpenWindow::deserialize(deserializer)?;
        validate_window_payload(WindowPayload {
            window_name: &raw.window_name,
            key: &raw.key,
            source: raw.source.as_deref(),
            partition: raw.partition.as_deref(),
            start: &raw.start,
            known_at: raw.known_at.as_ref(),
            segments: &raw.segments,
            tags: &raw.tags,
            boundary_changes: &[],
        })
        .map_err(serde::de::Error::custom)?;
        Ok(Self {
            id: raw.id,
            window_name: raw.window_name,
            key: raw.key,
            start: raw.start,
            known_at: raw.known_at,
            source: raw.source,
            partition: raw.partition,
            segments: raw.segments,
            tags: raw.tags,
        })
    }
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
            Self::Open(window) => window.start.clone(),
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
    /// Temporal axis governing the positions.
    pub axis: TemporalAxis,
    /// Timestamp clock identity, when applicable.
    pub clock: Option<String>,
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
    /// Temporal axis governing `start_position`.
    pub axis: TemporalAxis,
    /// Timestamp clock identity, when applicable.
    pub clock: Option<String>,
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
            axis: window.start().axis(),
            clock: window.start().clock().map(str::to_owned),
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
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct WindowHistory {
    closed: Vec<ClosedWindow>,
    open: Vec<OpenWindow>,
    #[serde(skip)]
    open_indexes: BTreeMap<WindowRecordId, usize>,
    annotations: Vec<WindowAnnotation>,
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
    fn new(history: &'a WindowHistory) -> Self {
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
            .filter(|record| record.finality == ComparisonFinality::Final)
            .cloned()
            .collect()
    }

    /// Materializes provisional snapshot records.
    #[must_use]
    pub fn open_windows(&self) -> Vec<WindowSnapshotRecord> {
        self.records
            .iter()
            .filter(|record| self.spec.matches(*record))
            .filter(|record| record.finality == ComparisonFinality::Provisional)
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

/// Summarizes recorded windows by segment.
pub fn summarize_by_segment(
    windows: impl IntoIterator<Item = WindowRecord>,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
    summarize_windows(windows, WindowGroupKind::Segment, name)
}

/// Summarizes recorded windows by tag.
pub fn summarize_by_tag(
    windows: impl IntoIterator<Item = WindowRecord>,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
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
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
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
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
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

/// Error returned when a summary dimension is invalid.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum SummaryError {
    /// A segment or tag name must contain non-whitespace text.
    #[error("summary dimension name cannot be empty")]
    EmptyName,
}

fn validate_summary_name(name: &str) -> Result<(), SummaryError> {
    if name.trim().is_empty() {
        return Err(SummaryError::EmptyName);
    }
    Ok(())
}

fn metadata_values(
    window: &WindowRecord,
    group_kind: WindowGroupKind,
    name: &str,
) -> Vec<PrimitiveValue> {
    let mut values = BTreeMap::<String, PrimitiveValue>::new();
    match group_kind {
        WindowGroupKind::Segment => {
            for segment in window.segments() {
                if segment.name == name {
                    let value = segment.value.clone();
                    values.entry(primitive_sort_key(&value)).or_insert(value);
                }
            }
        }
        WindowGroupKind::Tag => {
            for tag in window.tags() {
                if tag.name == name {
                    let value = tag.value.clone();
                    values.entry(primitive_sort_key(&value)).or_insert(value);
                }
            }
        }
    }
    values.into_values().collect()
}

fn snapshot_record(
    window: WindowRecord,
    horizon: TemporalPoint,
) -> Result<Option<WindowSnapshotRecord>, TemporalRangeError> {
    let start = window.start();
    if start.axis() != horizon.axis() || matches!(start.try_cmp(&horizon), Ok(Ordering::Greater)) {
        return Ok(None);
    }

    match window {
        WindowRecord::Closed(closed) => {
            if matches!(
                closed.range.end().try_cmp(&horizon),
                Ok(Ordering::Less | Ordering::Equal)
            ) {
                Ok(Some(WindowSnapshotRecord {
                    range: closed.range.clone(),
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
            let range = TemporalRange::new(open.start.clone(), horizon.clone())?;
            Ok(Some(WindowSnapshotRecord {
                window: WindowRecord::Open(open),
                range,
                finality: ComparisonFinality::Provisional,
            }))
        }
    }
}

fn sort_window_records(windows: &mut [WindowRecord]) {
    windows.sort_by(compare_window_records);
}

fn compare_window_records(left: &WindowRecord, right: &WindowRecord) -> Ordering {
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

fn primitive_sort_key(value: &PrimitiveValue) -> String {
    match value {
        PrimitiveValue::String(value) => format!("string:{value}"),
        PrimitiveValue::Integer(value) => format!("integer:{value:020}"),
        PrimitiveValue::Float(value) => format!("float:{:?}", value.as_f64()),
        PrimitiveValue::Bool(value) => format!("bool:{value}"),
        PrimitiveValue::Null => "null:".to_owned(),
    }
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
    ) -> Result<Self, WindowHistoryFixtureError> {
        let metadata = configure(WindowHistoryFixtureWindow::default());
        if let Some(error) = metadata.error {
            return Err(error.into());
        }
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
    pub fn open_window(
        mut self,
        window_name: impl Into<String>,
        key: impl Into<String>,
        start_position: i64,
        configure: impl FnOnce(WindowHistoryFixtureWindow) -> WindowHistoryFixtureWindow,
    ) -> Result<Self, WindowHistoryFixtureError> {
        let metadata = configure(WindowHistoryFixtureWindow::default());
        if let Some(error) = metadata.error {
            return Err(error.into());
        }
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
        Ok(self)
    }

    /// Builds the history.
    #[must_use]
    pub fn build(self) -> WindowHistory {
        self.history
    }

    fn next_id(&mut self) -> WindowRecordId {
        let id = WindowRecordId::generated(format!("window-{:04}", self.next_record_id));
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
    error: Option<WindowMetadataError>,
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
        match WindowSegment::new(name, value) {
            Ok(segment) => self.segments.push(segment),
            Err(error) if self.error.is_none() => self.error = Some(error),
            Err(_) => {}
        }
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
        match WindowSegment::new(name, value).and_then(|segment| segment.with_parent(parent_name)) {
            Ok(segment) => self.segments.push(segment),
            Err(error) if self.error.is_none() => self.error = Some(error),
            Err(_) => {}
        }
        self
    }

    /// Adds a tag.
    #[must_use]
    pub fn tag(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        match WindowTag::new(name, value) {
            Ok(tag) => self.tags.push(tag),
            Err(error) if self.error.is_none() => self.error = Some(error),
            Err(_) => {}
        }
        self
    }
}
#[cfg(test)]
#[path = "records_tests.rs"]
mod tests;
