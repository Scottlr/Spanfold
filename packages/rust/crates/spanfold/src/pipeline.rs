use std::{collections::BTreeSet, marker::PhantomData};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::recorder::{WindowObservation as RecorderObservation, WindowRecorderError};
use crate::{
    TemporalPoint, WindowBoundaryChange, WindowBoundaryReason, WindowHistory, WindowRecordId,
    WindowRecorder, WindowSegment, WindowTag,
};

mod builder;
mod definitions;
mod rollup;
mod runtime;
mod validation;

use definitions::{PipelineDefinitions, RollUpDefinition, WindowDefinition};
use rollup::{RollupChildId, RollupMembership, RollupMembershipKey};
use runtime::{
    ChildContext, EventRollupObservation, EventWindowObservation, PipelineRuntime, RuntimeStateKey,
    SourceWindowLifecycle, WindowObservation,
};

pub use crate::recorder::WindowTransitionKind;
pub use builder::{
    EventPipelineBuilder, RollUpSegmentProjection, WindowOptions, WindowPipelineBuilder, for_events,
};
pub use validation::EventPipelineBuildError;

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
    /// A recorder observation violated the canonical window metadata contract.
    #[error("invalid window observation: {0}")]
    InvalidObservation(#[source] WindowRecorderError),
}

impl From<WindowRecorderError> for IngestionError {
    fn from(error: WindowRecorderError) -> Self {
        match error {
            WindowRecorderError::Temporal(error) => Self::Temporal(error),
            WindowRecorderError::RecordIdOverflow => Self::RecordIdOverflow,
            error => Self::InvalidObservation(error),
        }
    }
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

/// Event ingestion pipeline that records source windows and roll-ups.
pub struct EventPipeline<T> {
    definitions: PipelineDefinitions<T>,
    runtime: PipelineRuntime,
    marker: PhantomData<T>,
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
        self.runtime.recorder.history()
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
        self.recorder.ensure_capacity(max_new_records)?;
        let event_point = definitions.event_time.as_ref().map_or_else(
            || TemporalPoint::position(next_position),
            |selector| TemporalPoint::timestamp_ticks(selector(&event)),
        );
        self.recorder.validate_point(&event_point)?;
        let mut observations = std::mem::take(&mut self.observation_buffer);
        if let Err(error) =
            self.observe_event(definitions, &event, source, partition, &mut observations)
        {
            observations.clear();
            self.observation_buffer = observations;
            return Err(error);
        }
        if let Err(error) =
            self.preflight_observations(definitions, &observations, source, partition)
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

    fn preflight_observations<T>(
        &self,
        definitions: &PipelineDefinitions<T>,
        observations: &[EventWindowObservation],
        source: Option<&str>,
        partition: Option<&str>,
    ) -> Result<(), IngestionError> {
        for (definition, observation) in definitions.windows.iter().zip(observations) {
            if matches!(observation.lifecycle, SourceWindowLifecycle::Pending(_)) {
                continue;
            }
            self.recorder.validate_parts(
                &definition.name,
                &observation.key,
                source,
                partition,
                &observation.segments,
                &observation.tags,
            )?;
            let rollup_tags = match observation.lifecycle {
                SourceWindowLifecycle::Active => observation.tags.clone(),
                SourceWindowLifecycle::Closing => self
                    .recorder
                    .active_state(
                        &observation.state_key.window_name,
                        &observation.state_key.key,
                        observation.state_key.source.as_deref(),
                        observation.state_key.partition.as_deref(),
                        &observation.state_key.segment_context,
                    )
                    .map(|state| state.tags)
                    .unwrap_or_default(),
                SourceWindowLifecycle::Inactive | SourceWindowLifecycle::Pending(_) => Vec::new(),
            };
            preflight_rollup_observations(
                &self.recorder,
                &definition.rollups,
                &observation.rollups,
                source,
                partition,
                &rollup_tags,
            )?;
        }
        Ok(())
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
            let state_key = RuntimeStateKey {
                window_name: definition.name.clone(),
                key: key.clone(),
                source: source.map(str::to_owned),
                partition: partition.map(str::to_owned),
                segment_context: String::new(),
            };
            let was_active = self.recorder.is_active(
                &state_key.window_name,
                &state_key.key,
                state_key.source.as_deref(),
                state_key.partition.as_deref(),
                &state_key.segment_context,
            );
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
            let previous = if matches!(lifecycle, SourceWindowLifecycle::Closing) {
                self.recorder.active_state(
                    &state_key.window_name,
                    &state_key.key,
                    state_key.source.as_deref(),
                    state_key.partition.as_deref(),
                    &state_key.segment_context,
                )
            } else {
                None
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
            let rollup_segments = if has_active_metadata {
                segments.as_slice()
            } else if let Some(previous) = previous.as_ref() {
                previous.segments.as_slice()
            } else {
                &[]
            };
            let rollups = if matches!(lifecycle, SourceWindowLifecycle::Pending(_)) {
                Vec::new()
            } else {
                observe_rollup_projections(&definition.rollups, event, rollup_segments)?
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
        let previous = self.recorder.active_state(
            &state_key.window_name,
            &state_key.key,
            state_key.source.as_deref(),
            state_key.partition.as_deref(),
            &state_key.segment_context,
        );
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
                &rollups,
                true,
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
                &rollups,
                true,
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
                &rollups,
                true,
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
        observations: &[EventRollupObservation],
        use_observed_metadata: bool,
        source: Option<&str>,
        partition: Option<&str>,
        child: ChildContext<'_>,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        debug_assert_eq!(definitions.len(), observations.len());
        for (definition, observation) in definitions.iter().zip(observations) {
            self.sync_rollup(
                definition,
                event,
                observation,
                use_observed_metadata,
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
        observation: &EventRollupObservation,
        use_observed_metadata: bool,
        source: Option<&str>,
        partition: Option<&str>,
        child: ChildContext<'_>,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        let (projected_segments, segment_context) = if use_observed_metadata {
            (
                observation.segments.clone(),
                observation.segment_context.clone(),
            )
        } else {
            let segments = project_segments(&definition.segment_projection, child.segments)?;
            let segment_context = stable_segments(&segments);
            (segments, segment_context)
        };
        let parent_key = observation.key.clone();
        let rollup_lineage = format!("{}>{}", child.lineage, definition.name);
        let membership_key = RollupMembershipKey {
            lineage: rollup_lineage.clone(),
            child_key: child.key.to_owned(),
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            membership_context: child.membership_context.to_owned(),
        };
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
                    &observation.rollups,
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
                &observation.rollups,
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
                    &observation.rollups,
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
                &observation.rollups,
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
        observations: &[EventRollupObservation],
        source: Option<&str>,
        partition: Option<&str>,
        rollup_lineage: &str,
        child_key: &str,
        child_membership_context: &str,
        event_point: TemporalPoint,
        membership: &RollupMembership,
        emissions: &mut Vec<WindowEmission>,
    ) -> Result<(), IngestionError> {
        let parent_state_key = RuntimeStateKey {
            window_name: rollup_lineage.to_owned(),
            key: membership.parent_key.clone(),
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            segment_context: membership.segment_context.clone(),
        };
        let is_active = {
            let Some(parent_state) = self.parents.get_mut(&parent_state_key) else {
                return Ok(());
            };
            let child_id = RollupChildId {
                key: child_key.to_owned(),
                membership_context: child_membership_context.to_owned(),
            };
            let Some(view) = parent_state.remove_child(&child_id) else {
                return Ok(());
            };
            (definition.is_active)(view)
        };

        emissions.extend(self.sync_window_state(WindowObservation {
            state_key: RuntimeStateKey {
                window_name: definition.name.clone(),
                key: membership.parent_key.clone(),
                source: source.map(str::to_owned),
                partition: partition.map(str::to_owned),
                segment_context: membership.segment_context.clone(),
            },
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
            false,
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
        observations: &[EventRollupObservation],
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
        let parent_state_key = RuntimeStateKey {
            window_name: rollup_lineage.to_owned(),
            key: membership.parent_key.clone(),
            source: source.map(str::to_owned),
            partition: partition.map(str::to_owned),
            segment_context: membership.segment_context.clone(),
        };
        let is_active = {
            let parent_state = self.parents.entry(parent_state_key.clone()).or_default();
            let child_id = RollupChildId {
                key: child_key.to_owned(),
                membership_context: child_membership_context.to_owned(),
            };
            let view = parent_state.set_child_activity(child_id, child_is_active);
            (definition.is_active)(view)
        };

        emissions.extend(self.sync_window_state(WindowObservation {
            state_key: RuntimeStateKey {
                window_name: definition.name.clone(),
                key: membership.parent_key.clone(),
                source: source.map(str::to_owned),
                partition: partition.map(str::to_owned),
                segment_context: membership.segment_context.clone(),
            },
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
            true,
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
        let state_context = observation.state_key.segment_context.clone();
        // Source/current roll-up metadata was validated by
        // `preflight_observations`; old-parent removal paths reuse previously
        // accepted membership metadata and therefore do not introduce a new
        // validation surface here.
        let recorder_observation = RecorderObservation::from_validated_parts(
            observation.window_name,
            observation.key,
            observation.event_point,
            observation.is_active,
            observation.source,
            observation.partition,
            state_context,
            observation.segments,
            observation.tags,
        );
        let transitions = self.recorder.observe_validated(recorder_observation)?;
        Ok(transitions
            .into_iter()
            .map(|transition| WindowEmission {
                kind: transition.kind,
                window_name: transition.window_name,
                key: transition.key,
                record_id: transition.record_id,
                position: self.position,
                source: transition.source,
                partition: transition.partition,
                segments: transition.segments,
                tags: transition.tags,
                boundary_reason: transition.boundary_reason,
                boundary_changes: transition.boundary_changes,
            })
            .collect())
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
    event: &T,
    child_segments: &[WindowSegment],
) -> Result<Vec<EventRollupObservation>, IngestionError> {
    let mut observations = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let segments = project_segments(&definition.segment_projection, child_segments)?;
        let segment_context = stable_segments(&segments);
        let key = (definition.key)(event);
        let rollups = observe_rollup_projections(&definition.rollups, event, &segments)?;
        observations.push(EventRollupObservation {
            key,
            segments,
            segment_context,
            rollups,
        });
    }
    Ok(observations)
}

fn preflight_rollup_observations<T>(
    recorder: &WindowRecorder,
    definitions: &[RollUpDefinition<T>],
    observations: &[EventRollupObservation],
    source: Option<&str>,
    partition: Option<&str>,
    tags: &[WindowTag],
) -> Result<(), IngestionError> {
    debug_assert_eq!(definitions.len(), observations.len());
    for (definition, observation) in definitions.iter().zip(observations) {
        recorder.validate_parts(
            &definition.name,
            &observation.key,
            source,
            partition,
            &observation.segments,
            tags,
        )?;
        preflight_rollup_observations(
            recorder,
            &definition.rollups,
            &observation.rollups,
            source,
            partition,
            tags,
        )?;
    }
    Ok(())
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

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
