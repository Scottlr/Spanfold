use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

use crate::{PrimitiveValue, TemporalAxis, TemporalPoint, TemporalRange, TemporalRangeError};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowPayloadValidationError {
    EmptyWindowName,
    EmptyWindowKey,
    EmptySource,
    EmptyPartition,
    DuplicateSegmentName {
        name: String,
    },
    InvalidSegmentParent {
        segment_name: String,
        parent_name: String,
    },
    DuplicateTagName {
        name: String,
    },
}

impl std::fmt::Display for WindowPayloadValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyWindowName => formatter.write_str("window name cannot be empty"),
            Self::EmptyWindowKey => formatter.write_str("window key cannot be empty"),
            Self::EmptySource => formatter.write_str("source cannot be empty"),
            Self::EmptyPartition => formatter.write_str("partition cannot be empty"),
            Self::DuplicateSegmentName { .. } => {
                formatter.write_str("segment names must be unique within a window")
            }
            Self::InvalidSegmentParent { .. } => {
                formatter.write_str("segment parent must precede and reference a captured segment")
            }
            Self::DuplicateTagName { .. } => {
                formatter.write_str("tag names must be unique within a window")
            }
        }
    }
}

pub(crate) fn validate_window_metadata(
    window_name: &str,
    key: &str,
    source: Option<&str>,
    partition: Option<&str>,
    segments: &[WindowSegment],
    tags: &[WindowTag],
) -> Result<(), WindowPayloadValidationError> {
    if window_name.trim().is_empty() {
        return Err(WindowPayloadValidationError::EmptyWindowName);
    }
    if key.trim().is_empty() {
        return Err(WindowPayloadValidationError::EmptyWindowKey);
    }
    if source.is_some_and(|value| value.trim().is_empty()) {
        return Err(WindowPayloadValidationError::EmptySource);
    }
    if partition.is_some_and(|value| value.trim().is_empty()) {
        return Err(WindowPayloadValidationError::EmptyPartition);
    }
    validate_window_segments(segments)?;
    validate_window_tags(tags)
}

pub(crate) fn validate_window_segments(
    segments: &[WindowSegment],
) -> Result<(), WindowPayloadValidationError> {
    let mut segment_names = BTreeSet::new();
    for segment in segments {
        if !segment_names.insert(segment.name.as_str()) {
            return Err(WindowPayloadValidationError::DuplicateSegmentName {
                name: segment.name.clone(),
            });
        }
        if let Some(parent) = segment.parent_name.as_deref()
            && !segment_names.contains(parent)
        {
            return Err(WindowPayloadValidationError::InvalidSegmentParent {
                segment_name: segment.name.clone(),
                parent_name: parent.to_owned(),
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_window_tags(tags: &[WindowTag]) -> Result<(), WindowPayloadValidationError> {
    let mut tag_names = BTreeSet::new();
    for tag in tags {
        if !tag_names.insert(tag.name.as_str()) {
            return Err(WindowPayloadValidationError::DuplicateTagName {
                name: tag.name.clone(),
            });
        }
    }
    Ok(())
}

fn validate_window_payload(payload: WindowPayload<'_>) -> Result<(), &'static str> {
    validate_window_metadata(
        payload.window_name,
        payload.key,
        payload.source,
        payload.partition,
        payload.segments,
        payload.tags,
    )
    .map_err(|error| match error {
        WindowPayloadValidationError::EmptyWindowName => "window name cannot be empty",
        WindowPayloadValidationError::EmptyWindowKey => "window key cannot be empty",
        WindowPayloadValidationError::EmptySource => "source cannot be empty",
        WindowPayloadValidationError::EmptyPartition => "partition cannot be empty",
        WindowPayloadValidationError::DuplicateSegmentName { .. } => {
            "segment names must be unique within a window"
        }
        WindowPayloadValidationError::InvalidSegmentParent { .. } => {
            "segment parent must precede and reference a captured segment"
        }
        WindowPayloadValidationError::DuplicateTagName { .. } => {
            "tag names must be unique within a window"
        }
    })?;
    if let Some(known_at) = payload.known_at
        && !payload.start.is_compatible_with(known_at)
    {
        return Err("known-at point must share the window temporal domain");
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

/// Finality state for a records-owned window snapshot.
///
/// Snapshot records intentionally describe only whether the visible range is
/// complete at the requested horizon. Comparison lifecycle states such as
/// revision and retraction belong to comparison-owned result metadata and are
/// introduced only when a higher layer translates these records.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WindowSnapshotFinality {
    /// The source window is complete at the snapshot horizon.
    Final,
    /// The visible range depends on the snapshot horizon.
    Provisional,
}

/// A snapshot view of one window at an explicit horizon.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowSnapshotRecord {
    /// Source window.
    pub window: WindowRecord,
    /// Visible range at the snapshot horizon.
    pub range: TemporalRange,
    /// Whether the visible range is final or provisional.
    pub finality: WindowSnapshotFinality,
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
