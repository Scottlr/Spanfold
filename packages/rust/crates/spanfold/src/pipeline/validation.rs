use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use super::definitions::{RollUpDefinition, WindowCallbackSet, WindowDefinition};

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

pub(super) fn validate_window_names<T>(
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

pub(super) fn validate_segment_projections<T>(
    windows: &[WindowDefinition<T>],
) -> Result<(), EventPipelineBuildError> {
    for window in windows {
        for rollup in &window.rollups {
            validate_rollup_projection(rollup)?;
        }
    }
    Ok(())
}

pub(super) fn window_definition_count<T>(definition: &WindowDefinition<T>) -> Option<u64> {
    definition.rollups.iter().try_fold(1_u64, |total, rollup| {
        total.checked_add(rollup_definition_count(rollup)?)
    })
}

fn rollup_definition_count<T>(definition: &RollUpDefinition<T>) -> Option<u64> {
    definition.rollups.iter().try_fold(1_u64, |total, rollup| {
        total.checked_add(rollup_definition_count(rollup)?)
    })
}

pub(super) fn collect_window_callbacks<T>(
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
