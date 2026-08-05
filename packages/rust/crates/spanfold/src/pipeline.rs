use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    marker::PhantomData,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ClosedWindow, OpenWindow, TemporalPoint, TemporalRange, WindowBoundaryChange,
    WindowBoundaryReason, WindowHistory, WindowRecordId, WindowSegment, WindowTag,
};

mod definitions;
mod runtime;

use definitions::*;
use runtime::*;

type SegmentTransform =
    Arc<dyn Fn(&crate::PrimitiveValue) -> crate::PrimitiveValue + Send + Sync + 'static>;

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
    /// A segment projection cannot produce a deterministic unique shape.
    #[error("invalid segment projection: {0}")]
    InvalidSegmentProjection(String),
}

/// Error returned when an event cannot be committed to the pipeline history.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IngestionError {
    /// The event's temporal point is incompatible with an active window.
    #[error(transparent)]
    Temporal(#[from] crate::TemporalRangeError),
    /// The processing-position counter cannot advance further.
    #[error("processing position overflow")]
    ProcessingPositionOverflow,
    /// The stable window-record counter cannot allocate another ID.
    #[error("window record id overflow")]
    RecordIdOverflow,
    /// Runtime segment values produced a non-unique projected shape.
    #[error("invalid segment projection: {0}")]
    InvalidSegmentProjection(String),
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
    ///
    /// Pipeline-generated IDs are unique within one pipeline instance. They are
    /// intentionally not globally durable; use semantic row IDs for exports.
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
    definitions: PipelineDefinitions<T>,
    runtime: PipelineRuntime,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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
            exit_when: None,
            enter_after: 1,
            exit_after: 1,
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

    /// Builds the pipeline or returns a configuration error.
    pub fn build(self) -> Result<EventPipeline<T>, EventPipelineBuildError> {
        self.try_build()
    }

    /// Builds the pipeline and panics if configuration is invalid.
    ///
    /// Prefer [`Self::build`] for user-provided configuration.
    #[must_use]
    #[track_caller]
    pub fn build_or_panic(self) -> EventPipeline<T> {
        self.try_build().expect("valid pipeline configuration")
    }

    /// Builds the pipeline or returns a configuration error.
    pub fn try_build(self) -> Result<EventPipeline<T>, EventPipelineBuildError> {
        validate_window_names(&self.windows)?;
        validate_segment_projections(&self.windows)?;
        let window_callbacks = collect_window_callbacks(&self.windows);
        let max_new_records = self.windows.iter().try_fold(0_u64, |total, definition| {
            total.checked_add(window_definition_count(definition)?)
        });
        let observation_buffer = Vec::with_capacity(self.windows.len());
        Ok(EventPipeline {
            definitions: PipelineDefinitions {
                windows: self.windows,
                max_new_records,
                event_time: self.event_time,
                emission_callbacks: self.emission_callbacks,
                window_callbacks,
            },
            runtime: PipelineRuntime {
                observation_buffer,
                record_windows: self.record_windows,
                history: WindowHistory::new(),
                active: HashMap::new(),
                pending_confirmations: HashMap::new(),
                parents: HashMap::new(),
                rollup_memberships: HashMap::new(),
                position: 0,
                next_record_id: 0,
            },
            marker: PhantomData,
        })
    }
}

impl<T> WindowPipelineBuilder<T> {
    /// Requires consecutive source-window observations before transitions commit.
    ///
    /// The source window's active predicate is the enter predicate. The event
    /// reaching each configured count becomes that transition's boundary.
    ///
    /// # Panics
    ///
    /// Panics if either confirmation count is zero.
    #[must_use]
    pub fn stabilize<P>(mut self, exit_when: P, enter_after: usize, exit_after: usize) -> Self
    where
        P: Fn(&T) -> bool + Send + Sync + 'static,
    {
        assert!(enter_after > 0, "enter confirmation count must be positive");
        assert!(exit_after > 0, "exit confirmation count must be positive");

        let source = &mut self.builder.windows[self.path[0]];
        source.exit_when = Some(Arc::new(exit_when));
        source.enter_after = enter_after;
        source.exit_after = exit_after;
        self
    }

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

    /// Builds the pipeline or returns a configuration error.
    pub fn build(self) -> Result<EventPipeline<T>, EventPipelineBuildError> {
        self.builder.build()
    }

    /// Builds the pipeline and panics if configuration is invalid.
    #[must_use]
    #[track_caller]
    pub fn build_or_panic(self) -> EventPipeline<T> {
        self.builder.build_or_panic()
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
        self.runtime.position
    }

    /// Returns the recorded window history.
    #[must_use]
    pub fn history(&self) -> &WindowHistory {
        &self.runtime.history
    }

    /// Returns configured pipeline metadata.
    #[must_use]
    pub fn metadata(&self) -> EventPipelineMetadata {
        EventPipelineMetadata {
            event_type: Some(std::any::type_name::<T>().to_owned()),
            windows: self
                .definitions
                .windows
                .iter()
                .map(window_metadata)
                .collect(),
        }
    }

    /// Ingests one event with optional source and partition context.
    pub fn ingest(
        &mut self,
        event: T,
        source: Option<&str>,
        partition: Option<&str>,
    ) -> Result<IngestionResult, IngestionError> {
        let result = self
            .runtime
            .ingest(&self.definitions, event, source, partition)?;
        for emission in &result.emissions {
            self.definitions.invoke_callbacks(emission);
        }
        Ok(result)
    }

    /// Ingests multiple events sequentially and returns all emitted transitions.
    pub fn ingest_many<I>(
        &mut self,
        events: I,
        source: Option<&str>,
        partition: Option<&str>,
    ) -> Result<IngestionResult, IngestionError>
    where
        I: IntoIterator<Item = T>,
    {
        let mut emissions = Vec::new();
        for event in events {
            emissions.extend(self.ingest(event, source, partition)?.emissions);
        }
        Ok(IngestionResult {
            processing_position: self.runtime.position,
            emissions,
        })
    }
}

impl<T> PipelineDefinitions<T> {
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
}

impl PipelineRuntime {
    fn ingest<T>(
        &mut self,
        definitions: &PipelineDefinitions<T>,
        event: T,
        source: Option<&str>,
        partition: Option<&str>,
    ) -> Result<IngestionResult, IngestionError> {
        let next_position = self
            .position
            .checked_add(1)
            .ok_or(IngestionError::ProcessingPositionOverflow)?;
        let max_new_records = definitions
            .max_new_records
            .ok_or(IngestionError::RecordIdOverflow)?;
        if self.next_record_id > u64::MAX.saturating_sub(max_new_records) {
            return Err(IngestionError::RecordIdOverflow);
        }
        let event_point = definitions.event_time.as_ref().map_or_else(
            || TemporalPoint::position(next_position),
            |selector| TemporalPoint::timestamp_ticks(selector(&event)),
        );
        for active in self.active.values() {
            TemporalRange::new(active.start.clone(), event_point.clone())?;
        }
        let mut observations = std::mem::take(&mut self.observation_buffer);
        if let Err(error) =
            self.observe_event(definitions, &event, source, partition, &mut observations)
        {
            observations.clear();
            self.observation_buffer = observations;
            return Err(error);
        }
        self.position = next_position;
        let mut emissions = Vec::new();
        let mut ingestion_error = None;
        for (definition, observation) in definitions.windows.iter().zip(observations.drain(..)) {
            if let Err(error) = self.ingest_definition(
                definition,
                &event,
                observation,
                event_point.clone(),
                source,
                partition,
                &mut emissions,
            ) {
                ingestion_error = Some(error);
                break;
            }
        }
        self.observation_buffer = observations;
        if let Some(error) = ingestion_error {
            return Err(error);
        }
        Ok(IngestionResult {
            processing_position: self.position,
            emissions,
        })
    }

    fn observe_event<T>(
        &self,
        definitions: &PipelineDefinitions<T>,
        event: &T,
        source: Option<&str>,
        partition: Option<&str>,
        observations: &mut Vec<EventWindowObservation>,
    ) -> Result<(), IngestionError> {
        observations.clear();
        for definition in &definitions.windows {
            let key = (definition.key)(event);
            let state_key = (
                definition.name.clone(),
                key.clone(),
                source.map(str::to_owned),
                partition.map(str::to_owned),
                String::new(),
            );
            let was_active = self.active.contains_key(&state_key);
            let predicate_matches = if was_active {
                definition.exit_when.as_ref().map_or_else(
                    || !(definition.is_active)(event),
                    |predicate| predicate(event),
                )
            } else {
                (definition.is_active)(event)
            };
            let lifecycle = if predicate_matches {
                let next_confirmation = self
                    .pending_confirmations
                    .get(&state_key)
                    .copied()
                    .unwrap_or_default()
                    .saturating_add(1);
                let required = if was_active {
                    definition.exit_after
                } else {
                    definition.enter_after
                };
                if next_confirmation < required {
                    SourceWindowLifecycle::Pending(next_confirmation)
                } else if was_active {
                    SourceWindowLifecycle::Closing
                } else {
                    SourceWindowLifecycle::Active
                }
            } else if was_active {
                SourceWindowLifecycle::Active
            } else {
                SourceWindowLifecycle::Inactive
            };
            let has_active_metadata = matches!(lifecycle, SourceWindowLifecycle::Active);
            let segments = if has_active_metadata {
                definition
                    .segments
                    .as_ref()
                    .map_or_else(Vec::new, |selector| selector(event))
            } else {
                Vec::new()
            };
            let tags = if has_active_metadata {
                definition
                    .tags
                    .as_ref()
                    .map_or_else(Vec::new, |selector| selector(event))
            } else {
                Vec::new()
            };
            let rollups = if has_active_metadata {
                observe_rollup_projections(&definition.rollups, &segments)?
            } else {
                Vec::new()
            };
            observations.push(EventWindowObservation {
                state_key,
                key,
                lifecycle,
                segments,
                tags,
                rollups,
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn ingest_definition<T>(
        &mut self,
        definition: &WindowDefinition<T>,
        event: &T,
        observation: EventWindowObservation,
        event_point: TemporalPoint,
        source: Option<&str>,
        partition: Option<&str>,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        let EventWindowObservation {
            state_key,
            key,
            lifecycle,
            segments,
            tags,
            rollups,
        } = observation;
        if let SourceWindowLifecycle::Pending(confirmation_count) = lifecycle {
            self.pending_confirmations
                .insert(state_key, confirmation_count);
            return Ok(());
        }

        self.pending_confirmations.remove(&state_key);
        let previous = self.active.get(&state_key).cloned();
        let is_active = matches!(lifecycle, SourceWindowLifecycle::Active);
        emissions.extend(self.sync_window_state(WindowObservation {
            state_key,
            window_name: definition.name.clone(),
            key: key.clone(),
            event_point: event_point.clone(),
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            is_active,
            segments: segments.clone(),
            tags: tags.clone(),
        })?);

        if is_active {
            self.sync_rollups(
                &definition.rollups,
                event,
                Some(rollups),
                source,
                partition,
                ChildContext {
                    lineage: &definition.name,
                    key: &key,
                    membership_context: "",
                    event_point: event_point.clone(),
                    is_active: true,
                    segments: &segments,
                    tags: &tags,
                },
                emissions,
            )?;
        } else if matches!(lifecycle, SourceWindowLifecycle::Closing)
            && let Some(previous) = previous
        {
            self.sync_rollups(
                &definition.rollups,
                event,
                None,
                source,
                partition,
                ChildContext {
                    lineage: &definition.name,
                    key: &key,
                    membership_context: "",
                    event_point: event_point.clone(),
                    is_active: false,
                    segments: &previous.segments,
                    tags: &previous.tags,
                },
                emissions,
            )?;
        } else {
            self.sync_rollups(
                &definition.rollups,
                event,
                None,
                source,
                partition,
                ChildContext {
                    lineage: &definition.name,
                    key: &key,
                    membership_context: "",
                    event_point: event_point.clone(),
                    is_active: false,
                    segments: &[],
                    tags: &[],
                },
                emissions,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn sync_rollups<T>(
        &mut self,
        definitions: &[RollUpDefinition<T>],
        event: &T,
        observations: Option<Vec<EventRollupObservation>>,
        source: Option<&str>,
        partition: Option<&str>,
        child: ChildContext<'_>,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        let mut observations = observations.map(Vec::into_iter);
        for definition in definitions {
            self.sync_rollup(
                definition,
                event,
                observations.as_mut().and_then(Iterator::next),
                source,
                partition,
                child.clone(),
                emissions,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn sync_rollup<T>(
        &mut self,
        definition: &RollUpDefinition<T>,
        event: &T,
        observation: Option<EventRollupObservation>,
        source: Option<&str>,
        partition: Option<&str>,
        child: ChildContext<'_>,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        let (projected_segments, segment_context, rollup_observations) = match observation {
            Some(observation) => (
                observation.segments,
                observation.segment_context,
                Some(observation.rollups),
            ),
            None => {
                let segments = project_segments(&definition.segment_projection, child.segments)?;
                let segment_context = stable_segments(&segments);
                (segments, segment_context, None)
            }
        };
        let parent_key = (definition.key)(event);
        let rollup_lineage = format!("{}>{}", child.lineage, definition.name);
        let membership_key = (
            rollup_lineage.clone(),
            child.key.to_owned(),
            source.map(str::to_owned),
            partition.map(str::to_owned),
            child.membership_context.to_owned(),
        );
        let current_membership = RollupMembership {
            parent_key,
            segment_context,
            segments: projected_segments,
            tags: child.tags.to_vec(),
        };
        let previous_membership = self.rollup_memberships.get(&membership_key).cloned();

        if child.is_active {
            if let Some(previous) = previous_membership.as_ref().filter(|previous| {
                previous.parent_key != current_membership.parent_key
                    || previous.segment_context != current_membership.segment_context
            }) {
                self.remove_rollup_parent(
                    definition,
                    event,
                    None,
                    source,
                    partition,
                    &rollup_lineage,
                    child.key,
                    child.membership_context,
                    child.event_point.clone(),
                    previous,
                    emissions,
                )?;
            }
            self.rollup_memberships
                .insert(membership_key, current_membership.clone());
            self.update_rollup_parent(
                definition,
                event,
                rollup_observations,
                source,
                partition,
                &rollup_lineage,
                child.key,
                child.membership_context,
                child.event_point,
                &current_membership,
                true,
                emissions,
            )?;
        } else {
            if let Some(previous) = previous_membership.as_ref().filter(|previous| {
                previous.parent_key != current_membership.parent_key
                    || previous.segment_context != current_membership.segment_context
            }) {
                self.remove_rollup_parent(
                    definition,
                    event,
                    None,
                    source,
                    partition,
                    &rollup_lineage,
                    child.key,
                    child.membership_context,
                    child.event_point.clone(),
                    previous,
                    emissions,
                )?;
            }
            self.rollup_memberships
                .insert(membership_key, current_membership.clone());
            self.update_rollup_parent(
                definition,
                event,
                None,
                source,
                partition,
                &rollup_lineage,
                child.key,
                child.membership_context,
                child.event_point,
                &current_membership,
                false,
                emissions,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn remove_rollup_parent<T>(
        &mut self,
        definition: &RollUpDefinition<T>,
        event: &T,
        observations: Option<Vec<EventRollupObservation>>,
        source: Option<&str>,
        partition: Option<&str>,
        rollup_lineage: &str,
        child_key: &str,
        child_membership_context: &str,
        event_point: TemporalPoint,
        membership: &RollupMembership,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        let parent_state_key = (
            rollup_lineage.to_owned(),
            membership.parent_key.clone(),
            source.map(str::to_owned),
            partition.map(str::to_owned),
            membership.segment_context.clone(),
        );
        let is_active = {
            let Some(parent_state) = self.parents.get_mut(&parent_state_key) else {
                return Ok(());
            };
            let child_id = RollupChildId {
                key: child_key.to_owned(),
                membership_context: child_membership_context.to_owned(),
            };
            if !parent_state.known_children.remove(&child_id) {
                return Ok(());
            }
            parent_state.active_children.remove(&child_id);
            (definition.is_active)(parent_state.view())
        };

        emissions.extend(self.sync_window_state(WindowObservation {
            state_key: (
                definition.name.clone(),
                membership.parent_key.clone(),
                source.map(str::to_owned),
                partition.map(str::to_owned),
                membership.segment_context.clone(),
            ),
            window_name: definition.name.clone(),
            key: membership.parent_key.clone(),
            event_point: event_point.clone(),
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            is_active,
            segments: membership.segments.clone(),
            tags: membership.tags.clone(),
        })?);

        self.sync_rollups(
            &definition.rollups,
            event,
            observations,
            source,
            partition,
            ChildContext {
                lineage: &definition.name,
                key: &membership.parent_key,
                membership_context: &membership.segment_context,
                event_point,
                is_active,
                segments: &membership.segments,
                tags: &membership.tags,
            },
            emissions,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn update_rollup_parent<T>(
        &mut self,
        definition: &RollUpDefinition<T>,
        event: &T,
        observations: Option<Vec<EventRollupObservation>>,
        source: Option<&str>,
        partition: Option<&str>,
        rollup_lineage: &str,
        child_key: &str,
        child_membership_context: &str,
        event_point: TemporalPoint,
        membership: &RollupMembership,
        child_is_active: bool,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        let parent_state_key = (
            rollup_lineage.to_owned(),
            membership.parent_key.clone(),
            source.map(str::to_owned),
            partition.map(str::to_owned),
            membership.segment_context.clone(),
        );
        let is_active = {
            let parent_state = self.parents.entry(parent_state_key.clone()).or_default();
            let child_id = RollupChildId {
                key: child_key.to_owned(),
                membership_context: child_membership_context.to_owned(),
            };
            parent_state.known_children.insert(child_id.clone());
            if child_is_active {
                parent_state.active_children.insert(child_id);
            } else {
                parent_state.active_children.remove(&child_id);
            }
            (definition.is_active)(parent_state.view())
        };

        emissions.extend(self.sync_window_state(WindowObservation {
            state_key: (
                definition.name.clone(),
                membership.parent_key.clone(),
                source.map(str::to_owned),
                partition.map(str::to_owned),
                membership.segment_context.clone(),
            ),
            window_name: definition.name.clone(),
            key: membership.parent_key.clone(),
            event_point: event_point.clone(),
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            is_active,
            segments: membership.segments.clone(),
            tags: membership.tags.clone(),
        })?);

        self.sync_rollups(
            &definition.rollups,
            event,
            observations,
            source,
            partition,
            ChildContext {
                lineage: &definition.name,
                key: &membership.parent_key,
                membership_context: &membership.segment_context,
                event_point,
                is_active,
                segments: &membership.segments,
                tags: &membership.tags,
            },
            emissions,
        )?;
        Ok(())
    }

    fn sync_window_state(
        &mut self,
        observation: WindowObservation,
    ) -> Result<Vec<WindowEmission>, IngestionError> {
        if observation.is_active {
            if let Some(previous) = self.active.get(&observation.state_key) {
                if previous.segments == observation.segments {
                    if previous.tags != observation.tags {
                        if let Some(state) = self.active.get_mut(&observation.state_key) {
                            state.tags = observation.tags.clone();
                        }
                        if self.record_windows
                            && let Some(id) = self
                                .active
                                .get(&observation.state_key)
                                .map(|state| state.id.clone())
                        {
                            self.history.update_open_tags(&id, observation.tags.clone());
                        }
                    }
                    return Ok(Vec::new());
                }
                let mut emissions = Vec::new();
                let changes = segment_changes(&previous.segments, &observation.segments);
                if let Some(emission) = self.close_window_state(
                    &observation.state_key,
                    observation.event_point.clone(),
                    Some(WindowBoundaryReason::SegmentChanged),
                    changes,
                )? {
                    emissions.push(emission);
                }
                emissions.push(self.open_window_state(observation)?);
                return Ok(emissions);
            }
            return Ok(vec![self.open_window_state(observation)?]);
        }

        Ok(self
            .close_window_state(
                &observation.state_key,
                observation.event_point,
                Some(WindowBoundaryReason::ActivePredicateEnded),
                Vec::new(),
            )?
            .into_iter()
            .collect())
    }

    fn open_window_state(
        &mut self,
        observation: WindowObservation,
    ) -> Result<WindowEmission, IngestionError> {
        let id = self.next_id()?;
        let open = OpenWindow {
            id: id.clone(),
            window_name: observation.window_name.clone(),
            key: observation.key.clone(),
            start: observation.event_point.clone(),
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
                start: observation.event_point.clone(),
                source: observation.source.clone(),
                partition: observation.partition.clone(),
                segments: observation.segments.clone(),
                tags: observation.tags.clone(),
            },
        );
        Ok(WindowEmission {
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
        })
    }

    fn close_window_state(
        &mut self,
        state_key: &RuntimeStateKey,
        event_point: TemporalPoint,
        boundary_reason: Option<WindowBoundaryReason>,
        boundary_changes: Vec<WindowBoundaryChange>,
    ) -> Result<Option<WindowEmission>, IngestionError> {
        let Some(open_state) = self.active.get(state_key).cloned() else {
            return Ok(None);
        };
        let range = TemporalRange::new(open_state.start.clone(), event_point.clone())?;
        self.active.remove(state_key);
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
        if self.record_windows {
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
        Ok(Some(emission))
    }

    fn next_id(&mut self) -> Result<WindowRecordId, IngestionError> {
        let id = WindowRecordId::generated(format!("pipeline-{:04}", self.next_record_id));
        self.next_record_id = self
            .next_record_id
            .checked_add(1)
            .ok_or(IngestionError::RecordIdOverflow)?;
        Ok(id)
    }
}

fn window_metadata<T>(definition: &WindowDefinition<T>) -> WindowMetadata {
    WindowMetadata {
        name: definition.name.clone(),
        rollups: definition.rollups.iter().map(rollup_metadata).collect(),
    }
}

fn window_definition_count<T>(definition: &WindowDefinition<T>) -> Option<u64> {
    definition.rollups.iter().try_fold(1_u64, |total, rollup| {
        total.checked_add(rollup_definition_count(rollup)?)
    })
}

fn rollup_definition_count<T>(definition: &RollUpDefinition<T>) -> Option<u64> {
    definition.rollups.iter().try_fold(1_u64, |total, rollup| {
        total.checked_add(rollup_definition_count(rollup)?)
    })
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

fn validate_segment_projections<T>(
    windows: &[WindowDefinition<T>],
) -> Result<(), EventPipelineBuildError> {
    for window in windows {
        for rollup in &window.rollups {
            validate_rollup_projection(rollup)?;
        }
    }
    Ok(())
}

fn validate_rollup_projection<T>(
    rollup: &RollUpDefinition<T>,
) -> Result<(), EventPipelineBuildError> {
    let mut projected_names = BTreeSet::new();
    for (original, projected) in &rollup.segment_projection.renamed_names {
        if original.trim().is_empty() || projected.trim().is_empty() {
            return Err(EventPipelineBuildError::InvalidSegmentProjection(
                "segment names cannot be empty".to_owned(),
            ));
        }
        if !projected_names.insert(projected) {
            return Err(EventPipelineBuildError::InvalidSegmentProjection(format!(
                "multiple renames target '{projected}'"
            )));
        }
        if rollup
            .segment_projection
            .renamed_names
            .keys()
            .any(|name| name != original && name == projected)
        {
            return Err(EventPipelineBuildError::InvalidSegmentProjection(format!(
                "rename target '{projected}' collides with a source name"
            )));
        }
    }
    for child in &rollup.rollups {
        validate_rollup_projection(child)?;
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
) -> Result<Vec<WindowSegment>, IngestionError> {
    if segments.is_empty()
        || (projection.preserved_names.is_none()
            && projection.dropped_names.is_empty()
            && projection.renamed_names.is_empty()
            && projection.value_transforms.is_empty())
    {
        return Ok(segments.to_vec());
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
        if !selected_projected_names.insert(projected_name.clone()) {
            return Err(IngestionError::InvalidSegmentProjection(format!(
                "projected segment '{projected_name}' is not unique"
            )));
        }
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

    Ok(projected)
}

fn observe_rollup_projections<T>(
    definitions: &[RollUpDefinition<T>],
    child_segments: &[WindowSegment],
) -> Result<Vec<EventRollupObservation>, IngestionError> {
    let mut observations = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let segments = project_segments(&definition.segment_projection, child_segments)?;
        let segment_context = stable_segments(&segments);
        let rollups = observe_rollup_projections(&definition.rollups, &segments)?;
        observations.push(EventRollupObservation {
            segments,
            segment_context,
            rollups,
        });
    }
    Ok(observations)
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
    let mut stable = String::new();
    for segment in segments {
        let parent = segment.parent_name.as_deref().unwrap_or_default();
        let value = serde_json::to_string(&segment.value).unwrap_or_default();
        for part in [parent, segment.name.as_str(), value.as_str()] {
            stable.push_str(&part.len().to_string());
            stable.push(':');
            stable.push_str(part);
            stable.push('|');
        }
    }
    stable
}

fn segment_changes(
    previous: &[WindowSegment],
    current: &[WindowSegment],
) -> Vec<WindowBoundaryChange> {
    let mut changes = Vec::new();
    let mut names = BTreeSet::new();
    names.extend(previous.iter().map(|segment| segment.name.as_str()));
    names.extend(current.iter().map(|segment| segment.name.as_str()));
    for name in names {
        let before = previous.iter().find(|segment| segment.name == name);
        let after = current.iter().find(|segment| segment.name == name);
        if before == after {
            continue;
        }
        changes.push(WindowBoundaryChange {
            segment_name: name.to_owned(),
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
#[path = "pipeline_tests.rs"]
mod tests;
