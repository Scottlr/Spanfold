mod annotations;
mod fixture;
mod history;
mod model;
mod query;
mod snapshot;
mod summary;

pub use annotations::{WindowAnnotation, WindowAnnotationTarget};
pub use fixture::{WindowHistoryFixture, WindowHistoryFixtureWindow};
pub use history::WindowHistory;
pub use model::{
    ClosedWindow, OpenWindow, WindowBoundaryChange, WindowBoundaryReason, WindowGroupKind,
    WindowGroupSummary, WindowHistoryFixtureError, WindowHistoryImportError, WindowHistorySnapshot,
    WindowMetadataError, WindowOverlap, WindowRecord, WindowRecordId, WindowResidualSegment,
    WindowSegment, WindowSnapshotFinality, WindowSnapshotRecord, WindowTag,
};
pub(crate) use model::{
    WindowPayloadValidationError, validate_window_metadata, validate_window_segments,
    validate_window_tags,
};
pub use query::{WindowHistoryQuery, WindowHistoryRefQuery, WindowSnapshotQuery};
pub use summary::{SummaryError, summarize_by_segment, summarize_by_tag};

#[cfg(test)]
use crate::{PrimitiveValue, TemporalPoint, TemporalRange};

#[cfg(test)]
#[path = "../records_tests.rs"]
mod tests;
