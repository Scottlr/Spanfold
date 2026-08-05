use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::records::{
    WindowPayloadValidationError, validate_window_metadata, validate_window_segments,
    validate_window_tags,
};
use crate::{
    ClosedWindow, OpenWindow, TemporalPoint, TemporalRange, TemporalRangeError,
    WindowBoundaryChange, WindowBoundaryReason, WindowHistory, WindowRecordId, WindowSegment,
    WindowTag,
};

/// Window transition kind returned by the event-to-window recorder.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WindowTransitionKind {
    /// A window opened.
    Opened,
    /// A window closed.
    Closed,
}

/// Explicit observation supplied to [`WindowRecorder`].
#[derive(Clone, Debug, PartialEq)]
pub struct WindowObservation {
    window_name: String,
    key: String,
    source: Option<String>,
    partition: Option<String>,
    state_context: String,
    point: TemporalPoint,
    active: bool,
    segments: Vec<WindowSegment>,
    tags: Vec<WindowTag>,
}

impl WindowObservation {
    /// Creates an observation for one logical window state.
    #[must_use = "handle invalid observation metadata"]
    pub fn new(
        window_name: impl Into<String>,
        key: impl Into<String>,
        point: TemporalPoint,
        active: bool,
    ) -> Result<Self, WindowRecorderError> {
        let observation = Self {
            window_name: window_name.into(),
            key: key.into(),
            source: None,
            partition: None,
            state_context: String::new(),
            point,
            active,
            segments: Vec::new(),
            tags: Vec::new(),
        };
        observation.validate()?;
        Ok(observation)
    }

    /// Adds an optional source and partition scope.
    pub fn with_scope(
        mut self,
        source: Option<String>,
        partition: Option<String>,
    ) -> Result<Self, WindowRecorderError> {
        self.source = source;
        self.partition = partition;
        self.validate()?;
        Ok(self)
    }

    /// Sets the private state context used when multiple lanes share an identity.
    #[must_use]
    pub(crate) fn with_state_context(mut self, state_context: impl Into<String>) -> Self {
        self.state_context = state_context.into();
        self
    }

    /// Attaches analytical segments to the observation.
    pub fn with_segments(
        mut self,
        segments: Vec<WindowSegment>,
    ) -> Result<Self, WindowRecorderError> {
        validate_window_segments(&segments).map_err(WindowRecorderError::from)?;
        self.segments = segments;
        Ok(self)
    }

    /// Attaches descriptive tags to the observation.
    pub fn with_tags(mut self, tags: Vec<WindowTag>) -> Result<Self, WindowRecorderError> {
        validate_window_tags(&tags).map_err(WindowRecorderError::from)?;
        self.tags = tags;
        Ok(self)
    }

    fn validate(&self) -> Result<(), WindowRecorderError> {
        Self::validate_parts(
            &self.window_name,
            &self.key,
            self.source.as_deref(),
            self.partition.as_deref(),
            &self.segments,
            &self.tags,
        )
        .map_err(WindowRecorderError::from)
    }

    pub(crate) fn validate_parts(
        window_name: &str,
        key: &str,
        source: Option<&str>,
        partition: Option<&str>,
        segments: &[WindowSegment],
        tags: &[WindowTag],
    ) -> Result<(), crate::records::WindowPayloadValidationError> {
        validate_window_metadata(window_name, key, source, partition, segments, tags)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_validated_parts(
        window_name: String,
        key: String,
        point: TemporalPoint,
        active: bool,
        source: Option<String>,
        partition: Option<String>,
        state_context: String,
        segments: Vec<WindowSegment>,
        tags: Vec<WindowTag>,
    ) -> Self {
        debug_assert!(
            Self::validate_parts(
                &window_name,
                &key,
                source.as_deref(),
                partition.as_deref(),
                &segments,
                &tags,
            )
            .is_ok()
        );
        let observation = Self {
            window_name,
            key,
            source,
            partition,
            state_context: String::new(),
            point,
            active,
            segments,
            tags,
        };
        observation.with_state_context(state_context)
    }
}

/// One transition emitted by [`WindowRecorder`].
#[derive(Clone, Debug, PartialEq)]
pub struct WindowRecorderTransition {
    /// Transition kind.
    pub kind: WindowTransitionKind,
    /// Window family name.
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Deterministic record identifier.
    pub record_id: WindowRecordId,
    /// Explicit transition point.
    pub point: TemporalPoint,
    /// Optional source/lane.
    pub source: Option<String>,
    /// Optional partition.
    pub partition: Option<String>,
    /// Analytical segments attached to the transition.
    pub segments: Vec<WindowSegment>,
    /// Descriptive tags attached to the transition.
    pub tags: Vec<WindowTag>,
    /// Reason a closed boundary was emitted, when known.
    pub boundary_reason: Option<WindowBoundaryReason>,
    /// Segment changes that caused this boundary.
    pub boundary_changes: Vec<WindowBoundaryChange>,
}

/// Error returned when an explicit observation cannot be recorded.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WindowRecorderError {
    /// The window family name is blank.
    #[error("window name cannot be empty")]
    EmptyWindowName,
    /// The logical window key is blank.
    #[error("window key cannot be empty")]
    EmptyWindowKey,
    /// The optional source is present but blank.
    #[error("source cannot be empty")]
    EmptySource,
    /// The optional partition is present but blank.
    #[error("partition cannot be empty")]
    EmptyPartition,
    /// A segment name occurs more than once.
    #[error("segment name '{name}' is duplicated")]
    DuplicateSegmentName {
        /// Repeated segment name.
        name: String,
    },
    /// A segment refers to a parent that has not already been captured.
    #[error(
        "segment '{segment_name}' parent '{parent_name}' must precede and reference a captured segment"
    )]
    InvalidSegmentParent {
        /// Child segment name.
        segment_name: String,
        /// Referenced parent segment name.
        parent_name: String,
    },
    /// A tag name occurs more than once.
    #[error("tag name '{name}' is duplicated")]
    DuplicateTagName {
        /// Repeated tag name.
        name: String,
    },
    /// The observation point is incompatible with an active window.
    #[error(transparent)]
    Temporal(#[from] TemporalRangeError),
    /// The deterministic record identifier counter cannot advance.
    #[error("window record id overflow")]
    RecordIdOverflow,
}

impl From<WindowPayloadValidationError> for WindowRecorderError {
    fn from(error: WindowPayloadValidationError) -> Self {
        match error {
            WindowPayloadValidationError::EmptyWindowName => Self::EmptyWindowName,
            WindowPayloadValidationError::EmptyWindowKey => Self::EmptyWindowKey,
            WindowPayloadValidationError::EmptySource => Self::EmptySource,
            WindowPayloadValidationError::EmptyPartition => Self::EmptyPartition,
            WindowPayloadValidationError::DuplicateSegmentName { name } => {
                Self::DuplicateSegmentName { name }
            }
            WindowPayloadValidationError::InvalidSegmentParent {
                segment_name,
                parent_name,
            } => Self::InvalidSegmentParent {
                segment_name,
                parent_name,
            },
            WindowPayloadValidationError::DuplicateTagName { name } => {
                Self::DuplicateTagName { name }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WindowStateKey {
    window_name: String,
    key: String,
    source: Option<String>,
    partition: Option<String>,
    state_context: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowRecorderActiveState {
    pub(crate) id: WindowRecordId,
    pub(crate) start: TemporalPoint,
    pub(crate) segments: Vec<WindowSegment>,
    pub(crate) tags: Vec<WindowTag>,
}

/// Concrete synchronous event-to-window lifecycle recorder.
pub struct WindowRecorder {
    record_windows: bool,
    history: WindowHistory,
    active: HashMap<WindowStateKey, WindowRecorderActiveState>,
    id_prefix: String,
    next_record_id: u64,
}

impl WindowRecorder {
    /// Creates a recorder with deterministic `window-####` record identifiers.
    #[must_use]
    pub fn new(record_windows: bool) -> Self {
        Self::with_id_prefix(record_windows, "window")
    }

    pub(crate) fn with_id_prefix(record_windows: bool, id_prefix: &str) -> Self {
        Self {
            record_windows,
            history: WindowHistory::new(),
            active: HashMap::new(),
            id_prefix: id_prefix.to_owned(),
            next_record_id: 0,
        }
    }

    /// Returns the recorded history. When recording is disabled, this remains empty.
    #[must_use]
    pub fn history(&self) -> &WindowHistory {
        &self.history
    }

    /// Records one explicit observation and returns any lifecycle transitions.
    pub fn observe(
        &mut self,
        observation: WindowObservation,
    ) -> Result<Vec<WindowRecorderTransition>, WindowRecorderError> {
        self.validate_observation(&observation)?;
        self.observe_validated(observation)
    }

    pub(crate) fn observe_validated(
        &mut self,
        observation: WindowObservation,
    ) -> Result<Vec<WindowRecorderTransition>, WindowRecorderError> {
        debug_assert!(self.validate_observation(&observation).is_ok());
        self.observe_inner(observation)
    }

    fn observe_inner(
        &mut self,
        observation: WindowObservation,
    ) -> Result<Vec<WindowRecorderTransition>, WindowRecorderError> {
        let state_key = WindowStateKey {
            window_name: observation.window_name.clone(),
            key: observation.key.clone(),
            source: observation.source.clone(),
            partition: observation.partition.clone(),
            state_context: observation.state_context.clone(),
        };
        let previous = self.active.get(&state_key).cloned();

        if let Some(previous) = previous {
            let range = TemporalRange::new(previous.start.clone(), observation.point.clone())?;
            if observation.active {
                if previous.segments == observation.segments {
                    if previous.tags != observation.tags {
                        if let Some(state) = self.active.get_mut(&state_key) {
                            state.tags = observation.tags.clone();
                        }
                        if self.record_windows {
                            self.history
                                .update_open_tags(&previous.id, observation.tags.clone());
                        }
                    }
                    return Ok(Vec::new());
                }

                self.ensure_capacity(1)?;
                let boundary_changes = segment_changes(&previous.segments, &observation.segments);
                let closed = self.close_active(
                    &state_key,
                    &observation,
                    range,
                    Some(WindowBoundaryReason::SegmentChanged),
                    boundary_changes,
                );
                let opened = self.open_active(state_key, observation)?;
                return Ok(vec![closed, opened]);
            }

            return Ok(vec![self.close_active(
                &state_key,
                &observation,
                range,
                Some(WindowBoundaryReason::ActivePredicateEnded),
                Vec::new(),
            )]);
        }

        if !observation.active {
            return Ok(Vec::new());
        }

        self.ensure_capacity(1)?;
        Ok(vec![self.open_active(state_key, observation)?])
    }

    pub(crate) fn validate_observation(
        &self,
        observation: &WindowObservation,
    ) -> Result<(), WindowRecorderError> {
        observation.validate()
    }

    pub(crate) fn validate_parts(
        &self,
        window_name: &str,
        key: &str,
        source: Option<&str>,
        partition: Option<&str>,
        segments: &[WindowSegment],
        tags: &[WindowTag],
    ) -> Result<(), WindowRecorderError> {
        WindowObservation::validate_parts(window_name, key, source, partition, segments, tags)
            .map_err(WindowRecorderError::from)
    }

    pub(crate) fn is_active(
        &self,
        window_name: &str,
        key: &str,
        source: Option<&str>,
        partition: Option<&str>,
        state_context: &str,
    ) -> bool {
        self.active_state(window_name, key, source, partition, state_context)
            .is_some()
    }

    pub(crate) fn active_state(
        &self,
        window_name: &str,
        key: &str,
        source: Option<&str>,
        partition: Option<&str>,
        state_context: &str,
    ) -> Option<WindowRecorderActiveState> {
        self.active
            .get(&WindowStateKey {
                window_name: window_name.to_owned(),
                key: key.to_owned(),
                source: source.map(str::to_owned),
                partition: partition.map(str::to_owned),
                state_context: state_context.to_owned(),
            })
            .cloned()
    }

    pub(crate) fn validate_point(&self, point: &TemporalPoint) -> Result<(), WindowRecorderError> {
        for active in self.active.values() {
            TemporalRange::new(active.start.clone(), point.clone())?;
        }
        Ok(())
    }

    pub(crate) fn ensure_capacity(
        &self,
        additional_records: u64,
    ) -> Result<(), WindowRecorderError> {
        if self.next_record_id > u64::MAX.saturating_sub(additional_records) {
            return Err(WindowRecorderError::RecordIdOverflow);
        }
        Ok(())
    }

    fn open_active(
        &mut self,
        state_key: WindowStateKey,
        observation: WindowObservation,
    ) -> Result<WindowRecorderTransition, WindowRecorderError> {
        let record_id = self.next_id()?;
        let active = WindowRecorderActiveState {
            id: record_id.clone(),
            start: observation.point.clone(),
            segments: observation.segments.clone(),
            tags: observation.tags.clone(),
        };
        if self.record_windows {
            self.history.push_open(OpenWindow {
                id: record_id.clone(),
                window_name: observation.window_name.clone(),
                key: observation.key.clone(),
                start: observation.point.clone(),
                known_at: None,
                source: observation.source.clone(),
                partition: observation.partition.clone(),
                segments: observation.segments.clone(),
                tags: observation.tags.clone(),
            });
        }
        self.active.insert(state_key, active);
        Ok(WindowRecorderTransition {
            kind: WindowTransitionKind::Opened,
            window_name: observation.window_name,
            key: observation.key,
            record_id,
            point: observation.point,
            source: observation.source,
            partition: observation.partition,
            segments: observation.segments,
            tags: observation.tags,
            boundary_reason: None,
            boundary_changes: Vec::new(),
        })
    }

    fn close_active(
        &mut self,
        state_key: &WindowStateKey,
        observation: &WindowObservation,
        range: TemporalRange,
        boundary_reason: Option<WindowBoundaryReason>,
        boundary_changes: Vec<WindowBoundaryChange>,
    ) -> WindowRecorderTransition {
        let previous = self
            .active
            .remove(state_key)
            .expect("active recorder state was validated before closing");
        if self.record_windows {
            self.history.remove_open(&previous.id);
            self.history.push_closed(ClosedWindow {
                id: previous.id.clone(),
                window_name: observation.window_name.clone(),
                key: observation.key.clone(),
                range,
                known_at: None,
                source: observation.source.clone(),
                partition: observation.partition.clone(),
                segments: previous.segments.clone(),
                tags: previous.tags.clone(),
                boundary_reason,
                boundary_changes: boundary_changes.clone(),
            });
        }
        WindowRecorderTransition {
            kind: WindowTransitionKind::Closed,
            window_name: observation.window_name.clone(),
            key: observation.key.clone(),
            record_id: previous.id,
            point: observation.point.clone(),
            source: observation.source.clone(),
            partition: observation.partition.clone(),
            segments: previous.segments,
            tags: previous.tags,
            boundary_reason,
            boundary_changes,
        }
    }

    fn next_id(&mut self) -> Result<WindowRecordId, WindowRecorderError> {
        let id =
            WindowRecordId::generated(format!("{}-{:04}", self.id_prefix, self.next_record_id));
        self.next_record_id = self
            .next_record_id
            .checked_add(1)
            .ok_or(WindowRecorderError::RecordIdOverflow)?;
        Ok(id)
    }
}

fn segment_changes(
    previous: &[WindowSegment],
    current: &[WindowSegment],
) -> Vec<WindowBoundaryChange> {
    let mut changes = Vec::new();
    let mut names = BTreeSet::new();
    names.extend(previous.iter().map(|segment| segment.name()));
    names.extend(current.iter().map(|segment| segment.name()));
    for name in names {
        let before = previous.iter().find(|segment| segment.name() == name);
        let after = current.iter().find(|segment| segment.name() == name);
        if before == after {
            continue;
        }
        changes.push(WindowBoundaryChange {
            segment_name: name.to_owned(),
            previous_value: before.map(|segment| segment.value().clone()),
            current_value: after.map(|segment| segment.value().clone()),
        });
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(name: &str, value: &str) -> WindowSegment {
        WindowSegment::new(name, value).expect("segment")
    }

    fn tag(name: &str, value: &str) -> WindowTag {
        WindowTag::new(name, value).expect("tag")
    }

    fn observation(
        window_name: &str,
        key: &str,
        point: TemporalPoint,
        active: bool,
    ) -> WindowObservation {
        WindowObservation::new(window_name, key, point, active).expect("observation")
    }

    #[test]
    fn explicit_points_preserve_processing_and_timestamp_domains() {
        let mut processing = WindowRecorder::new(true);
        processing
            .observe(observation(
                "window",
                "key",
                TemporalPoint::position(4),
                true,
            ))
            .expect("open processing window");
        processing
            .observe(observation(
                "window",
                "key",
                TemporalPoint::position(9),
                false,
            ))
            .expect("close processing window");
        let processing_window = &processing.history().closed_windows()[0];
        assert_eq!(processing_window.range.start(), TemporalPoint::position(4));
        assert_eq!(processing_window.range.end(), TemporalPoint::position(9));

        let mut timestamp = WindowRecorder::new(true);
        let start = TemporalPoint::timestamp_ticks_with_clock(100, "clock-a");
        let end = TemporalPoint::timestamp_ticks_with_clock(125, "clock-a");
        timestamp
            .observe(observation("window", "key", start.clone(), true))
            .expect("open timestamp window");
        let transition = timestamp
            .observe(observation("window", "key", end.clone(), false))
            .expect("close timestamp window");
        assert_eq!(transition[0].point, end);
        let timestamp_window = &timestamp.history().closed_windows()[0];
        assert_eq!(timestamp_window.range.start(), start);
        assert_eq!(timestamp_window.range.end(), transition[0].point);
        assert_eq!(timestamp_window.range.start().clock(), Some("clock-a"));
    }

    #[test]
    fn unchanged_segments_update_tags_without_reopening() {
        let mut recorder = WindowRecorder::new(true);
        let first = recorder
            .observe(
                observation("window", "key", TemporalPoint::position(1), true)
                    .with_segments(vec![segment("state", "open")])
                    .expect("segments")
                    .with_tags(vec![tag("label", "first")])
                    .expect("tags"),
            )
            .expect("open window");
        let second = recorder
            .observe(
                observation("window", "key", TemporalPoint::position(2), true)
                    .with_segments(vec![segment("state", "open")])
                    .expect("segments")
                    .with_tags(vec![tag("label", "second")])
                    .expect("tags"),
            )
            .expect("update tags");

        assert_eq!(first.len(), 1);
        assert!(second.is_empty());
        assert_eq!(recorder.history().open_windows().len(), 1);
        assert_eq!(
            recorder.history().open_windows()[0].tags[0].value(),
            &crate::PrimitiveValue::from("second")
        );
        assert_eq!(recorder.history().open_windows()[0].id, first[0].record_id);
    }

    #[test]
    fn segment_changes_close_and_reopen_with_boundary_metadata() {
        let mut recorder = WindowRecorder::new(true);
        recorder
            .observe(
                observation("window", "key", TemporalPoint::position(1), true)
                    .with_segments(vec![segment("state", "open")])
                    .expect("segments"),
            )
            .expect("open window");
        let transitions = recorder
            .observe(
                observation("window", "key", TemporalPoint::position(3), true)
                    .with_segments(vec![segment("state", "closed")])
                    .expect("segments"),
            )
            .expect("reopen changed segment");

        assert_eq!(
            transitions.iter().map(|item| item.kind).collect::<Vec<_>>(),
            vec![WindowTransitionKind::Closed, WindowTransitionKind::Opened]
        );
        assert_eq!(
            transitions[0].boundary_reason,
            Some(WindowBoundaryReason::SegmentChanged)
        );
        assert_eq!(transitions[0].boundary_changes[0].segment_name, "state");
        assert_eq!(recorder.history().closed_windows().len(), 1);
        assert_eq!(recorder.history().open_windows().len(), 1);
    }

    #[test]
    fn inactive_observation_closes_active_window() {
        let mut recorder = WindowRecorder::new(true);
        recorder
            .observe(observation(
                "window",
                "key",
                TemporalPoint::position(1),
                true,
            ))
            .expect("open window");
        let transitions = recorder
            .observe(observation(
                "window",
                "key",
                TemporalPoint::position(4),
                false,
            ))
            .expect("close window");

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].kind, WindowTransitionKind::Closed);
        assert_eq!(
            transitions[0].boundary_reason,
            Some(WindowBoundaryReason::ActivePredicateEnded)
        );
        assert!(recorder.history().open_windows().is_empty());
        assert_eq!(recorder.history().closed_windows().len(), 1);
    }

    #[test]
    fn incompatible_points_leave_active_state_unchanged() {
        let mut recorder = WindowRecorder::new(true);
        recorder
            .observe(observation(
                "window",
                "key",
                TemporalPoint::timestamp_ticks_with_clock(10, "clock-a"),
                true,
            ))
            .expect("open window");
        let error = recorder
            .observe(observation(
                "window",
                "key",
                TemporalPoint::timestamp_ticks_with_clock(11, "clock-b"),
                false,
            ))
            .expect_err("clock mismatch should fail");

        assert!(matches!(error, WindowRecorderError::Temporal(_)));
        assert!(recorder.history().closed_windows().is_empty());
        assert_eq!(recorder.history().open_windows().len(), 1);
        let transitions = recorder
            .observe(observation(
                "window",
                "key",
                TemporalPoint::timestamp_ticks_with_clock(12, "clock-a"),
                false,
            ))
            .expect("original clock still closes");
        assert_eq!(transitions[0].kind, WindowTransitionKind::Closed);
    }

    #[test]
    fn invalid_metadata_is_typed_before_configuration_is_committed() {
        assert!(matches!(
            WindowObservation::new(" ", "key", TemporalPoint::position(1), true),
            Err(WindowRecorderError::EmptyWindowName)
        ));
        assert!(matches!(
            WindowObservation::new("window", " ", TemporalPoint::position(1), true),
            Err(WindowRecorderError::EmptyWindowKey)
        ));

        let base = observation("window", "key", TemporalPoint::position(1), true);
        assert!(matches!(
            base.clone().with_scope(Some(" ".to_owned()), None),
            Err(WindowRecorderError::EmptySource)
        ));
        assert!(matches!(
            base.clone().with_scope(None, Some(" ".to_owned())),
            Err(WindowRecorderError::EmptyPartition)
        ));
        let duplicate_segment = vec![segment("state", "open"), segment("state", "closed")];
        assert!(matches!(
            base.clone().with_segments(duplicate_segment),
            Err(WindowRecorderError::DuplicateSegmentName { .. })
        ));
        let invalid_parent = vec![
            segment("child", "open")
                .with_parent("missing")
                .expect("parent metadata"),
        ];
        assert!(matches!(
            base.clone().with_segments(invalid_parent),
            Err(WindowRecorderError::InvalidSegmentParent { .. })
        ));
        let duplicate_tag = vec![tag("label", "one"), tag("label", "two")];
        assert!(matches!(
            base.with_tags(duplicate_tag),
            Err(WindowRecorderError::DuplicateTagName { .. })
        ));
    }

    #[test]
    fn observe_defense_rejects_invalid_metadata_without_advancing_state() {
        let mut recorder = WindowRecorder::new(true);
        let invalid = WindowObservation {
            window_name: " ".to_owned(),
            key: "key".to_owned(),
            source: None,
            partition: None,
            state_context: String::new(),
            point: TemporalPoint::position(1),
            active: true,
            segments: Vec::new(),
            tags: Vec::new(),
        };
        assert!(matches!(
            recorder.observe(invalid),
            Err(WindowRecorderError::EmptyWindowName)
        ));
        assert!(recorder.history().open_windows().is_empty());
        assert!(recorder.history().closed_windows().is_empty());

        let transitions = recorder
            .observe(observation(
                "window",
                "key",
                TemporalPoint::position(1),
                true,
            ))
            .expect("valid observation");
        assert_eq!(transitions[0].record_id.as_str(), "window-0000");
        assert_eq!(recorder.history().open_windows().len(), 1);
    }

    #[test]
    fn valid_history_round_trips_through_serde() {
        let mut recorder = WindowRecorder::new(true);
        let open = observation("window", "key", TemporalPoint::position(1), true)
            .with_scope(Some("source-a".to_owned()), Some("partition-1".to_owned()))
            .expect("scope")
            .with_segments(vec![segment("state", "open")])
            .expect("segments")
            .with_tags(vec![tag("label", "first")])
            .expect("tags");
        recorder.observe(open).expect("open observation");
        let json = serde_json::to_string(recorder.history()).expect("serialize history");
        let restored: WindowHistory = serde_json::from_str(&json).expect("deserialize history");
        assert_eq!(&restored, recorder.history());
    }
}
