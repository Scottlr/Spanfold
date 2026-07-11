#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Core Rust primitives for Spanfold.
//!
//! This crate is the start of Spanfold's Rust 1.95.0 / Rust 2024
//! implementation. It intentionally begins with strongly typed data structures
//! and builders that future comparison, export, and CLI work can use without a
//! mechanical translation from the .NET implementation.

mod analytics;
mod builders;
mod changelog;
mod comparison;
mod explain;
mod export;
mod extensions;
mod fixture;
mod liveness;
mod pipeline;
mod primitive;
mod records;
mod temporal;
mod testing;

pub use analytics::{
    HierarchyComparisonResult, HierarchyComparisonRow, HierarchyComparisonRowKind,
    SourceMatrixCell, SourceMatrixResult, compare_hierarchy, compare_sources,
};
pub use builders::{ComparisonSelectorBuilder, WindowComparisonBuilder};
pub use changelog::{ComparisonChangelogEntry, create_changelog, replay_changelog};
pub use comparison::{
    AgainstSelection, AlignedComparison, AlignedSegmentArtifact, AsOfDirection, AsOfMatchStatus,
    AsOfRow, CohortActivity, Comparator, ComparatorSummary, ComparisonDiagnostic,
    ComparisonDuplicateWindowPolicy, ComparisonFinality, ComparisonNormalizationPolicy,
    ComparisonNullTimestampPolicy, ComparisonOutputOptions, ComparisonPlan, ComparisonResult,
    ComparisonRowFinality, ComparisonRows, ComparisonScope, ComparisonSelector,
    ComparisonSelectorError, ComparisonSide, ContainmentRow, ContainmentStatus, CoverageRow,
    CoverageSummary, DiagnosticSeverity, ExcludedWindowRecord, GapRow, LeadLagDirection,
    LeadLagRow, LeadLagSummary, LeadLagTransition, MissingRow, NormalizedWindowRecord,
    OpenWindowPolicy, OverlapRow, PreparedComparison, ResidualRow, RowPoint, RowRange,
    SymmetricDifferenceRow, WindowArtifact, WindowFilter, align, compare, compare_live, prepare,
    prepare_live,
};
pub use explain::ComparisonExplanationFormat;
pub use export::{
    ComparisonDebugHtmlOptions, ComparisonExportError, ComparisonLlmContextOptions,
    export_plan_json, export_result_debug_html, export_result_json, export_result_json_lines,
    export_result_llm_context, export_result_markdown, write_result_json_lines,
};
pub use extensions::{
    CohortEvidenceMetadata, ComparisonExtensionBuilder, ComparisonExtensionComparator,
    ComparisonExtensionDescriptor, ComparisonExtensionMetadata, ComparisonExtensionSelector,
};
pub use fixture::{ContractFixture, FixtureError};
pub use liveness::{LaneKey, LaneLivenessError, LaneLivenessSignal, LaneLivenessTracker};
pub use pipeline::{
    ChildActivityView, EventPipeline, EventPipelineBuildError, EventPipelineBuilder,
    EventPipelineMetadata, IngestionResult, RollUpSegmentProjection, WindowEmission,
    WindowMetadata, WindowOptions, WindowPipelineBuilder, WindowTransitionKind, for_events,
};
pub use primitive::PrimitiveValue;
pub use records::{
    ClosedWindow, OpenWindow, WindowAnnotation, WindowAnnotationTarget, WindowBoundaryChange,
    WindowBoundaryReason, WindowGroupKind, WindowGroupSummary, WindowHistory, WindowHistoryFixture,
    WindowHistoryFixtureWindow, WindowHistoryQuery, WindowHistorySnapshot, WindowOverlap,
    WindowRecord, WindowRecordId, WindowResidualSegment, WindowSegment, WindowSnapshotQuery,
    WindowSnapshotRecord, WindowTag, summarize_by_segment, summarize_by_tag,
};
pub use temporal::{TemporalAxis, TemporalPoint, TemporalRange, TemporalRangeError};
pub use testing::{
    SpanfoldAssert, SpanfoldAssertionError, SpanfoldSnapshot, VirtualComparisonClock,
    WindowHistoryFixtureBuilder, WindowHistoryFixtureWindowBuilder,
};
