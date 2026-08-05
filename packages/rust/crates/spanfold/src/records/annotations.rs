use serde::{Deserialize, Serialize};

use super::model::{ClosedWindow, OpenWindow, WindowRecord};
use crate::{PrimitiveValue, TemporalAxis, TemporalPoint};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
/// Stable target identity for window annotations.
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
