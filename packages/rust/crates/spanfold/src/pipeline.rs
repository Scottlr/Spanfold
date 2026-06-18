use std::{
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ClosedWindow, OpenWindow, TemporalPoint, TemporalRange, WindowBoundaryChange,
    WindowBoundaryReason, WindowHistory, WindowRecordId, WindowSegment, WindowTag,
};

type KeySelector<T> = Arc<dyn Fn(&T) -> String + Send + Sync + 'static>;
type ActivePredicate<T> = Arc<dyn Fn(&T) -> bool + Send + Sync + 'static>;
type EventTimeSelector<T> = Arc<dyn Fn(&T) -> i64 + Send + Sync + 'static>;
type SegmentSelector<T> = Arc<dyn Fn(&T) -> Vec<WindowSegment> + Send + Sync + 'static>;
type TagSelector<T> = Arc<dyn Fn(&T) -> Vec<WindowTag> + Send + Sync + 'static>;
type RollupPredicate = Arc<dyn Fn(ChildActivityView) -> bool + Send + Sync + 'static>;
type EmissionCallback = Arc<dyn Fn(&WindowEmission) + Send + Sync + 'static>;
type SegmentTransform =
    Arc<dyn Fn(&crate::PrimitiveValue) -> crate::PrimitiveValue + Send + Sync + 'static>;
type RuntimeStateKey = (String, String, Option<String>, Option<String>, String);

#[derive(Clone, Default)]
struct WindowCallbackSet {
    opened: Vec<EmissionCallback>,
    closed: Vec<EmissionCallback>,
}

struct RollUpDefinition<T> {
    name: String,
    key: KeySelector<T>,
    is_active: RollupPredicate,
    rollups: Vec<RollUpDefinition<T>>,
    callbacks: WindowCallbackSet,
    segment_projection: RollUpSegmentProjection,
}

struct WindowDefinition<T> {
    name: String,
    key: KeySelector<T>,
    is_active: ActivePredicate<T>,
    segments: Option<SegmentSelector<T>>,
    tags: Option<TagSelector<T>>,
    rollups: Vec<RollUpDefinition<T>>,
    callbacks: WindowCallbackSet,
}

impl<T> Clone for RollUpDefinition<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            key: Arc::clone(&self.key),
            is_active: Arc::clone(&self.is_active),
            rollups: self.rollups.clone(),
            callbacks: self.callbacks.clone(),
            segment_projection: self.segment_projection.clone(),
        }
    }
}

impl<T> Clone for WindowDefinition<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            key: Arc::clone(&self.key),
            is_active: Arc::clone(&self.is_active),
            segments: self.segments.as_ref().map(Arc::clone),
            tags: self.tags.as_ref().map(Arc::clone),
            rollups: self.rollups.clone(),
            callbacks: self.callbacks.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct OpenState {
    id: WindowRecordId,
    start: TemporalPoint,
    source: Option<String>,
    partition: Option<String>,
    segments: Vec<WindowSegment>,
    tags: Vec<WindowTag>,
}

#[derive(Clone, Debug, Default)]
struct ParentState {
    children: BTreeMap<String, bool>,
}

#[derive(Clone, Copy)]
struct ChildContext<'a> {
    lineage: &'a str,
    key: &'a str,
    event_point: TemporalPoint,
    is_active: bool,
    segments: &'a [WindowSegment],
    tags: &'a [WindowTag],
}

struct RollupSegmentTransition<'a> {
    previous_child: ChildContext<'a>,
    current_segments: &'a [WindowSegment],
    current_tags: &'a [WindowTag],
}

struct WindowObservation {
    state_key: RuntimeStateKey,
    window_name: String,
    key: String,
    event_point: TemporalPoint,
    source: Option<String>,
    partition: Option<String>,
    is_active: bool,
    segments: Vec<WindowSegment>,
    tags: Vec<WindowTag>,
}

impl ParentState {
    fn view(&self) -> ChildActivityView {
        ChildActivityView {
            active_count: self.children.values().filter(|active| **active).count(),
            total_count: self.children.len(),
        }
    }
}

/// Snapshot of known child activity for a roll-up parent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildActivityView {
    /// Number of active children.
    pub active_count: usize,
    /// Number of known children.
    pub total_count: usize,
}

impl ChildActivityView {
    /// Returns whether every known child is active.
    #[must_use]
    pub fn all_active(self) -> bool {
        self.total_count > 0 && self.active_count == self.total_count
    }

    /// Returns whether at least one known child is active.
    #[must_use]
    pub fn any_active(self) -> bool {
        self.active_count > 0
    }
}

/// Configures which child segment dimensions a roll-up preserves.
#[derive(Clone, Default)]
pub struct RollUpSegmentProjection {
    preserved_names: Option<BTreeSet<String>>,
    dropped_names: BTreeSet<String>,
    renamed_names: BTreeMap<String, String>,
    value_transforms: BTreeMap<String, SegmentTransform>,
}

impl RollUpSegmentProjection {
    /// Creates a projection that preserves every child segment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Preserves only the named segment dimensions.
    #[must_use]
    pub fn preserve(mut self, name: impl Into<String>) -> Self {
        self.preserved_names
            .get_or_insert_with(BTreeSet::new)
            .insert(name.into());
        self
    }

    /// Drops the named segment dimension.
    #[must_use]
    pub fn drop(mut self, name: impl Into<String>) -> Self {
        self.dropped_names.insert(name.into());
        self
    }

    /// Renames a child segment dimension on the roll-up.
    #[must_use]
    pub fn rename(mut self, name: impl Into<String>, projected_name: impl Into<String>) -> Self {
        self.renamed_names
            .insert(name.into(), projected_name.into());
        self
    }

    /// Transforms a child segment value before it is emitted on the roll-up.
    #[must_use]
    pub fn transform<F>(mut self, name: impl Into<String>, transform: F) -> Self
    where
        F: Fn(&crate::PrimitiveValue) -> crate::PrimitiveValue + Send + Sync + 'static,
    {
        self.value_transforms
            .insert(name.into(), Arc::new(transform));
        self
    }
}

/// Callback configuration for one source window or roll-up.
#[derive(Clone, Default)]
pub struct WindowOptions {
    callbacks: WindowCallbackSet,
}

impl WindowOptions {
    /// Creates empty window callback options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a callback invoked when this window opens.
    #[must_use]
    pub fn on_opened<F>(mut self, callback: F) -> Self
    where
        F: Fn(&WindowEmission) + Send + Sync + 'static,
    {
        self.callbacks.opened.push(Arc::new(callback));
        self
    }

    /// Registers a callback invoked when this window closes.
    #[must_use]
    pub fn on_closed<F>(mut self, callback: F) -> Self
    where
        F: Fn(&WindowEmission) + Send + Sync + 'static,
    {
        self.callbacks.closed.push(Arc::new(callback));
        self
    }
}

/// Error returned when a pipeline configuration is invalid.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EventPipelineBuildError {
    /// A window or roll-up name was empty or whitespace-only.
    #[error("window name cannot be empty")]
    EmptyWindowName,
    /// A window or roll-up name was configured more than once.
    #[error("duplicate window name '{0}'")]
    DuplicateWindowName(String),
}

/// Window transition kind emitted during ingestion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WindowTransitionKind {
    /// A window opened.
    Opened,
    /// A window closed.
    Closed,
}

/// One window transition emitted during ingestion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowEmission {
    /// Transition kind.
    pub kind: WindowTransitionKind,
    /// Window family name.
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Window record ID.
    pub record_id: WindowRecordId,
    /// Processing position for the transition.
    pub position: i64,
    /// Optional source/lane.
    pub source: Option<String>,
    /// Optional partition.
    pub partition: Option<String>,
    /// Analytical segment values attached to the emitted window.
    pub segments: Vec<WindowSegment>,
    /// Descriptive non-boundary metadata attached to the emitted window.
    pub tags: Vec<WindowTag>,
    /// Reason a closed boundary was emitted, when known.
    pub boundary_reason: Option<WindowBoundaryReason>,
    /// Segment changes that caused this boundary.
    pub boundary_changes: Vec<WindowBoundaryChange>,
}

/// Result of ingesting one event.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IngestionResult {
    /// Processing position after ingestion.
    pub processing_position: i64,
    /// Window transitions emitted by the event.
    pub emissions: Vec<WindowEmission>,
}

impl IngestionResult {
    /// Returns whether the event emitted any window transitions.
    #[must_use]
    pub fn has_emissions(&self) -> bool {
        !self.emissions.is_empty()
    }
}

/// Metadata for one configured window or roll-up.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WindowMetadata {
    /// Window or roll-up name.
    pub name: String,
    /// Nested roll-up metadata.
    pub rollups: Vec<WindowMetadata>,
}

/// Metadata for a configured event pipeline.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventPipelineMetadata {
    /// Event type name, when known.
    pub event_type: Option<String>,
    /// Configured top-level windows.
    pub windows: Vec<WindowMetadata>,
}

/// Builder for an event ingestion pipeline.
#[derive(Clone)]
pub struct EventPipelineBuilder<T> {
    windows: Vec<WindowDefinition<T>>,
    event_time: Option<EventTimeSelector<T>>,
    emission_callbacks: Vec<EmissionCallback>,
    record_windows: bool,
    marker: PhantomData<T>,
}

impl<T> Default for EventPipelineBuilder<T> {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            event_time: None,
            emission_callbacks: Vec::new(),
            record_windows: false,
            marker: PhantomData,
        }
    }
}

/// Builder returned while configuring one source window and its roll-ups.
pub struct WindowPipelineBuilder<T> {
    builder: EventPipelineBuilder<T>,
    path: Vec<usize>,
}

/// Event ingestion pipeline that records source windows and roll-ups.
pub struct EventPipeline<T> {
    windows: Vec<WindowDefinition<T>>,
    event_time: Option<EventTimeSelector<T>>,
    emission_callbacks: Vec<EmissionCallback>,
    window_callbacks: BTreeMap<String, WindowCallbackSet>,
    record_windows: bool,
    history: WindowHistory,
    active: BTreeMap<RuntimeStateKey, OpenState>,
    parents: BTreeMap<RuntimeStateKey, ParentState>,
    position: i64,
    next_record_id: u64,
    marker: PhantomData<T>,
}

/// Creates a new event pipeline builder for one event type.
#[must_use]
pub fn for_events<T>() -> EventPipelineBuilder<T> {
    EventPipelineBuilder::default()
}

impl<T> EventPipelineBuilder<T> {
    /// Starts recording windows for the configured event type.
    #[must_use]
    pub fn record_windows(mut self) -> Self {
        self.record_windows = true;
        self
    }

    /// Records windows on a timestamp axis selected from each event.
    #[must_use]
    pub fn with_event_time<F>(mut self, selector: F) -> Self
    where
        F: Fn(&T) -> i64 + Send + Sync + 'static,
    {
        self.event_time = Some(Arc::new(selector));
        self
    }

    /// Registers a callback invoked for every emitted transition.
    #[must_use]
    pub fn on_emission<F>(mut self, callback: F) -> Self
    where
        F: Fn(&WindowEmission) + Send + Sync + 'static,
    {
        self.emission_callbacks.push(Arc::new(callback));
        self
    }

    /// Adds a tracked window and returns the builder.
    #[must_use]
    pub fn track_window<K, F, P>(mut self, name: impl Into<String>, key: F, is_active: P) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
    {
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: None,
            tags: None,
            rollups: Vec::new(),
            callbacks: WindowCallbackSet::default(),
        });
        self
    }

    /// Adds a tracked window with lifecycle callbacks and returns the builder.
    #[must_use]
    pub fn track_window_with_options<K, F, P, C>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        configure: C,
    ) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
        C: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure(WindowOptions::new());
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: None,
            tags: None,
            rollups: Vec::new(),
            callbacks: options.callbacks,
        });
        self
    }

    /// Adds a tracked window with segment and tag selectors.
    #[must_use]
    pub fn track_window_with_metadata<K, F, P, S, G>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        segments: S,
        tags: G,
    ) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
        S: Fn(&T) -> Vec<WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<WindowTag> + Send + Sync + 'static,
    {
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: Some(Arc::new(segments)),
            tags: Some(Arc::new(tags)),
            rollups: Vec::new(),
            callbacks: WindowCallbackSet::default(),
        });
        self
    }

    /// Adds a tracked window with metadata selectors and lifecycle callbacks.
    #[must_use]
    pub fn track_window_with_metadata_and_options<K, F, P, S, G, C>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        segments: S,
        tags: G,
        configure: C,
    ) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
        S: Fn(&T) -> Vec<WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<WindowTag> + Send + Sync + 'static,
        C: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure(WindowOptions::new());
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: Some(Arc::new(segments)),
            tags: Some(Arc::new(tags)),
            rollups: Vec::new(),
            callbacks: options.callbacks,
        });
        self
    }

    /// Adds a tracked window and returns a nested roll-up builder.
    #[must_use]
    pub fn window<K, F, P>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
    ) -> WindowPipelineBuilder<T>
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
    {
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: None,
            tags: None,
            rollups: Vec::new(),
            callbacks: WindowCallbackSet::default(),
        });
        WindowPipelineBuilder {
            path: vec![self.windows.len() - 1],
            builder: self,
        }
    }

    /// Adds a tracked window with lifecycle callbacks and returns a nested roll-up builder.
    #[must_use]
    pub fn window_with_options<K, F, P, C>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        configure: C,
    ) -> WindowPipelineBuilder<T>
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
        C: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure(WindowOptions::new());
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: None,
            tags: None,
            rollups: Vec::new(),
            callbacks: options.callbacks,
        });
        WindowPipelineBuilder {
            path: vec![self.windows.len() - 1],
            builder: self,
        }
    }

    /// Adds a tracked window with metadata selectors and returns a nested roll-up builder.
    #[must_use]
    pub fn window_with_metadata<K, F, P, S, G>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        segments: S,
        tags: G,
    ) -> WindowPipelineBuilder<T>
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
        S: Fn(&T) -> Vec<WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<WindowTag> + Send + Sync + 'static,
    {
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: Some(Arc::new(segments)),
            tags: Some(Arc::new(tags)),
            rollups: Vec::new(),
            callbacks: WindowCallbackSet::default(),
        });
        WindowPipelineBuilder {
            path: vec![self.windows.len() - 1],
            builder: self,
        }
    }

    /// Adds a tracked window with metadata selectors and lifecycle callbacks.
    #[must_use]
    pub fn window_with_metadata_and_options<K, F, P, S, G, C>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        segments: S,
        tags: G,
        configure: C,
    ) -> WindowPipelineBuilder<T>
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
        S: Fn(&T) -> Vec<WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<WindowTag> + Send + Sync + 'static,
        C: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure(WindowOptions::new());
        self.windows.push(WindowDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            segments: Some(Arc::new(segments)),
            tags: Some(Arc::new(tags)),
            rollups: Vec::new(),
            callbacks: options.callbacks,
        });
        WindowPipelineBuilder {
            path: vec![self.windows.len() - 1],
            builder: self,
        }
    }

    /// Builds the pipeline.
    #[must_use]
    pub fn build(self) -> EventPipeline<T> {
        self.try_build().expect("valid pipeline configuration")
    }

    /// Builds the pipeline or returns a configuration error.
    pub fn try_build(self) -> Result<EventPipeline<T>, EventPipelineBuildError> {
        validate_window_names(&self.windows)?;
        let window_callbacks = collect_window_callbacks(&self.windows);
        Ok(EventPipeline {
            windows: self.windows,
            event_time: self.event_time,
            emission_callbacks: self.emission_callbacks,
            window_callbacks,
            record_windows: self.record_windows,
            history: WindowHistory::new(),
            active: BTreeMap::new(),
            parents: BTreeMap::new(),
            position: 0,
            next_record_id: 0,
            marker: PhantomData,
        })
    }
}

impl<T> WindowPipelineBuilder<T> {
    /// Registers a callback invoked for every emitted transition.
    #[must_use]
    pub fn on_emission<F>(mut self, callback: F) -> Self
    where
        F: Fn(&WindowEmission) + Send + Sync + 'static,
    {
        self.builder.emission_callbacks.push(Arc::new(callback));
        self
    }

    /// Adds a nested roll-up to the current window or roll-up node.
    #[must_use]
    pub fn roll_up<K, F, P>(mut self, name: impl Into<String>, key: F, is_active: P) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(ChildActivityView) -> bool + Send + Sync + 'static,
    {
        let definition = RollUpDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            rollups: Vec::new(),
            callbacks: WindowCallbackSet::default(),
            segment_projection: RollUpSegmentProjection::default(),
        };
        let next_index = add_rollup(&mut self.builder.windows, &self.path, definition);
        self.path.push(next_index);
        self
    }

    /// Adds a nested roll-up with lifecycle callbacks.
    #[must_use]
    pub fn roll_up_with_options<K, F, P, C>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        configure: C,
    ) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(ChildActivityView) -> bool + Send + Sync + 'static,
        C: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure(WindowOptions::new());
        let definition = RollUpDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            rollups: Vec::new(),
            callbacks: options.callbacks,
            segment_projection: RollUpSegmentProjection::default(),
        };
        let next_index = add_rollup(&mut self.builder.windows, &self.path, definition);
        self.path.push(next_index);
        self
    }

    /// Adds a nested roll-up with segment projection rules.
    #[must_use]
    pub fn roll_up_with_segment_projection<K, F, P, C>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        configure_projection: C,
    ) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(ChildActivityView) -> bool + Send + Sync + 'static,
        C: FnOnce(RollUpSegmentProjection) -> RollUpSegmentProjection,
    {
        let definition = RollUpDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            rollups: Vec::new(),
            callbacks: WindowCallbackSet::default(),
            segment_projection: configure_projection(RollUpSegmentProjection::new()),
        };
        let next_index = add_rollup(&mut self.builder.windows, &self.path, definition);
        self.path.push(next_index);
        self
    }

    /// Adds a nested roll-up with segment projection rules and lifecycle callbacks.
    #[must_use]
    pub fn roll_up_with_segment_projection_and_options<K, F, P, C, O>(
        mut self,
        name: impl Into<String>,
        key: F,
        is_active: P,
        configure_projection: C,
        configure_options: O,
    ) -> Self
    where
        K: Into<String> + 'static,
        F: Fn(&T) -> K + Send + Sync + 'static,
        P: Fn(ChildActivityView) -> bool + Send + Sync + 'static,
        C: FnOnce(RollUpSegmentProjection) -> RollUpSegmentProjection,
        O: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure_options(WindowOptions::new());
        let definition = RollUpDefinition {
            name: name.into(),
            key: Arc::new(move |event| key(event).into()),
            is_active: Arc::new(is_active),
            rollups: Vec::new(),
            callbacks: options.callbacks,
            segment_projection: configure_projection(RollUpSegmentProjection::new()),
        };
        let next_index = add_rollup(&mut self.builder.windows, &self.path, definition);
        self.path.push(next_index);
        self
    }

    /// Builds the pipeline.
    #[must_use]
    pub fn build(self) -> EventPipeline<T> {
        self.builder.build()
    }

    /// Builds the pipeline or returns a configuration error.
    pub fn try_build(self) -> Result<EventPipeline<T>, EventPipelineBuildError> {
        self.builder.try_build()
    }
}

impl<T> EventPipeline<T> {
    /// Returns the latest processing position.
    #[must_use]
    pub fn processing_position(&self) -> i64 {
        self.position
    }

    /// Returns the recorded window history.
    #[must_use]
    pub fn history(&self) -> &WindowHistory {
        &self.history
    }

    /// Returns configured pipeline metadata.
    #[must_use]
    pub fn metadata(&self) -> EventPipelineMetadata {
        EventPipelineMetadata {
            event_type: None,
            windows: self.windows.iter().map(window_metadata).collect(),
        }
    }

    /// Ingests one event with optional source and partition context.
    pub fn ingest(
        &mut self,
        event: T,
        source: Option<&str>,
        partition: Option<&str>,
    ) -> IngestionResult {
        self.position += 1;
        let event_point = self.event_time.as_ref().map_or_else(
            || TemporalPoint::position(self.position),
            |selector| TemporalPoint::timestamp_ticks(selector(&event)),
        );
        let mut emissions = Vec::new();
        for definition in self.windows.clone() {
            self.ingest_definition(
                &definition,
                &event,
                event_point,
                source,
                partition,
                &mut emissions,
            );
        }
        for emission in &emissions {
            self.invoke_callbacks(emission);
        }
        IngestionResult {
            processing_position: self.position,
            emissions,
        }
    }

    /// Ingests multiple events sequentially and returns all emitted transitions.
    pub fn ingest_many<I>(
        &mut self,
        events: I,
        source: Option<&str>,
        partition: Option<&str>,
    ) -> IngestionResult
    where
        I: IntoIterator<Item = T>,
    {
        let mut emissions = Vec::new();
        for event in events {
            emissions.extend(self.ingest(event, source, partition).emissions);
        }
        IngestionResult {
            processing_position: self.position,
            emissions,
        }
    }

    fn ingest_definition(
        &mut self,
        definition: &WindowDefinition<T>,
        event: &T,
        event_point: TemporalPoint,
        source: Option<&str>,
        partition: Option<&str>,
        emissions: &mut Vec<WindowEmission>,
    ) {
        let key = (definition.key)(event);
        let is_active = (definition.is_active)(event);
        let segments = if is_active {
            definition
                .segments
                .as_ref()
                .map_or_else(Vec::new, |selector| selector(event))
        } else {
            Vec::new()
        };
        let tags = if is_active {
            definition
                .tags
                .as_ref()
                .map_or_else(Vec::new, |selector| selector(event))
        } else {
            Vec::new()
        };
        let state_key = (
            definition.name.clone(),
            key.clone(),
            source.map(str::to_owned),
            partition.map(str::to_owned),
            String::new(),
        );
        let previous = self.active.get(&state_key).cloned();
        emissions.extend(self.sync_window_state(WindowObservation {
            state_key,
            window_name: definition.name.clone(),
            key: key.clone(),
            event_point,
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            is_active,
            segments: segments.clone(),
            tags: tags.clone(),
        }));

        if is_active {
            if let Some(previous) = previous
                && previous.segments != segments
            {
                self.sync_rollup_segment_transition(
                    &definition.rollups,
                    event,
                    source,
                    partition,
                    RollupSegmentTransition {
                        previous_child: ChildContext {
                            lineage: &definition.name,
                            key: &key,
                            event_point,
                            is_active: true,
                            segments: &previous.segments,
                            tags: &previous.tags,
                        },
                        current_segments: &segments,
                        current_tags: &tags,
                    },
                    emissions,
                );
                return;
            }
            self.sync_rollups(
                &definition.rollups,
                event,
                source,
                partition,
                ChildContext {
                    lineage: &definition.name,
                    key: &key,
                    event_point,
                    is_active: true,
                    segments: &segments,
                    tags: &tags,
                },
                emissions,
            );
        } else if let Some(previous) = previous {
            self.sync_rollups(
                &definition.rollups,
                event,
                source,
                partition,
                ChildContext {
                    lineage: &definition.name,
                    key: &key,
                    event_point,
                    is_active: false,
                    segments: &previous.segments,
                    tags: &previous.tags,
                },
                emissions,
            );
        } else {
            self.sync_rollups(
                &definition.rollups,
                event,
                source,
                partition,
                ChildContext {
                    lineage: &definition.name,
                    key: &key,
                    event_point,
                    is_active: false,
                    segments: &[],
                    tags: &[],
                },
                emissions,
            );
        }
    }

    fn sync_rollups(
        &mut self,
        definitions: &[RollUpDefinition<T>],
        event: &T,
        source: Option<&str>,
        partition: Option<&str>,
        child: ChildContext<'_>,
        emissions: &mut Vec<WindowEmission>,
    ) {
        for definition in definitions {
            self.sync_rollup(definition, event, source, partition, child, emissions);
        }
    }

    fn sync_rollup(
        &mut self,
        definition: &RollUpDefinition<T>,
        event: &T,
        source: Option<&str>,
        partition: Option<&str>,
        child: ChildContext<'_>,
        emissions: &mut Vec<WindowEmission>,
    ) {
        let projected_segments = project_segments(&definition.segment_projection, child.segments);
        let segment_context = stable_segments(&projected_segments);
        let key = (definition.key)(event);
        let state_key = (
            format!("{}>{}", child.lineage, definition.name),
            key.clone(),
            source.map(str::to_owned),
            partition.map(str::to_owned),
            segment_context.clone(),
        );
        let parent_state = self.parents.entry(state_key).or_default();
        parent_state
            .children
            .insert(child.key.to_owned(), child.is_active);
        let is_active = (definition.is_active)(parent_state.view());
        emissions.extend(self.sync_window_state(WindowObservation {
            state_key: (
                definition.name.clone(),
                key.clone(),
                source.map(str::to_owned),
                partition.map(str::to_owned),
                segment_context,
            ),
            window_name: definition.name.clone(),
            key: key.clone(),
            event_point: child.event_point,
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            is_active,
            segments: projected_segments.clone(),
            tags: child.tags.to_vec(),
        }));

        self.sync_rollups(
            &definition.rollups,
            event,
            source,
            partition,
            ChildContext {
                lineage: &definition.name,
                key: &key,
                event_point: child.event_point,
                is_active,
                segments: &projected_segments,
                tags: child.tags,
            },
            emissions,
        );
    }

    fn sync_rollup_segment_transition(
        &mut self,
        definitions: &[RollUpDefinition<T>],
        event: &T,
        source: Option<&str>,
        partition: Option<&str>,
        transition: RollupSegmentTransition<'_>,
        emissions: &mut Vec<WindowEmission>,
    ) {
        for definition in definitions {
            let previous_projected = project_segments(
                &definition.segment_projection,
                transition.previous_child.segments,
            );
            let current_projected =
                project_segments(&definition.segment_projection, transition.current_segments);
            if previous_projected != current_projected {
                self.sync_rollup(
                    definition,
                    event,
                    source,
                    partition,
                    ChildContext {
                        is_active: false,
                        ..transition.previous_child
                    },
                    emissions,
                );
                self.sync_rollup(
                    definition,
                    event,
                    source,
                    partition,
                    ChildContext {
                        lineage: transition.previous_child.lineage,
                        key: transition.previous_child.key,
                        event_point: transition.previous_child.event_point,
                        is_active: true,
                        segments: transition.current_segments,
                        tags: transition.current_tags,
                    },
                    emissions,
                );
            } else {
                self.sync_rollup(
                    definition,
                    event,
                    source,
                    partition,
                    ChildContext {
                        lineage: transition.previous_child.lineage,
                        key: transition.previous_child.key,
                        event_point: transition.previous_child.event_point,
                        is_active: true,
                        segments: transition.current_segments,
                        tags: transition.current_tags,
                    },
                    emissions,
                );
            }
        }
    }

    fn sync_window_state(&mut self, observation: WindowObservation) -> Vec<WindowEmission> {
        if observation.is_active {
            if let Some(previous) = self.active.get(&observation.state_key) {
                if previous.segments == observation.segments {
                    return Vec::new();
                }
                let mut emissions = Vec::new();
                let changes = segment_changes(&previous.segments, &observation.segments);
                if let Some(emission) = self.close_window_state(
                    &observation.state_key,
                    observation.event_point,
                    Some(WindowBoundaryReason::SegmentChanged),
                    changes,
                ) {
                    emissions.push(emission);
                }
                emissions.push(self.open_window_state(observation));
                return emissions;
            }
            return vec![self.open_window_state(observation)];
        }

        self.close_window_state(
            &observation.state_key,
            observation.event_point,
            Some(WindowBoundaryReason::ActivePredicateEnded),
            Vec::new(),
        )
        .into_iter()
        .collect()
    }

    fn open_window_state(&mut self, observation: WindowObservation) -> WindowEmission {
        let id = self.next_id();
        let open = OpenWindow {
            id: id.clone(),
            window_name: observation.window_name.clone(),
            key: observation.key.clone(),
            start: observation.event_point,
            known_at: None,
            source: observation.source.clone(),
            partition: observation.partition.clone(),
            segments: observation.segments.clone(),
            tags: observation.tags.clone(),
        };
        if self.record_windows {
            self.history.push_open(open);
        }
        self.active.insert(
            observation.state_key,
            OpenState {
                id: id.clone(),
                start: observation.event_point,
                source: observation.source.clone(),
                partition: observation.partition.clone(),
                segments: observation.segments.clone(),
                tags: observation.tags.clone(),
            },
        );
        WindowEmission {
            kind: WindowTransitionKind::Opened,
            window_name: observation.window_name,
            key: observation.key,
            record_id: id,
            position: self.position,
            source: observation.source,
            partition: observation.partition,
            segments: observation.segments,
            tags: observation.tags,
            boundary_reason: None,
            boundary_changes: Vec::new(),
        }
    }

    fn close_window_state(
        &mut self,
        state_key: &RuntimeStateKey,
        event_point: TemporalPoint,
        boundary_reason: Option<WindowBoundaryReason>,
        boundary_changes: Vec<WindowBoundaryChange>,
    ) -> Option<WindowEmission> {
        let open_state = self.active.remove(state_key)?;
        let emission = WindowEmission {
            kind: WindowTransitionKind::Closed,
            window_name: state_key.0.clone(),
            key: state_key.1.clone(),
            record_id: open_state.id.clone(),
            position: self.position,
            source: open_state.source.clone(),
            partition: open_state.partition.clone(),
            segments: open_state.segments.clone(),
            tags: open_state.tags.clone(),
            boundary_reason,
            boundary_changes: boundary_changes.clone(),
        };
        if self.record_windows
            && let Ok(range) = TemporalRange::new(open_state.start, event_point)
        {
            self.history.remove_open(&open_state.id);
            self.history.push_closed(ClosedWindow {
                id: open_state.id,
                window_name: state_key.0.clone(),
                key: state_key.1.clone(),
                range,
                known_at: None,
                source: open_state.source,
                partition: open_state.partition,
                segments: open_state.segments,
                tags: open_state.tags,
                boundary_reason,
                boundary_changes,
            });
        }
        Some(emission)
    }

    fn invoke_callbacks(&self, emission: &WindowEmission) {
        if let Some(callbacks) = self.window_callbacks.get(&emission.window_name) {
            let selected = if emission.kind == WindowTransitionKind::Opened {
                &callbacks.opened
            } else {
                &callbacks.closed
            };
            for callback in selected {
                callback(emission);
            }
        }
        for callback in &self.emission_callbacks {
            callback(emission);
        }
    }

    fn next_id(&mut self) -> WindowRecordId {
        let id = WindowRecordId::new(format!("pipeline-{:04}", self.next_record_id));
        self.next_record_id += 1;
        id
    }
}

fn window_metadata<T>(definition: &WindowDefinition<T>) -> WindowMetadata {
    WindowMetadata {
        name: definition.name.clone(),
        rollups: definition.rollups.iter().map(rollup_metadata).collect(),
    }
}

fn rollup_metadata<T>(definition: &RollUpDefinition<T>) -> WindowMetadata {
    WindowMetadata {
        name: definition.name.clone(),
        rollups: definition.rollups.iter().map(rollup_metadata).collect(),
    }
}

fn validate_window_names<T>(
    windows: &[WindowDefinition<T>],
) -> Result<(), EventPipelineBuildError> {
    let mut names = BTreeSet::new();
    for window in windows {
        validate_window_name(&window.name, &mut names)?;
        for rollup in &window.rollups {
            validate_rollup_name(rollup, &mut names)?;
        }
    }
    Ok(())
}

fn validate_window_name(
    name: &str,
    names: &mut BTreeSet<String>,
) -> Result<(), EventPipelineBuildError> {
    if name.trim().is_empty() {
        return Err(EventPipelineBuildError::EmptyWindowName);
    }
    if !names.insert(name.to_owned()) {
        return Err(EventPipelineBuildError::DuplicateWindowName(
            name.to_owned(),
        ));
    }
    Ok(())
}

fn validate_rollup_name<T>(
    rollup: &RollUpDefinition<T>,
    names: &mut BTreeSet<String>,
) -> Result<(), EventPipelineBuildError> {
    validate_window_name(&rollup.name, names)?;
    for child in &rollup.rollups {
        validate_rollup_name(child, names)?;
    }
    Ok(())
}

fn collect_window_callbacks<T>(
    windows: &[WindowDefinition<T>],
) -> BTreeMap<String, WindowCallbackSet> {
    let mut callbacks = BTreeMap::new();
    for window in windows {
        collect_window_callbacks_for_window(window, &mut callbacks);
    }
    callbacks
}

fn collect_window_callbacks_for_window<T>(
    window: &WindowDefinition<T>,
    callbacks: &mut BTreeMap<String, WindowCallbackSet>,
) {
    callbacks.insert(window.name.clone(), window.callbacks.clone());
    for rollup in &window.rollups {
        collect_window_callbacks_for_rollup(rollup, callbacks);
    }
}

fn collect_window_callbacks_for_rollup<T>(
    rollup: &RollUpDefinition<T>,
    callbacks: &mut BTreeMap<String, WindowCallbackSet>,
) {
    callbacks.insert(rollup.name.clone(), rollup.callbacks.clone());
    for child in &rollup.rollups {
        collect_window_callbacks_for_rollup(child, callbacks);
    }
}

fn project_segments(
    projection: &RollUpSegmentProjection,
    segments: &[WindowSegment],
) -> Vec<WindowSegment> {
    if segments.is_empty()
        || (projection.preserved_names.is_none()
            && projection.dropped_names.is_empty()
            && projection.renamed_names.is_empty()
            && projection.value_transforms.is_empty())
    {
        return segments.to_vec();
    }

    let mut projected = Vec::new();
    let mut selected_original_names = BTreeSet::new();
    let mut selected_projected_names = BTreeSet::new();

    for segment in segments {
        if !should_keep_segment(projection, &segment.name) {
            continue;
        }
        let projected_name = projection
            .renamed_names
            .get(&segment.name)
            .cloned()
            .unwrap_or_else(|| segment.name.clone());
        assert!(
            selected_projected_names.insert(projected_name.clone()),
            "Roll-up segment projection produced duplicate segment '{}'.",
            projected_name
        );
        let value = projection.value_transforms.get(&segment.name).map_or_else(
            || segment.value.clone(),
            |transform| transform(&segment.value),
        );
        projected.push(WindowSegment {
            name: projected_name,
            value,
            parent_name: segment.parent_name.clone(),
        });
        selected_original_names.insert(segment.name.clone());
    }

    for segment in &mut projected {
        if let Some(parent_name) = &segment.parent_name {
            if selected_original_names.contains(parent_name) {
                segment.parent_name = projection
                    .renamed_names
                    .get(parent_name)
                    .cloned()
                    .or_else(|| Some(parent_name.clone()));
            } else {
                segment.parent_name = None;
            }
        }
    }

    projected
}

fn should_keep_segment(projection: &RollUpSegmentProjection, name: &str) -> bool {
    if projection.dropped_names.contains(name) {
        return false;
    }
    projection
        .preserved_names
        .as_ref()
        .is_none_or(|names| names.contains(name))
}

fn stable_segments(segments: &[WindowSegment]) -> String {
    if segments.is_empty() {
        return String::new();
    }
    let mut stable = String::new();
    for segment in segments {
        stable.push_str(segment.parent_name.as_deref().unwrap_or_default());
        stable.push('/');
        stable.push_str(&segment.name);
        stable.push('=');
        stable.push_str(&format!("{:?}", segment.value));
        stable.push(';');
    }
    stable
}

fn segment_changes(
    previous: &[WindowSegment],
    current: &[WindowSegment],
) -> Vec<WindowBoundaryChange> {
    let mut changes = Vec::new();
    for index in 0..previous.len().max(current.len()) {
        let before = previous.get(index);
        let after = current.get(index);
        if before == after {
            continue;
        }
        let segment_name = before.map_or_else(
            || after.expect("current segment exists").name.clone(),
            |segment| segment.name.clone(),
        );
        changes.push(WindowBoundaryChange {
            segment_name,
            previous_value: before.map(|segment| segment.value.clone()),
            current_value: after.map(|segment| segment.value.clone()),
        });
    }
    changes
}

fn add_rollup<T>(
    windows: &mut [WindowDefinition<T>],
    path: &[usize],
    definition: RollUpDefinition<T>,
) -> usize {
    let mut rollups = &mut windows[path[0]].rollups;
    for index in &path[1..] {
        rollups = &mut rollups[*index].rollups;
    }
    rollups.push(definition);
    rollups.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct PriceTick {
        selection_id: &'static str,
        market_id: &'static str,
        fixture_id: &'static str,
        price: f64,
        observed_at: i64,
    }

    #[test]
    fn track_window_records_closed_history() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .track_window(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
            )
            .build();

        let first = pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            Some("provider-a"),
            None,
        );
        let second = pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.2,
                observed_at: 130,
            },
            Some("provider-a"),
            None,
        );

        assert_eq!(first.emissions[0].kind, WindowTransitionKind::Opened);
        assert_eq!(second.emissions[0].kind, WindowTransitionKind::Closed);
        assert!(first.has_emissions());
        assert_eq!(pipeline.history().closed_windows().len(), 1);
        assert_eq!(
            pipeline.history().closed_windows()[0].window_name,
            "SelectionSuspension"
        );
        assert_eq!(pipeline.metadata().windows[0].name, "SelectionSuspension");
    }

    #[test]
    fn nested_rollups_record_parent_windows() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .window(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
            )
            .roll_up(
                "MarketSuspension",
                |tick| tick.market_id,
                |children| children.any_active(),
            )
            .roll_up(
                "FixtureSuspension",
                |tick| tick.fixture_id,
                |children| children.any_active(),
            )
            .build();

        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            None,
            None,
        );
        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 130,
            },
            None,
            None,
        );

        let history = pipeline.history();
        assert_eq!(history.closed_windows().len(), 3);
        let hierarchy = history.compare_hierarchy(
            "Market explanation",
            "MarketSuspension",
            "SelectionSuspension",
        );
        assert_eq!(hierarchy.rows.len(), 1);
        assert_eq!(
            hierarchy.rows[0].kind,
            crate::HierarchyComparisonRowKind::ParentExplained
        );
        let metadata = pipeline.metadata();
        assert_eq!(metadata.windows[0].rollups[0].name, "MarketSuspension");
        assert_eq!(
            metadata.windows[0].rollups[0].rollups[0].name,
            "FixtureSuspension"
        );
    }

    #[test]
    fn event_time_selector_records_timestamp_axis_windows() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .with_event_time(|tick| tick.observed_at)
            .track_window(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
            )
            .build();

        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 1_000,
            },
            None,
            None,
        );
        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 1_250,
            },
            None,
            None,
        );

        let window = &pipeline.history().closed_windows()[0];
        assert_eq!(window.range.start(), TemporalPoint::timestamp_ticks(1_000));
        assert_eq!(window.range.end(), TemporalPoint::timestamp_ticks(1_250));
    }

    #[test]
    fn ingest_many_aggregates_emissions() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .track_window(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
            )
            .build();

        let result = pipeline.ingest_many(
            [
                PriceTick {
                    selection_id: "selection-1",
                    market_id: "market-1",
                    fixture_id: "fixture-1",
                    price: 0.0,
                    observed_at: 100,
                },
                PriceTick {
                    selection_id: "selection-1",
                    market_id: "market-1",
                    fixture_id: "fixture-1",
                    price: 1.1,
                    observed_at: 101,
                },
            ],
            Some("provider-a"),
            None,
        );

        assert_eq!(result.processing_position, 2);
        assert_eq!(
            result
                .emissions
                .iter()
                .map(|emission| emission.kind)
                .collect::<Vec<_>>(),
            vec![WindowTransitionKind::Opened, WindowTransitionKind::Closed]
        );
    }

    #[test]
    fn try_build_rejects_empty_and_duplicate_window_names() {
        let empty = for_events::<PriceTick>()
            .record_windows()
            .track_window("", |tick| tick.selection_id, |tick| tick.price == 0.0)
            .try_build();

        assert!(matches!(
            empty,
            Err(EventPipelineBuildError::EmptyWindowName)
        ));

        let duplicate = for_events::<PriceTick>()
            .record_windows()
            .window(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
            )
            .roll_up(
                "SelectionSuspension",
                |tick| tick.market_id,
                |children| children.any_active(),
            )
            .try_build();

        assert!(matches!(
            duplicate,
            Err(EventPipelineBuildError::DuplicateWindowName(name)) if name == "SelectionSuspension"
        ));
    }

    #[test]
    fn segment_change_closes_and_reopens_active_window() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .track_window_with_metadata(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
                |tick| vec![WindowSegment::new("market", tick.market_id)],
                |tick| vec![WindowTag::new("fixture", tick.fixture_id)],
            )
            .build();

        let first = pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            Some("provider-a"),
            None,
        );
        let second = pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-2",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 101,
            },
            Some("provider-a"),
            None,
        );

        assert_eq!(first.emissions.len(), 1);
        assert_eq!(
            second
                .emissions
                .iter()
                .map(|emission| emission.kind)
                .collect::<Vec<_>>(),
            vec![WindowTransitionKind::Closed, WindowTransitionKind::Opened]
        );
        assert_eq!(pipeline.history().closed_windows().len(), 1);
        assert_eq!(pipeline.history().open_windows().len(), 1);
        assert_eq!(
            pipeline.history().closed_windows()[0].segments[0].value,
            crate::PrimitiveValue::from("market-1")
        );
        assert_eq!(
            pipeline.history().open_windows()[0].segments[0].value,
            crate::PrimitiveValue::from("market-2")
        );
        assert_eq!(pipeline.history().open_windows()[0].tags.len(), 1);
        assert_eq!(
            pipeline.history().closed_windows()[0].boundary_reason,
            Some(WindowBoundaryReason::SegmentChanged)
        );
        assert_eq!(
            pipeline.history().closed_windows()[0].boundary_changes[0].segment_name,
            "market"
        );
        assert_eq!(
            pipeline.history().closed_windows()[0].boundary_changes[0].previous_value,
            Some(crate::PrimitiveValue::from("market-1"))
        );
        assert_eq!(
            pipeline.history().closed_windows()[0].boundary_changes[0].current_value,
            Some(crate::PrimitiveValue::from("market-2"))
        );
        assert_eq!(
            second.emissions[0].boundary_reason,
            Some(WindowBoundaryReason::SegmentChanged)
        );
        assert_eq!(second.emissions[0].segments.len(), 1);
    }

    #[test]
    fn rollups_preserve_child_segment_context_and_reopen_on_segment_change() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .window_with_metadata(
                "SelectionPriced",
                |tick| tick.selection_id,
                |tick| tick.price > 0.0,
                |tick| vec![WindowSegment::new("phase", tick.market_id)],
                |_| Vec::new(),
            )
            .roll_up(
                "FixturePriced",
                |tick| tick.fixture_id,
                |children| children.any_active(),
            )
            .build();

        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "Pregame",
                fixture_id: "fixture-1",
                price: 1.01,
                observed_at: 100,
            },
            None,
            None,
        );
        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "InPlay",
                fixture_id: "fixture-1",
                price: 1.01,
                observed_at: 101,
            },
            None,
            None,
        );

        let closed_rollup = pipeline
            .history()
            .closed_windows()
            .iter()
            .find(|window| window.window_name == "FixturePriced")
            .expect("closed roll-up");
        let open_rollup = pipeline
            .history()
            .open_windows()
            .iter()
            .find(|window| window.window_name == "FixturePriced")
            .expect("open roll-up");

        assert_eq!(
            closed_rollup.segments[0].value,
            crate::PrimitiveValue::from("Pregame")
        );
        assert_eq!(
            open_rollup.segments[0].value,
            crate::PrimitiveValue::from("InPlay")
        );
    }

    #[test]
    fn rollup_segment_projection_can_drop_rename_and_transform() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .window_with_metadata(
                "SelectionPriced",
                |tick| tick.selection_id,
                |tick| tick.price > 0.0,
                |tick| {
                    vec![
                        WindowSegment::new("phase", tick.market_id),
                        WindowSegment::new("state", tick.fixture_id).with_parent("phase"),
                    ]
                },
                |_| Vec::new(),
            )
            .roll_up_with_segment_projection(
                "MarketPriced",
                |tick| tick.fixture_id,
                |children| children.any_active(),
                |projection| {
                    projection
                        .preserve("phase")
                        .rename("phase", "lifecycle")
                        .transform("phase", |value| match value {
                            crate::PrimitiveValue::String(value) => {
                                crate::PrimitiveValue::from(value.to_uppercase())
                            }
                            other => other.clone(),
                        })
                },
            )
            .build();

        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "in-play",
                fixture_id: "fixture-1",
                price: 1.01,
                observed_at: 100,
            },
            None,
            None,
        );

        let open_rollup = pipeline
            .history()
            .open_windows()
            .iter()
            .find(|window| window.window_name == "MarketPriced")
            .expect("open roll-up");

        assert_eq!(open_rollup.segments.len(), 1);
        assert_eq!(open_rollup.segments[0].name, "lifecycle");
        assert_eq!(
            open_rollup.segments[0].value,
            crate::PrimitiveValue::from("IN-PLAY")
        );
        assert_eq!(open_rollup.segments[0].parent_name, None);
    }

    #[test]
    #[should_panic(expected = "duplicate segment 'phase'")]
    fn rollup_rejects_duplicate_projected_segment_names() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .window_with_metadata(
                "SelectionPriced",
                |tick| tick.selection_id,
                |tick| tick.price > 0.0,
                |tick| {
                    vec![
                        WindowSegment::new("phase", tick.market_id),
                        WindowSegment::new("state", tick.fixture_id),
                    ]
                },
                |_| Vec::new(),
            )
            .roll_up_with_segment_projection(
                "MarketPriced",
                |tick| tick.fixture_id,
                |children| children.any_active(),
                |projection| projection.rename("state", "phase"),
            )
            .build();

        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "InPlay",
                fixture_id: "Suspended",
                price: 1.01,
                observed_at: 100,
            },
            None,
            None,
        );
    }

    #[test]
    fn callbacks_run_window_specific_before_global_callbacks() {
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let opened = Arc::clone(&calls);
        let closed = Arc::clone(&calls);
        let global = Arc::clone(&calls);
        let maintenance = Arc::clone(&calls);

        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .on_emission(move |emission| {
                global
                    .lock()
                    .expect("callback lock")
                    .push(format!("global:{:?}", emission.kind));
            })
            .track_window_with_options(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
                move |options| {
                    let opened = Arc::clone(&opened);
                    let closed = Arc::clone(&closed);
                    options
                        .on_opened(move |emission| {
                            opened
                                .lock()
                                .expect("callback lock")
                                .push(format!("opened:{}", emission.window_name));
                        })
                        .on_closed(move |emission| {
                            closed
                                .lock()
                                .expect("callback lock")
                                .push(format!("closed:{}", emission.window_name));
                        })
                },
            )
            .track_window_with_options(
                "SelectionMaintenance",
                |tick| tick.selection_id,
                |tick| tick.price < 0.0,
                move |options| {
                    options.on_opened(move |emission| {
                        maintenance
                            .lock()
                            .expect("callback lock")
                            .push(format!("maintenance:{}", emission.window_name));
                    })
                },
            )
            .build();

        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            None,
            None,
        );
        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 101,
            },
            None,
            None,
        );

        assert_eq!(
            &*calls.lock().expect("callback lock"),
            &[
                "opened:SelectionSuspension",
                "global:Opened",
                "closed:SelectionSuspension",
                "global:Closed",
            ]
        );
    }

    #[test]
    fn active_predicate_close_records_boundary_reason() {
        let mut pipeline = for_events::<PriceTick>()
            .record_windows()
            .track_window(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
            )
            .build();

        pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            None,
            None,
        );
        let result = pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 101,
            },
            None,
            None,
        );

        assert_eq!(
            pipeline.history().closed_windows()[0].boundary_reason,
            Some(WindowBoundaryReason::ActivePredicateEnded)
        );
        assert!(
            pipeline.history().closed_windows()[0]
                .boundary_changes
                .is_empty()
        );
        assert_eq!(
            result.emissions[0].boundary_reason,
            Some(WindowBoundaryReason::ActivePredicateEnded)
        );
    }

    #[test]
    fn window_recording_is_opt_in_but_emissions_still_fire() {
        let mut pipeline = for_events::<PriceTick>()
            .track_window(
                "SelectionSuspension",
                |tick| tick.selection_id,
                |tick| tick.price == 0.0,
            )
            .build();

        let opened = pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            None,
            None,
        );
        let closed = pipeline.ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 101,
            },
            None,
            None,
        );

        assert_eq!(opened.emissions.len(), 1);
        assert_eq!(closed.emissions.len(), 1);
        assert!(pipeline.history().open_windows().is_empty());
        assert!(pipeline.history().closed_windows().is_empty());
    }
}
