#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Core Rust primitives for Spanfold.
//!
//! This crate is the start of Spanfold's Rust 1.95.0 / Rust 2024
//! implementation. It intentionally begins with strongly typed data structures
//! and builders that future comparison, export, and CLI work can use without a
//! mechanical translation from the .NET implementation.
//!
//! ```rust
//! use spanfold::for_events;
//!
//! #[derive(Clone)]
//! struct Event { id: &'static str, active: bool }
//!
//! let mut pipeline = for_events::<Event>()
//!     .record_windows()
//!     .track_window("Active", |event| event.id, |event| event.active)
//!     .build();
//! pipeline.ingest(Event { id: "one", active: true }, Some("source-a"), None)?;
//! # Ok::<(), spanfold::IngestionError>(())
//! ```

mod analytics;
mod builders;
mod changelog;
mod comparison;
mod explain;
/// Portable comparison artifact encoders and atomic file sinks.
pub mod export;
mod extensions;
mod fixture;
mod liveness;
/// Event-driven window pipeline and fallible ingestion API.
pub mod pipeline;
mod primitive;
/// Window history, records, and query/summarization APIs.
pub mod records;
/// Temporal axes, points, ranges, and domain validation.
pub mod temporal;
mod testing;

pub use analytics::{
    HierarchyComparisonResult, HierarchyComparisonRow, HierarchyComparisonRowKind,
    SourceMatrixCell, SourceMatrixResult, compare_hierarchy, compare_sources,
};
pub use builders::{ComparisonSelectorBuilder, WindowComparisonBuilder};
pub use changelog::{
    ComparisonChangeKind, ComparisonChangelogEntry, create_changelog, replay_changelog,
};
pub use comparison::{
    AgainstSelection, AlignedComparison, AlignedSegmentArtifact, AsOfDirection, AsOfMatchStatus,
    AsOfRow, CohortActivity, Comparator, ComparatorParseError, ComparatorSummary,
    ComparisonDiagnostic, ComparisonDuplicateWindowPolicy, ComparisonFinality,
    ComparisonNormalizationPolicy, ComparisonNullTimestampPolicy, ComparisonOutputOptions,
    ComparisonPlan, ComparisonResult, ComparisonRowFinality, ComparisonRows, ComparisonScope,
    ComparisonSelector, ComparisonSelectorError, ComparisonSide, ContainmentRow, ContainmentStatus,
    CoverageRow, CoverageSummary, DiagnosticSeverity, ExcludedWindowRecord, GapRow,
    LeadLagDirection, LeadLagRow, LeadLagSummary, LeadLagTransition, MissingRow,
    NormalizedWindowRecord, OpenWindowPolicy, OverlapRow, PreparedComparison, ResidualRow,
    RowPoint, RowRange, SymmetricDifferenceRow, WindowArtifact, WindowFilter, align, compare,
    compare_live, prepare, prepare_live,
};
pub use explain::ComparisonExplanationFormat;
pub use export::{
    ComparisonDebugHtmlOptions, ComparisonExportError, ComparisonLlmContextOptions,
    export_plan_json, export_result_debug_html, export_result_json, export_result_json_lines,
    export_result_llm_context, export_result_markdown, write_result_json_lines,
};
pub use extensions::{
    CohortEvidenceMetadata, ComparisonExtensionBuildError, ComparisonExtensionBuilder,
    ComparisonExtensionComparator, ComparisonExtensionDescriptor, ComparisonExtensionMetadata,
    ComparisonExtensionSelector,
};
pub use fixture::{ContractFixture, FixtureError};
pub use liveness::{LaneKey, LaneLivenessError, LaneLivenessSignal, LaneLivenessTracker};
pub use pipeline::{
    ChildActivityView, EventPipeline, EventPipelineBuildError, EventPipelineBuilder,
    EventPipelineMetadata, IngestionError, IngestionResult, RollUpSegmentProjection,
    WindowEmission, WindowMetadata, WindowOptions, WindowPipelineBuilder, WindowTransitionKind,
    for_events,
};
pub use primitive::{PrimitiveValue, PrimitiveValueError};
pub use records::{
    ClosedWindow, OpenWindow, SummaryError, WindowAnnotation, WindowAnnotationTarget,
    WindowBoundaryChange, WindowBoundaryReason, WindowGroupKind, WindowGroupSummary, WindowHistory,
    WindowHistoryFixture, WindowHistoryFixtureWindow, WindowHistoryQuery, WindowHistorySnapshot,
    WindowOverlap, WindowRecord, WindowRecordId, WindowResidualSegment, WindowSegment,
    WindowSnapshotQuery, WindowSnapshotRecord, WindowTag, summarize_by_segment, summarize_by_tag,
};
pub use temporal::{
    TemporalAxis, TemporalPoint, TemporalPointError, TemporalRange, TemporalRangeError,
};
pub use testing::{
    SpanfoldAssert, SpanfoldAssertionError, SpanfoldSnapshot, VirtualComparisonClock,
    WindowHistoryFixtureBuilder, WindowHistoryFixtureWindowBuilder,
};
