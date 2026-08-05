use std::{marker::PhantomData, sync::Arc};

use super::{
    ChildActivityView, EventPipeline, WindowEmission,
    definitions::{
        ActivePredicate, EmissionCallback, EventTimeSelector, KeySelector, PipelineDefinitions,
        RollUpDefinition, RollupPredicate, SegmentSelector, TagSelector, WindowCallbackSet,
        WindowDefinition,
    },
    runtime::PipelineRuntime,
    validation::{
        EventPipelineBuildError, collect_window_callbacks, validate_segment_projections,
        validate_window_names, window_definition_count,
    },
};

type SegmentTransform =
    Arc<dyn Fn(&crate::PrimitiveValue) -> crate::PrimitiveValue + Send + Sync + 'static>;

/// Configures which child segment dimensions a roll-up preserves.
#[derive(Clone, Default)]
pub struct RollUpSegmentProjection {
    pub(super) preserved_names: Option<std::collections::BTreeSet<String>>,
    pub(super) dropped_names: std::collections::BTreeSet<String>,
    pub(super) renamed_names: std::collections::BTreeMap<String, String>,
    pub(super) value_transforms: std::collections::BTreeMap<String, SegmentTransform>,
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
            .get_or_insert_with(std::collections::BTreeSet::new)
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

    /// Transforms a segment value before it is emitted on the roll-up.
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
    pub(super) callbacks: WindowCallbackSet,
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

/// Builder for an event ingestion pipeline.
#[derive(Clone)]
pub struct EventPipelineBuilder<T> {
    pub(super) windows: Vec<WindowDefinition<T>>,
    pub(super) event_time: Option<EventTimeSelector<T>>,
    pub(super) emission_callbacks: Vec<EmissionCallback>,
    pub(super) record_windows: bool,
    pub(super) marker: PhantomData<T>,
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
    pub(super) builder: EventPipelineBuilder<T>,
    pub(super) path: Vec<usize>,
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
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            None,
            None,
            WindowCallbackSet::default(),
        ));
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
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            None,
            None,
            options.callbacks,
        ));
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
        S: Fn(&T) -> Vec<crate::WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<crate::WindowTag> + Send + Sync + 'static,
    {
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            Some(Arc::new(segments)),
            Some(Arc::new(tags)),
            WindowCallbackSet::default(),
        ));
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
        S: Fn(&T) -> Vec<crate::WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<crate::WindowTag> + Send + Sync + 'static,
        C: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure(WindowOptions::new());
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            Some(Arc::new(segments)),
            Some(Arc::new(tags)),
            options.callbacks,
        ));
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
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            None,
            None,
            WindowCallbackSet::default(),
        ));
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
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            None,
            None,
            options.callbacks,
        ));
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
        S: Fn(&T) -> Vec<crate::WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<crate::WindowTag> + Send + Sync + 'static,
    {
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            Some(Arc::new(segments)),
            Some(Arc::new(tags)),
            WindowCallbackSet::default(),
        ));
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
        S: Fn(&T) -> Vec<crate::WindowSegment> + Send + Sync + 'static,
        G: Fn(&T) -> Vec<crate::WindowTag> + Send + Sync + 'static,
        C: FnOnce(WindowOptions) -> WindowOptions,
    {
        let options = configure(WindowOptions::new());
        self.windows.push(new_window_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            Some(Arc::new(segments)),
            Some(Arc::new(tags)),
            options.callbacks,
        ));
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
                history: crate::WindowHistory::new(),
                active: std::collections::HashMap::new(),
                pending_confirmations: std::collections::HashMap::new(),
                parents: std::collections::HashMap::new(),
                rollup_memberships: std::collections::HashMap::new(),
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
        let definition = new_rollup_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            WindowCallbackSet::default(),
            RollUpSegmentProjection::default(),
        );
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
        let definition = new_rollup_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            options.callbacks,
            RollUpSegmentProjection::default(),
        );
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
        let definition = new_rollup_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            WindowCallbackSet::default(),
            configure_projection(RollUpSegmentProjection::new()),
        );
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
        let definition = new_rollup_definition(
            name.into(),
            Arc::new(move |event| key(event).into()),
            Arc::new(is_active),
            options.callbacks,
            configure_projection(RollUpSegmentProjection::new()),
        );
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

fn new_window_definition<T>(
    name: String,
    key: KeySelector<T>,
    is_active: ActivePredicate<T>,
    segments: Option<SegmentSelector<T>>,
    tags: Option<TagSelector<T>>,
    callbacks: WindowCallbackSet,
) -> WindowDefinition<T> {
    WindowDefinition {
        name,
        key,
        is_active,
        exit_when: None,
        enter_after: 1,
        exit_after: 1,
        segments,
        tags,
        rollups: Vec::new(),
        callbacks,
    }
}

fn new_rollup_definition<T>(
    name: String,
    key: KeySelector<T>,
    is_active: RollupPredicate,
    callbacks: WindowCallbackSet,
    segment_projection: RollUpSegmentProjection,
) -> RollUpDefinition<T> {
    RollUpDefinition {
        name,
        key,
        is_active,
        rollups: Vec::new(),
        callbacks,
        segment_projection,
    }
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
