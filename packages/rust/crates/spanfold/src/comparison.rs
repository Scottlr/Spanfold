use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::window_normalization::{
    NormalizedWindowEvidence, RawWindowRef, WindowNormalizationFailure, WindowNormalizationRequest,
};
use crate::{
    ComparisonExtensionMetadata, TemporalAxis, TemporalPoint, WindowHistory, WindowSegment,
    WindowTag,
};

macro_rules! for_each_comparison_row_family {
    ($callback:ident) => {
        $callback! {
            (Overlap, overlap, overlap_rows, overlap_rows_with_finality, "overlap", "overlap rows"),
            (Residual, residual, residual_rows, residual_rows_with_finality, "residual", "residual rows"),
            (Missing, missing, missing_rows, missing_rows_with_finality, "missing", "missing rows"),
            (Coverage, coverage, coverage_rows, coverage_rows_with_finality, "coverage", "coverage rows"),
            (Gap, gap, gap_rows, gap_rows_with_finality, "gap", "gap rows"),
            (
                SymmetricDifference,
                symmetric_difference,
                symmetric_difference_rows,
                symmetric_difference_rows_with_finality,
                "symmetric-difference",
                "symmetric difference rows"
            ),
            (
                Containment,
                containment,
                containment_rows,
                containment_rows_with_finality,
                "containment",
                "containment rows"
            ),
            (LeadLag, lead_lag, lead_lag_rows, lead_lag_rows_with_finality, "lead-lag", "lead lag rows"),
            (AsOf, as_of, as_of_rows, as_of_rows_with_finality, "as-of", "as of rows"),
        }
    };
}
pub(crate) use for_each_comparison_row_family;

mod selector;
pub use selector::{AgainstSelection, CohortActivity, ComparisonSelector, ComparisonSelectorError};
mod diagnostics;
use diagnostics::push_diagnostic_once;
pub use diagnostics::{ComparisonDiagnostic, DiagnosticSeverity};
mod critic;
use critic::runtime_critic_diagnostics;
mod plan;
#[allow(unused_imports)]
pub(crate) use plan::ComparisonSelection;
pub use plan::{
    ComparisonDuplicateWindowPolicy, ComparisonNormalizationPolicy, ComparisonNullTimestampPolicy,
    ComparisonOutputOptions, ComparisonPlan, ComparisonScope, OpenWindowPolicy, WindowFilter,
};

mod rows;
use rows::RowAccumulator;
pub use rows::*;
mod state;
use state::{ComparisonResultState, ComparisonRowState};
mod comparators;
use comparators::*;
mod finality;
use finality::*;
mod trace;
pub use trace::{
    AnyComparisonRowTrace, ComparisonRowTrace, ComparisonRowTraceError, ComparisonRowTraceLineage,
};

/// Comparator family supported by the Rust implementation.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Comparator {
    /// Overlap rows where target and comparison are both active.
    Overlap,
    /// Residual target-only rows.
    Residual,
    /// Missing comparison-only rows.
    Missing,
    /// Coverage rows across target segments.
    Coverage,
    /// Gap rows for observed uncovered spans.
    Gap,
    /// Symmetric-difference rows for disagreement spans.
    SymmetricDifference,
    /// Containment rows for target segments relative to comparison coverage.
    Containment,
    /// Lead/lag measurements over target and comparison transitions.
    LeadLag {
        /// Transition point to compare.
        transition: LeadLagTransition,
        /// Temporal axis for the measurement.
        axis: TemporalAxis,
        /// Allowed delta magnitude.
        tolerance_magnitude: i64,
    },
    /// As-of point-in-time lookup.
    AsOf {
        /// Lookup direction.
        direction: AsOfDirection,
        /// Temporal axis for lookup.
        axis: TemporalAxis,
        /// Allowed match distance.
        tolerance_magnitude: i64,
    },
}

impl Comparator {
    /// Parses a comparator declaration.
    pub fn parse(value: &str) -> Option<Self> {
        Self::parse_result(value).ok()
    }

    /// Parses a comparator declaration with a reason for malformed input.
    pub fn parse_result(value: &str) -> Result<Self, ComparatorParseError> {
        match value {
            "overlap" => Ok(Self::Overlap),
            "residual" => Ok(Self::Residual),
            "missing" => Ok(Self::Missing),
            "coverage" => Ok(Self::Coverage),
            "gap" => Ok(Self::Gap),
            "symmetric-difference" => Ok(Self::SymmetricDifference),
            "containment" => Ok(Self::Containment),
            _ => parse_parameterized_comparator(value)
                .ok_or_else(|| ComparatorParseError(value.to_owned())),
        }
    }

    /// Returns the comparator declaration used in exports.
    #[must_use]
    pub fn declaration(&self) -> String {
        match self {
            Self::Overlap => "overlap".to_owned(),
            Self::Residual => "residual".to_owned(),
            Self::Missing => "missing".to_owned(),
            Self::Coverage => "coverage".to_owned(),
            Self::Gap => "gap".to_owned(),
            Self::SymmetricDifference => "symmetric-difference".to_owned(),
            Self::Containment => "containment".to_owned(),
            Self::LeadLag {
                transition,
                axis,
                tolerance_magnitude,
            } => {
                format!(
                    "lead-lag:{}:{}:{tolerance_magnitude}",
                    lead_lag_transition_name(transition),
                    temporal_axis_name(*axis)
                )
            }
            Self::AsOf {
                direction,
                axis,
                tolerance_magnitude,
            } => format!(
                "asof:{}:{}:{tolerance_magnitude}",
                as_of_direction_name(direction),
                temporal_axis_name(*axis)
            ),
        }
    }
}

/// Detailed comparator declaration parse error.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
#[error("unsupported comparator declaration '{0}'")]
pub struct ComparatorParseError(String);

fn temporal_axis_name(axis: TemporalAxis) -> &'static str {
    match axis {
        TemporalAxis::ProcessingPosition => "position",
        TemporalAxis::Timestamp => "timestamp",
    }
}

fn lead_lag_transition_name(transition: &LeadLagTransition) -> &'static str {
    match transition {
        LeadLagTransition::Start => "start",
        LeadLagTransition::End => "end",
    }
}

fn as_of_direction_name(direction: &AsOfDirection) -> &'static str {
    match direction {
        AsOfDirection::Previous => "previous",
        AsOfDirection::Next => "next",
        AsOfDirection::Nearest => "nearest",
    }
}

/// Diagnostic severity.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SegmentRef<'a> {
    start: crate::TemporalPoint,
    end: crate::TemporalPoint,
    record_id: &'a str,
    record_ids: Vec<String>,
    source: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AlignedSegment {
    start: i64,
    end: i64,
    axis: TemporalAxis,
    clock: Option<String>,
    target_record_ids: Vec<String>,
    against_record_ids: Vec<String>,
    against_is_active: bool,
    against_active_sources: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TransitionPoint<'a> {
    record_id: &'a str,
    point: crate::TemporalPoint,
}

type GroupKey = (String, String, Option<String>, TemporalAxis, Option<String>);
type GroupWindows<'a> = (Vec<SegmentRef<'a>>, Vec<SegmentRef<'a>>);

struct ResultArtifacts {
    comparator_summaries: Vec<ComparatorSummary>,
    coverage_summaries: Vec<CoverageSummary>,
    lead_lag_summaries: Vec<LeadLagSummary>,
    extension_metadata: Vec<ComparisonExtensionMetadata>,
    rows: ComparisonRows,
    state: ComparisonResultState,
}

/// Portable selected/excluded/normalized window artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowArtifact {
    /// Deterministic record ID.
    #[serde(rename = "recordId")]
    pub record_id: String,
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional source.
    pub source: Option<String>,
    /// Optional partition.
    pub partition: Option<String>,
    /// Start temporal point.
    pub start: crate::TemporalPoint,
    /// End temporal point when the source window is closed.
    pub end: Option<crate::TemporalPoint>,
    /// Known-at temporal point, when supplied.
    #[serde(rename = "knownAt")]
    pub known_at: Option<crate::TemporalPoint>,
    /// Whether the source window remained open.
    #[serde(rename = "isOpen")]
    pub is_open: bool,
    /// Segments.
    pub segments: Vec<WindowSegment>,
    /// Tags.
    pub tags: Vec<WindowTag>,
}

/// Excluded window artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExcludedWindowRecord {
    /// Deterministic record ID.
    #[serde(rename = "recordId")]
    pub record_id: String,
    /// Exclusion reason.
    pub reason: String,
    /// Diagnostic code, when any.
    #[serde(rename = "diagnosticCode")]
    pub diagnostic_code: Option<String>,
    /// Excluded window payload.
    pub window: WindowArtifact,
}

/// Normalized window artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NormalizedWindowRecord {
    /// Deterministic record ID.
    #[serde(rename = "recordId")]
    pub record_id: String,
    /// All source record IDs contributing to this normalized window.
    #[serde(rename = "recordIds")]
    pub record_ids: Vec<String>,
    /// Selector name.
    #[serde(rename = "selectorName")]
    pub selector_name: String,
    /// Comparison side.
    pub side: ComparisonSide,
    /// Normalized range.
    pub range: crate::TemporalRange,
    /// Whether the range depends on an open window clipped to a horizon.
    #[serde(rename = "isProvisional")]
    pub is_provisional: bool,
    /// Segments carried into alignment.
    pub segments: Vec<WindowSegment>,
    /// Backing window payload.
    pub window: WindowArtifact,
}

/// Prepared comparison artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PreparedComparison {
    /// Source plan for the prepared comparison.
    #[serde(skip)]
    pub(crate) plan: ComparisonPlan,
    /// Preparation diagnostics.
    pub(crate) diagnostics: Vec<ComparisonDiagnostic>,
    /// Selected windows.
    #[serde(rename = "selectedWindows")]
    pub(crate) selected_windows: Vec<WindowArtifact>,
    /// Excluded windows.
    #[serde(rename = "excludedWindows")]
    pub(crate) excluded_windows: Vec<ExcludedWindowRecord>,
    /// Normalized windows.
    #[serde(rename = "normalizedWindows")]
    pub(crate) normalized_windows: Vec<NormalizedWindowRecord>,
}

impl PreparedComparison {
    /// Returns preparation diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[ComparisonDiagnostic] {
        &self.diagnostics
    }

    /// Returns selected window artifacts.
    #[must_use]
    pub fn selected_windows(&self) -> &[WindowArtifact] {
        &self.selected_windows
    }

    /// Returns excluded window artifacts.
    #[must_use]
    pub fn excluded_windows(&self) -> &[ExcludedWindowRecord] {
        &self.excluded_windows
    }

    /// Returns normalized windows.
    #[must_use]
    pub fn normalized_windows(&self) -> &[NormalizedWindowRecord] {
        &self.normalized_windows
    }
}

/// Aligned segment artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AlignedSegmentArtifact {
    /// Deterministic segment identifier.
    #[serde(rename = "segmentId")]
    pub segment_id: String,
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Aligned range.
    pub range: RowRange,
    /// Target record IDs covering the range.
    #[serde(rename = "targetRecordIds")]
    pub target_record_ids: Vec<String>,
    /// Comparison record IDs covering the range.
    #[serde(rename = "againstRecordIds")]
    pub against_record_ids: Vec<String>,
    /// Whether the comparison side was active after selector evaluation.
    #[serde(rename = "againstIsActive")]
    pub against_is_active: bool,
    /// Sources active on the comparison side during the aligned segment.
    #[serde(rename = "againstActiveSources")]
    pub against_active_sources: Vec<String>,
}

/// Aligned comparison artifact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AlignedComparison {
    /// Deterministic aligned segments.
    pub segments: Vec<AlignedSegmentArtifact>,
}

/// Executes a comparison over closed windows.
#[must_use]
pub fn compare(history: &WindowHistory, plan: &ComparisonPlan) -> ComparisonResult {
    execute_compare(history, plan, None)
}

/// Executes a live comparison by clipping open windows to an evaluation horizon.
#[must_use]
pub fn compare_live(
    history: &WindowHistory,
    plan: &ComparisonPlan,
    evaluation_horizon: crate::TemporalPoint,
) -> ComparisonResult {
    execute_compare(history, plan, Some(evaluation_horizon))
}

/// Prepares a comparison without running comparators.
#[must_use]
pub fn prepare(history: &WindowHistory, plan: &ComparisonPlan) -> PreparedComparison {
    prepare_internal(history, plan, None)
}

/// Prepares a live comparison by clipping open windows to an evaluation horizon.
#[must_use]
pub fn prepare_live(
    history: &WindowHistory,
    plan: &ComparisonPlan,
    evaluation_horizon: crate::TemporalPoint,
) -> PreparedComparison {
    prepare_internal(history, plan, Some(evaluation_horizon))
}

/// Aligns prepared normalized windows into deterministic segments.
#[must_use]
pub fn align(prepared: &PreparedComparison) -> AlignedComparison {
    align_internal(prepared)
}

fn execute_compare(
    history: &WindowHistory,
    plan: &ComparisonPlan,
    live_horizon_override: Option<crate::TemporalPoint>,
) -> ComparisonResult {
    let structural_diagnostics = plan.validate();
    if structural_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return invalid_result(plan, structural_diagnostics);
    }

    let mut diagnostics = structural_diagnostics;
    let prepared = prepare_internal(history, plan, live_horizon_override.clone());
    diagnostics.extend(prepared.diagnostics.clone());
    diagnostics.extend(runtime_critic_diagnostics(
        plan,
        &prepared,
        live_horizon_override.clone(),
    ));
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        let rows = ComparisonRows::default();
        let state =
            ComparisonResultState::new(Some(prepared), None, ComparisonRowState::empty(&rows));
        let mut result = materialize_result(
            plan,
            &plan.name,
            false,
            diagnostics,
            ResultArtifacts {
                comparator_summaries: Vec::new(),
                coverage_summaries: Vec::new(),
                lead_lag_summaries: Vec::new(),
                extension_metadata: Vec::new(),
                rows,
                state,
            },
        );
        result.known_at = plan.known_at.as_ref().map(row_point_from_temporal_point);
        result.evaluation_horizon = live_horizon_override
            .as_ref()
            .or(plan.open_window_horizon.as_ref())
            .map(row_point_from_temporal_point);
        return result;
    }

    let groups = group_normalized_windows(&prepared);
    let aligned = align_grouped(&prepared, &groups);
    let mut rows = RowAccumulator::default();
    let mut comparator_summaries = Vec::new();
    let mut lead_lag_summaries = Vec::new();

    for comparator in &plan.comparators {
        let row_count = match comparator {
            Comparator::Overlap => {
                let emitted = build_overlap_rows(&aligned);
                let count = emitted.len();
                rows.overlap.extend(emitted);
                count
            }
            Comparator::Residual => {
                let emitted = build_residual_rows(&aligned);
                let count = emitted.len();
                rows.residual.extend(emitted);
                count
            }
            Comparator::Missing => {
                let emitted = build_missing_rows(&aligned);
                let count = emitted.len();
                rows.missing.extend(emitted);
                count
            }
            Comparator::Coverage => {
                let emitted = build_coverage_rows(&aligned);
                let count = emitted.len();
                rows.coverage.extend(emitted);
                count
            }
            Comparator::Gap => {
                let emitted = build_gap_rows(&aligned);
                let count = emitted.len();
                rows.gap.extend(emitted);
                count
            }
            Comparator::SymmetricDifference => {
                let emitted = build_symmetric_difference_rows(&aligned);
                let count = emitted.len();
                rows.symmetric_difference.extend(emitted);
                count
            }
            Comparator::Containment => {
                let emitted = build_containment_rows(&aligned, &prepared);
                let count = emitted.len();
                rows.containment.extend(emitted);
                count
            }
            Comparator::LeadLag {
                transition,
                axis,
                tolerance_magnitude,
            } => {
                let (emitted, summary) =
                    build_lead_lag_rows(&groups, transition.clone(), *axis, *tolerance_magnitude);
                let count = emitted.len();
                rows.lead_lag.extend(emitted);
                lead_lag_summaries.push(summary);
                count
            }
            Comparator::AsOf {
                direction,
                axis,
                tolerance_magnitude,
            } => {
                let (emitted, extra_diagnostics) =
                    build_as_of_rows(&groups, direction.clone(), *axis, *tolerance_magnitude);
                let count = emitted.len();
                rows.as_of.extend(emitted);
                diagnostics.extend(extra_diagnostics);
                count
            }
        };

        comparator_summaries.push(ComparatorSummary {
            comparator_name: comparator.declaration(),
            row_count,
        });
    }

    let provisional_record_ids = prepared
        .normalized_windows
        .iter()
        .filter(|window| window.is_provisional)
        .map(|window| window.record_id.clone())
        .collect::<BTreeSet<_>>();
    let gap_provisional_record_ids = prepared
        .normalized_windows
        .iter()
        .filter(|window| window.is_provisional)
        .flat_map(|window| window.record_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let rows = rows.into_shared();
    let coverage_summaries = build_coverage_summaries(&rows.coverage);
    let row_state = build_row_state(&rows, &provisional_record_ids, &gap_provisional_record_ids);
    let extension_metadata = build_extension_metadata(&aligned, plan);

    let mut result = materialize_result(
        plan,
        &plan.name,
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error),
        diagnostics,
        ResultArtifacts {
            comparator_summaries,
            coverage_summaries,
            lead_lag_summaries,
            extension_metadata,
            rows,
            state: ComparisonResultState::new(Some(prepared), Some(aligned), row_state),
        },
    );
    result.known_at = plan.known_at.as_ref().map(row_point_from_temporal_point);
    result.evaluation_horizon = live_horizon_override
        .as_ref()
        .or(plan.open_window_horizon.as_ref())
        .map(row_point_from_temporal_point);
    result
}

fn invalid_result(
    plan: &ComparisonPlan,
    diagnostics: Vec<ComparisonDiagnostic>,
) -> ComparisonResult {
    let rows = ComparisonRows::default();
    let state = ComparisonResultState::new(None, None, ComparisonRowState::empty(&rows));
    let mut result = materialize_result(
        plan,
        &plan.name,
        false,
        diagnostics,
        ResultArtifacts {
            comparator_summaries: Vec::new(),
            coverage_summaries: Vec::new(),
            lead_lag_summaries: Vec::new(),
            extension_metadata: Vec::new(),
            rows,
            state,
        },
    );
    result.known_at = plan.known_at.as_ref().map(row_point_from_temporal_point);
    result.evaluation_horizon = plan
        .open_window_horizon
        .as_ref()
        .map(row_point_from_temporal_point);
    result
}

fn parse_parameterized_comparator(value: &str) -> Option<Comparator> {
    let parts: Vec<&str> = value.split(':').collect();
    match parts.as_slice() {
        ["lead-lag", transition, axis, tolerance] => Some(Comparator::LeadLag {
            transition: parse_lead_lag_transition(transition)?,
            axis: parse_temporal_axis(axis)?,
            tolerance_magnitude: parse_non_negative_i64(tolerance)?,
        }),
        ["asof", direction, axis, tolerance] => Some(Comparator::AsOf {
            direction: parse_as_of_direction(direction)?,
            axis: parse_temporal_axis(axis)?,
            tolerance_magnitude: parse_non_negative_i64(tolerance)?,
        }),
        _ => None,
    }
}

fn parse_temporal_axis(value: &str) -> Option<TemporalAxis> {
    match value {
        "position" | "ProcessingPosition" => Some(TemporalAxis::ProcessingPosition),
        "timestamp" | "Timestamp" => Some(TemporalAxis::Timestamp),
        _ => None,
    }
}

fn parse_lead_lag_transition(value: &str) -> Option<LeadLagTransition> {
    match value {
        "start" | "Start" => Some(LeadLagTransition::Start),
        "end" | "End" => Some(LeadLagTransition::End),
        _ => None,
    }
}

fn parse_as_of_direction(value: &str) -> Option<AsOfDirection> {
    match value {
        "previous" | "Previous" => Some(AsOfDirection::Previous),
        "next" | "Next" => Some(AsOfDirection::Next),
        "nearest" | "Nearest" => Some(AsOfDirection::Nearest),
        _ => None,
    }
}

fn parse_non_negative_i64(value: &str) -> Option<i64> {
    let parsed = value.parse::<i64>().ok()?;
    (parsed >= 0).then_some(parsed)
}

fn materialize_result(
    plan: &ComparisonPlan,
    plan_name: &str,
    is_valid: bool,
    diagnostics: Vec<ComparisonDiagnostic>,
    artifacts: ResultArtifacts,
) -> ComparisonResult {
    let state = Arc::new(artifacts.state);
    let rows = artifacts.rows;
    let prepared = plan
        .output
        .include_explain_data
        .then(|| state.prepared())
        .flatten()
        .map(|prepared| serde_json::to_value(prepared).expect("prepared artifact"));
    let aligned = plan
        .output
        .include_aligned_segments
        .then(|| state.aligned())
        .flatten()
        .map(|aligned| serde_json::to_value(aligned).expect("aligned artifact"));
    let row_finalities = state.row_finalities();

    ComparisonResult {
        schema: "spanfold.comparison.result".to_owned(),
        schema_version: 0,
        artifact: "result".to_owned(),
        plan: plan.clone(),
        plan_name: plan_name.to_owned(),
        is_valid,
        diagnostics,
        prepared,
        aligned,
        known_at: None,
        evaluation_horizon: None,
        comparator_summaries: artifacts.comparator_summaries,
        coverage_summaries: artifacts.coverage_summaries,
        lead_lag_summaries: artifacts.lead_lag_summaries,
        row_finalities,
        extension_metadata: artifacts.extension_metadata,
        rows,
        state,
    }
}

fn build_coverage_summaries(rows: &[CoverageRow]) -> Vec<CoverageSummary> {
    let mut grouped: BTreeMap<(String, String, Option<String>), (i128, i128)> = BTreeMap::new();
    for row in rows {
        let entry = grouped
            .entry((
                row.window_name.clone(),
                row.key.clone(),
                row.partition.clone(),
            ))
            .or_insert((0, 0));
        entry.0 += i128::from(row.target_magnitude);
        entry.1 += i128::from(row.covered_magnitude);
    }

    grouped
        .into_iter()
        .map(
            |((window_name, key, partition), (target_magnitude, covered_magnitude))| {
                CoverageSummary {
                    window_name,
                    key,
                    partition,
                    target_magnitude: target_magnitude as f64,
                    target_magnitude_exact: target_magnitude,
                    covered_magnitude: covered_magnitude as f64,
                    covered_magnitude_exact: covered_magnitude,
                    coverage_ratio: if target_magnitude == 0 {
                        0.0
                    } else {
                        covered_magnitude as f64 / target_magnitude as f64
                    },
                }
            },
        )
        .collect()
}

fn build_extension_metadata(
    aligned: &AlignedComparison,
    plan: &ComparisonPlan,
) -> Vec<ComparisonExtensionMetadata> {
    let Some(AgainstSelection::Cohort {
        activity, sources, ..
    }) = plan.legacy_against()
    else {
        return Vec::new();
    };

    aligned
        .segments
        .iter()
        .enumerate()
        .map(|(index, segment)| ComparisonExtensionMetadata {
            extension_id: "spanfold.cohort".to_owned(),
            key: format!("segment[{index}]"),
            value: serde_json::json!({
                "rule": activity.name(),
                "required": required_activity_count(activity, sources.len()),
                "activeCount": segment.against_active_sources.len(),
                "isActive": segment.against_is_active,
                "activeSources": segment.against_active_sources,
            })
            .to_string(),
        })
        .collect()
}

fn required_activity_count(activity: &CohortActivity, member_count: usize) -> usize {
    match activity {
        CohortActivity::Any => 1,
        CohortActivity::All => member_count,
        CohortActivity::None => 0,
        CohortActivity::AtLeast { count }
        | CohortActivity::AtMost { count }
        | CohortActivity::Exactly { count } => *count,
    }
}

fn prepare_internal(
    history: &WindowHistory,
    plan: &ComparisonPlan,
    live_horizon_override: Option<crate::TemporalPoint>,
) -> PreparedComparison {
    let structural_diagnostics = plan.validate();
    if structural_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return PreparedComparison {
            plan: plan.clone(),
            diagnostics: structural_diagnostics,
            selected_windows: Vec::new(),
            excluded_windows: Vec::new(),
            normalized_windows: Vec::new(),
        };
    }

    let mut diagnostics = structural_diagnostics;
    let mut selected_windows = Vec::new();
    let mut excluded_windows = Vec::new();
    let mut normalized_windows = Vec::new();

    let scope = ComparisonScope {
        window_name: plan.scope_window.clone(),
        key: plan.scope_key.clone(),
        partition: plan.scope_partition.clone(),
        time_axis: plan.time_axis,
        segment_filters: plan.scope_segments.clone(),
        tag_filters: plan.scope_tags.clone(),
    };
    let normalization_request = WindowNormalizationRequest {
        scope: &scope,
        time_axis: plan.time_axis,
        known_at: plan.known_at.as_ref(),
        null_timestamp_policy: plan.null_timestamp_policy,
        require_closed: plan.require_closed_windows,
        open_window_policy: plan.open_window_policy,
        evaluation_horizon: live_horizon_override
            .as_ref()
            .or(plan.open_window_horizon.as_ref()),
    };
    let candidates = crate::window_normalization::ordered_candidates(history);

    let target_selector = plan.effective_target_selector();
    let target_selector_name = if plan.explicit_target_selector().is_some() {
        target_selector.name.as_str()
    } else {
        "target"
    };
    let against_selectors = plan.effective_against_selectors();
    let use_explicit_against_selector_names = !plan.explicit_against_selectors().is_empty();

    for candidate in candidates {
        let window = to_window_artifact(&candidate);
        let record = candidate.to_window_record();
        let normalization =
            crate::window_normalization::normalize_window(candidate, &normalization_request);
        if matches!(
            &normalization,
            Err(WindowNormalizationFailure::FutureWindowExcluded { .. })
        ) {
            push_normalization_exclusion(
                &candidate,
                normalization
                    .as_ref()
                    .expect_err("matched future exclusion"),
                &mut diagnostics,
                &mut excluded_windows,
            );
            continue;
        }
        if matches!(&normalization, Ok(None)) {
            push_scope_exclusion(&candidate, &mut excluded_windows);
            continue;
        }

        let is_target = target_selector.matches(&record);
        let matching_against_selectors = against_selectors
            .iter()
            .filter(|selector| selector.matches(&record))
            .collect::<Vec<_>>();
        if !is_target && matching_against_selectors.is_empty() {
            excluded_windows.push(ExcludedWindowRecord {
                record_id: window.record_id.clone(),
                reason: "Window did not match target or comparison selectors.".to_owned(),
                diagnostic_code: None,
                window,
            });
            continue;
        }

        selected_windows.push(window.clone());
        if is_target
            && let Some(normalized) = normalize_for_side(
                &candidate,
                target_selector_name,
                ComparisonSide::Target,
                &normalization,
                &mut diagnostics,
                &mut excluded_windows,
            )
        {
            normalized_windows.push(normalized);
        }
        for selector in matching_against_selectors {
            let selector_name = if use_explicit_against_selector_names {
                selector.name.as_str()
            } else {
                "against"
            };
            if let Some(normalized) = normalize_for_side(
                &candidate,
                selector_name,
                ComparisonSide::Against,
                &normalization,
                &mut diagnostics,
                &mut excluded_windows,
            ) {
                normalized_windows.push(normalized);
            }
        }
    }

    let normalized_windows =
        postprocess_normalized_windows(normalized_windows, plan, &mut diagnostics);
    let target_ids = normalized_windows
        .iter()
        .filter(|window| window.side == ComparisonSide::Target)
        .flat_map(|window| window.record_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    if normalized_windows.iter().any(|window| {
        window.side == ComparisonSide::Against
            && window
                .record_ids
                .iter()
                .any(|record_id| target_ids.contains(record_id))
    }) {
        push_diagnostic_once(
            &mut diagnostics,
            "SelfComparison",
            DiagnosticSeverity::Error,
        );
    }

    PreparedComparison {
        plan: plan.clone(),
        diagnostics,
        selected_windows,
        excluded_windows,
        normalized_windows,
    }
}

fn postprocess_normalized_windows(
    windows: Vec<NormalizedWindowRecord>,
    plan: &ComparisonPlan,
    diagnostics: &mut Vec<ComparisonDiagnostic>,
) -> Vec<NormalizedWindowRecord> {
    let deduplicated = deduplicate_normalized_windows(windows, plan, diagnostics);
    if plan.coalesce_adjacent_windows {
        coalesce_normalized_windows(deduplicated)
    } else {
        deduplicated
    }
}

fn deduplicate_normalized_windows(
    windows: Vec<NormalizedWindowRecord>,
    plan: &ComparisonPlan,
    diagnostics: &mut Vec<ComparisonDiagnostic>,
) -> Vec<NormalizedWindowRecord> {
    let mut seen = BTreeSet::<Vec<u8>>::new();
    let mut rows = Vec::new();
    for window in windows {
        let key = normalized_duplicate_key(&window);
        if seen.contains(&key) {
            push_diagnostic_once(diagnostics, "DuplicateWindow", DiagnosticSeverity::Warning);
            if plan.duplicate_window_policy == ComparisonDuplicateWindowPolicy::Reject {
                continue;
            }
        } else {
            seen.insert(key);
        }
        rows.push(window);
    }
    rows
}

fn coalesce_normalized_windows(
    windows: Vec<NormalizedWindowRecord>,
) -> Vec<NormalizedWindowRecord> {
    let mut groups: BTreeMap<Vec<u8>, Vec<NormalizedWindowRecord>> = BTreeMap::new();
    for window in windows {
        groups
            .entry(normalized_coalesce_key(&window))
            .or_default()
            .push(window);
    }

    let mut rows = Vec::new();
    for mut windows in groups.into_values() {
        windows.sort_by(|left, right| {
            left.range
                .start()
                .try_cmp(&right.range.start())
                .expect("coalescing groups share a temporal domain")
                .then_with(|| {
                    left.range
                        .end()
                        .try_cmp(&right.range.end())
                        .expect("coalescing groups share a temporal domain")
                })
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
        for window in windows {
            if let Some(previous) = rows.last_mut()
                && can_coalesce_normalized(previous, &window)
            {
                previous.range =
                    crate::TemporalRange::new(previous.range.start(), window.range.end())
                        .expect("adjacent normalized ranges share an axis");
                previous.is_provisional |= window.is_provisional;
                previous.record_ids.extend(window.record_ids);
                continue;
            }
            rows.push(window);
        }
    }
    rows
}

fn can_coalesce_normalized(
    first: &NormalizedWindowRecord,
    second: &NormalizedWindowRecord,
) -> bool {
    normalized_coalesce_key(first) == normalized_coalesce_key(second)
        && first.range.end() == second.range.start()
}

fn normalized_duplicate_key(window: &NormalizedWindowRecord) -> Vec<u8> {
    serde_json::to_vec(&(
        &window.selector_name,
        &window.side,
        &window.window.window_name,
        &window.window.key,
        &window.window.source,
        &window.window.partition,
        &window.range,
        &window.segments,
    ))
    .unwrap_or_default()
}

fn normalized_coalesce_key(window: &NormalizedWindowRecord) -> Vec<u8> {
    serde_json::to_vec(&(
        &window.selector_name,
        &window.side,
        &window.window.window_name,
        &window.window.key,
        &window.window.source,
        &window.window.partition,
        &window.segments,
        &window.window.tags,
    ))
    .unwrap_or_default()
}

fn align_internal(prepared: &PreparedComparison) -> AlignedComparison {
    let groups = group_normalized_windows(prepared);
    align_grouped(prepared, &groups)
}

fn align_grouped(
    prepared: &PreparedComparison,
    groups: &BTreeMap<GroupKey, GroupWindows<'_>>,
) -> AlignedComparison {
    let mut segments = Vec::new();
    for ((window_name, key, partition, axis, clock), (targets, againsts)) in groups {
        let against = prepared.plan.against_for_alignment();
        for segment in aligned_segments(targets.as_slice(), againsts.as_slice(), &against) {
            segments.push(AlignedSegmentArtifact {
                segment_id: format!("segment[{}]", segments.len()),
                window_name: window_name.clone(),
                key: key.clone(),
                partition: partition.clone(),
                range: RowRange {
                    start: segment.start,
                    end: segment.end,
                    axis: *axis,
                    clock: clock.clone(),
                },
                target_record_ids: segment.target_record_ids,
                against_record_ids: segment.against_record_ids,
                against_is_active: segment.against_is_active,
                against_active_sources: segment.against_active_sources,
            });
        }
    }
    AlignedComparison { segments }
}

fn group_normalized_windows(prepared: &PreparedComparison) -> BTreeMap<GroupKey, GroupWindows<'_>> {
    let mut groups: BTreeMap<GroupKey, GroupWindows<'_>> = BTreeMap::new();
    for normalized in &prepared.normalized_windows {
        let group = groups
            .entry((
                normalized.window.window_name.clone(),
                normalized.window.key.clone(),
                normalized.window.partition.clone(),
                normalized.range.start().axis(),
                normalized.range.start().clock().map(str::to_owned),
            ))
            .or_default();
        let segment = SegmentRef {
            start: normalized.range.start(),
            end: normalized.range.end(),
            record_id: normalized.record_id.as_str(),
            record_ids: normalized.record_ids.clone(),
            source: normalized.window.source.as_deref(),
        };
        match normalized.side {
            ComparisonSide::Target => group.0.push(segment),
            ComparisonSide::Against => group.1.push(segment),
        }
    }
    groups
}

fn row_point_from_temporal_point(point: &crate::TemporalPoint) -> RowPoint {
    RowPoint {
        axis: point.axis(),
        magnitude: point.magnitude(),
        clock: point.clock().map(str::to_owned),
    }
}

fn to_window_artifact(candidate: &RawWindowRef<'_>) -> WindowArtifact {
    WindowArtifact {
        record_id: candidate.record_id().to_owned(),
        window_name: candidate.window_name().to_owned(),
        key: candidate.key().to_owned(),
        source: candidate.source().map(str::to_owned),
        partition: candidate.partition().map(str::to_owned),
        start: candidate.start_point(),
        end: candidate.end_point(),
        known_at: candidate.known_at_point(),
        is_open: candidate.is_open(),
        segments: candidate.segments().to_vec(),
        tags: candidate.tags().to_vec(),
    }
}

fn push_scope_exclusion(
    candidate: &RawWindowRef<'_>,
    excluded_windows: &mut Vec<ExcludedWindowRecord>,
) {
    let window = to_window_artifact(candidate);
    excluded_windows.push(ExcludedWindowRecord {
        record_id: window.record_id.clone(),
        reason: "Window is outside the comparison scope.".to_owned(),
        diagnostic_code: None,
        window,
    });
}

fn push_normalization_exclusion(
    candidate: &RawWindowRef<'_>,
    failure: &WindowNormalizationFailure,
    diagnostics: &mut Vec<ComparisonDiagnostic>,
    excluded_windows: &mut Vec<ExcludedWindowRecord>,
) {
    let (reason, code, severity) = match failure {
        WindowNormalizationFailure::FutureWindowExcluded { .. } => (
            "Window was not available at the configured known-at point.".to_owned(),
            "FutureWindowExcluded",
            DiagnosticSeverity::Warning,
        ),
        WindowNormalizationFailure::MissingTimestamp { policy, .. } => (
            "Window temporal axis does not match the comparison plan.".to_owned(),
            "MissingEventTime",
            match policy {
                ComparisonNullTimestampPolicy::Reject => DiagnosticSeverity::Error,
                ComparisonNullTimestampPolicy::Exclude => DiagnosticSeverity::Warning,
            },
        ),
        WindowNormalizationFailure::TemporalAxisMismatch { .. } => (
            "Window temporal axis does not match the comparison plan.".to_owned(),
            "TemporalAxisMismatch",
            DiagnosticSeverity::Error,
        ),
        WindowNormalizationFailure::OpenWindowWithoutPolicy => (
            "Open windows require an explicit clipping policy.".to_owned(),
            "OpenWindowsWithoutPolicy",
            DiagnosticSeverity::Error,
        ),
        WindowNormalizationFailure::InvalidRangeDuration { .. } => (
            "Open-window horizon cannot be earlier than the window start.".to_owned(),
            "InvalidRangeDuration",
            DiagnosticSeverity::Error,
        ),
        WindowNormalizationFailure::InvalidTemporalRange { error } => (
            error.to_string(),
            "InvalidTemporalRange",
            DiagnosticSeverity::Error,
        ),
    };
    let window = to_window_artifact(candidate);
    excluded_windows.push(ExcludedWindowRecord {
        record_id: window.record_id.clone(),
        reason,
        diagnostic_code: Some(code.to_owned()),
        window,
    });
    diagnostics.push(ComparisonDiagnostic {
        code: code.to_owned(),
        severity,
    });
}

fn normalize_for_side(
    candidate: &RawWindowRef<'_>,
    selector_name: &str,
    side: ComparisonSide,
    normalization: &Result<Option<NormalizedWindowEvidence<'_>>, WindowNormalizationFailure>,
    diagnostics: &mut Vec<ComparisonDiagnostic>,
    excluded_windows: &mut Vec<ExcludedWindowRecord>,
) -> Option<NormalizedWindowRecord> {
    match normalization {
        Ok(Some(evidence)) => Some(NormalizedWindowRecord {
            record_id: evidence.candidate.record_id().to_owned(),
            record_ids: vec![evidence.candidate.record_id().to_owned()],
            selector_name: selector_name.to_owned(),
            side,
            range: evidence.range.clone(),
            is_provisional: evidence.is_provisional,
            segments: evidence.candidate.segments().to_vec(),
            window: to_window_artifact(&evidence.candidate),
        }),
        Ok(None) => None,
        Err(failure) => {
            push_normalization_exclusion(candidate, failure, diagnostics, excluded_windows);
            None
        }
    }
}

fn aligned_segments(
    targets: &[SegmentRef<'_>],
    againsts: &[SegmentRef<'_>],
    against_selection: &AgainstSelection,
) -> Vec<AlignedSegment> {
    let mut points = Vec::with_capacity((targets.len() + againsts.len()) * 2);
    for item in targets {
        points.push(item.start.clone());
        points.push(item.end.clone());
    }
    for item in againsts {
        points.push(item.start.clone());
        points.push(item.end.clone());
    }

    points.sort_by(|left, right| {
        left.try_cmp(right)
            .expect("alignment groups share a temporal domain")
    });
    points.dedup();

    let mut target_starts = targets
        .iter()
        .enumerate()
        .map(|(index, item)| (item.start.clone(), index))
        .collect::<Vec<_>>();
    let mut target_ends = targets
        .iter()
        .enumerate()
        .map(|(index, item)| (item.end.clone(), index))
        .collect::<Vec<_>>();
    let mut against_starts = againsts
        .iter()
        .enumerate()
        .map(|(index, item)| (item.start.clone(), index))
        .collect::<Vec<_>>();
    let mut against_ends = againsts
        .iter()
        .enumerate()
        .map(|(index, item)| (item.end.clone(), index))
        .collect::<Vec<_>>();
    for events in [
        &mut target_starts,
        &mut target_ends,
        &mut against_starts,
        &mut against_ends,
    ] {
        events.sort_by(|left, right| {
            left.0
                .try_cmp(&right.0)
                .expect("alignment groups share a temporal domain")
        });
    }
    let mut active_targets = BTreeSet::new();
    let mut active_againsts = BTreeSet::new();
    let mut active_against_source_counts = BTreeMap::new();
    let mut target_start_index = 0;
    let mut target_end_index = 0;
    let mut against_start_index = 0;
    let mut against_end_index = 0;
    let mut segments = Vec::new();
    for pair in points.windows(2) {
        let start = pair[0].clone();
        let end = pair[1].clone();
        if !matches!(start.try_cmp(&end), Ok(Ordering::Less)) {
            continue;
        }

        while target_end_index < target_ends.len()
            && matches!(
                target_ends[target_end_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            active_targets.remove(&target_ends[target_end_index].1);
            target_end_index += 1;
        }
        while against_end_index < against_ends.len()
            && matches!(
                against_ends[against_end_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            let index = against_ends[against_end_index].1;
            active_againsts.remove(&index);
            if let Some(source) = againsts[index].source
                && let Some(count) = active_against_source_counts.get_mut(source)
            {
                *count -= 1;
                if *count == 0 {
                    active_against_source_counts.remove(source);
                }
            }
            against_end_index += 1;
        }
        while target_start_index < target_starts.len()
            && matches!(
                target_starts[target_start_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            active_targets.insert(target_starts[target_start_index].1);
            target_start_index += 1;
        }
        while against_start_index < against_starts.len()
            && matches!(
                against_starts[against_start_index].0.try_cmp(&start),
                Ok(Ordering::Less | Ordering::Equal)
            )
        {
            let index = against_starts[against_start_index].1;
            active_againsts.insert(index);
            if let Some(source) = againsts[index].source {
                *active_against_source_counts.entry(source).or_insert(0) += 1;
            }
            against_start_index += 1;
        }

        let mut target_record_ids = Vec::new();
        let mut against_record_ids = Vec::new();
        for index in &active_targets {
            target_record_ids.extend(targets[*index].record_ids.iter().cloned());
        }
        for index in &active_againsts {
            against_record_ids.extend(againsts[*index].record_ids.iter().cloned());
        }

        let active_sources = active_against_source_counts
            .keys()
            .map(|source| (*source).to_owned())
            .collect::<Vec<_>>();

        let against_is_active = match against_selection {
            AgainstSelection::Sources(_) => !active_sources.is_empty(),
            AgainstSelection::Cohort {
                sources, activity, ..
            } => activity.is_active(active_sources.len(), sources.len()),
        };

        segments.push(AlignedSegment {
            start: start.magnitude(),
            end: end.magnitude(),
            axis: start.axis(),
            clock: start.clock().map(str::to_owned),
            target_record_ids,
            against_record_ids,
            against_is_active,
            against_active_sources: active_sources,
        });
    }
    segments
}

#[cfg(test)]
mod tests {
    #![allow(unused_must_use)]

    use crate::{
        ClosedWindow, TemporalRange, WindowHistoryFixture, WindowRecordId, fixture::ContractFixture,
    };

    use super::*;

    #[test]
    fn selectors_match_windows_and_compose_predicates() {
        let window = crate::WindowRecord::Closed(crate::ClosedWindow {
            id: crate::WindowRecordId::new("record-1").expect("record id"),
            window_name: "DeviceOffline".to_owned(),
            key: "device-1".to_owned(),
            range: crate::TemporalRange::positions(10, 20).expect("range"),
            known_at: None,
            source: Some("provider-a".to_owned()),
            partition: Some("fleet-a".to_owned()),
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        });

        assert!(ComparisonSelector::for_source("provider-a").matches(&window));
        assert!(!ComparisonSelector::for_source("provider-b").matches(&window));
        assert!(ComparisonSelector::for_window_name("DeviceOffline").matches(&window));
        assert!(ComparisonSelector::for_key("device-1").matches(&window));
        assert!(ComparisonSelector::for_partition("fleet-a").matches(&window));
        assert!(
            ComparisonSelector::for_window_name("DeviceOffline")
                .and(ComparisonSelector::for_source("provider-a"))
                .matches(&window)
        );
        assert!(
            ComparisonSelector::for_source("provider-b")
                .or(ComparisonSelector::for_source("provider-a"))
                .matches(&window)
        );
    }

    #[test]
    fn runtime_selector_is_not_serializable_and_uses_predicate() {
        let window = crate::WindowRecord::Closed(crate::ClosedWindow {
            id: crate::WindowRecordId::new("record-1").expect("record id"),
            window_name: "DeviceOffline".to_owned(),
            key: "device-1".to_owned(),
            range: crate::TemporalRange::positions(0, 12).expect("range"),
            known_at: None,
            source: None,
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        });
        let selector = ComparisonSelector::runtime_only(
            "long-window",
            "window duration is longer than ten positions",
            |record| {
                record
                    .end()
                    .is_some_and(|end| end.magnitude() - record.start().magnitude() > 10)
            },
        );

        assert!(selector.matches(&window));
        assert!(!selector.is_serializable);
    }

    #[test]
    fn plan_validate_reports_structural_and_selector_diagnostics() {
        let history = WindowHistoryFixture::new().build();
        let complete = history
            .compare("Provider QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .overlap()
            .plan()
            .clone();

        assert!(complete.validate().is_empty());

        let missing = history.compare(" ").plan().clone();
        let missing_codes = missing
            .validate()
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();

        assert!(missing_codes.iter().any(|code| code == "MissingName"));
        assert!(missing_codes.iter().any(|code| code == "MissingTarget"));
        assert!(missing_codes.iter().any(|code| code == "MissingAgainst"));
        assert!(missing_codes.iter().any(|code| code == "MissingComparator"));

        let runtime_only = history
            .compare("Runtime selector QA")
            .target_selector(ComparisonSelector::runtime_only(
                "provider-a",
                "runtime provider selector",
                |_| true,
            ))
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .overlap()
            .strict()
            .plan()
            .clone();

        assert!(runtime_only.validate().is_empty());
        assert!(!runtime_only.is_serializable());
    }

    #[test]
    fn range_selectors_use_half_open_start_ranges() {
        let position_window = crate::WindowRecord::Closed(crate::ClosedWindow {
            id: crate::WindowRecordId::new("record-1").expect("record id"),
            window_name: "DeviceOffline".to_owned(),
            key: "device-1".to_owned(),
            range: crate::TemporalRange::positions(10, 20).expect("range"),
            known_at: None,
            source: None,
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        });
        let time_window = crate::WindowRecord::Closed(crate::ClosedWindow {
            id: crate::WindowRecordId::new("record-2").expect("record id"),
            window_name: "DeviceOffline".to_owned(),
            key: "device-1".to_owned(),
            range: crate::TemporalRange::new(
                crate::TemporalPoint::timestamp_ticks(15),
                crate::TemporalPoint::timestamp_ticks(20),
            )
            .expect("range"),
            known_at: None,
            source: None,
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        });

        assert!(
            ComparisonSelector::for_position_range(10, Some(20))
                .expect("selector")
                .matches(&position_window)
        );
        assert!(
            !ComparisonSelector::for_position_range(11, Some(20))
                .expect("selector")
                .matches(&position_window)
        );
        assert!(
            ComparisonSelector::for_time_range(10, Some(20))
                .expect("selector")
                .matches(&time_window)
        );
        assert!(matches!(
            ComparisonSelector::for_position_range(20, Some(10)),
            Err(ComparisonSelectorError::RangeEndBeforeStart)
        ));
    }

    #[test]
    fn basic_overlap_fixture_matches_expected_counts() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/basic-overlap.json"
        ))
        .expect("fixture should parse");
        let result = compare(fixture.history(), fixture.plan());

        assert!(result.is_valid);
        assert_eq!(result.comparator_summaries[0].row_count, 1);
        assert_eq!(result.comparator_summaries[1].row_count, 1);
        assert_eq!(result.comparator_summaries[2].row_count, 2);
        assert_eq!(result.overlap_rows()[0].range.start, 3);
        assert_eq!(result.overlap_rows()[0].range.end, 5);
        assert_eq!(result.residual_rows()[0].range.start, 1);
        assert_eq!(result.residual_rows()[0].range.end, 3);
    }

    #[test]
    fn canonical_rows_preserve_accessors_and_direct_serialization() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/basic-overlap.json"
        ))
        .expect("fixture should parse");
        let result = compare(fixture.history(), fixture.plan());

        assert_eq!(result.state.row_finalities(), result.row_finalities);
        assert_eq!(
            serde_json::to_value(result.state.prepared().expect("typed prepared"))
                .expect("serialize typed prepared"),
            *result
                .prepared
                .as_ref()
                .expect("prepared compatibility value")
        );
        assert_eq!(
            serde_json::to_value(result.state.aligned().expect("typed aligned"))
                .expect("serialize typed aligned"),
            *result
                .aligned
                .as_ref()
                .expect("aligned compatibility value")
        );

        let serialized = serde_json::to_value(&result).expect("serialize result directly");
        let object = serialized.as_object().expect("result object");
        assert!(object.contains_key("prepared"));
        assert!(object.contains_key("aligned"));
        assert!(object.contains_key("rows"));
        assert!(object.contains_key("rowFinalities"));
        assert!(!object.contains_key("state"));
        assert!(!object.contains_key("plan"));
        assert!(!object.contains_key("overlap_rows"));
        assert!(!object.contains_key("residual_rows"));
        assert!(!object.contains_key("missing_rows"));
        assert!(!object.contains_key("coverage_rows"));
        assert!(!object.contains_key("gap_rows"));
        assert!(!object.contains_key("symmetric_difference_rows"));
        assert!(!object.contains_key("containment_rows"));
        assert!(!object.contains_key("lead_lag_rows"));
        assert!(!object.contains_key("as_of_rows"));

        assert_eq!(result.overlap_rows(), result.rows.overlap.as_slice());
        assert_eq!(result.residual_rows(), result.rows.residual.as_slice());
        assert_eq!(result.missing_rows(), result.rows.missing.as_slice());
        assert_eq!(result.coverage_rows(), result.rows.coverage.as_slice());
        assert_eq!(result.gap_rows(), result.rows.gap.as_slice());
        assert_eq!(
            result.symmetric_difference_rows(),
            result.rows.symmetric_difference.as_slice()
        );
        assert_eq!(
            result.containment_rows(),
            result.rows.containment.as_slice()
        );
        assert_eq!(result.lead_lag_rows(), result.rows.lead_lag.as_slice());
        assert_eq!(result.as_of_rows(), result.rows.as_of.as_slice());
    }

    #[test]
    fn processing_window_artifacts_export_typed_axis_neutral_points() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |window| {
                window.source("provider-a").known_at_position(2)
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 7, |window| {
                window.source("provider-b")
            })
            .expect("against")
            .build();
        let plan = ComparisonPlan::new(
            "Typed processing artifacts",
            "provider-a",
            AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            vec![Comparator::Overlap],
        )
        .with_scope_window(Some("DeviceOffline".to_owned()));

        let result = compare(&history, &plan);
        let artifacts = &result.state.prepared().expect("prepared").selected_windows;
        assert_eq!(artifacts.len(), 2);
        let target = artifacts
            .iter()
            .find(|artifact| artifact.source.as_deref() == Some("provider-a"))
            .expect("target artifact");
        assert_eq!(target.start, TemporalPoint::position(1));
        assert_eq!(target.end, Some(TemporalPoint::position(5)));
        assert_eq!(target.known_at, Some(TemporalPoint::position(2)));

        let json = serde_json::to_value(target).expect("artifact json");
        let object = json.as_object().expect("artifact object");
        assert!(object.contains_key("start"));
        assert!(object.contains_key("end"));
        assert!(object.contains_key("knownAt"));
        assert!(!object.contains_key("startPosition"));
        assert!(!object.contains_key("endPosition"));
        assert!(!object.contains_key("knownAtPosition"));
        assert_eq!(object["start"]["axis"], "ProcessingPosition");
        assert_eq!(object["start"]["magnitude"], 1);
        assert_eq!(object["knownAt"]["magnitude"], 2);
    }

    #[test]
    fn timestamp_window_artifacts_retain_axis_and_clock_identity() {
        let clock = "event-clock";
        let point = |magnitude| TemporalPoint::timestamp_ticks_with_clock(magnitude, clock);
        let window = |id: &str, source: &str, start: i64, end: i64| ClosedWindow {
            id: WindowRecordId::new(id).expect("record id"),
            window_name: "DeviceOffline".to_owned(),
            key: "device-1".to_owned(),
            range: TemporalRange::new(point(start), point(end)).expect("range"),
            known_at: Some(point(100)),
            source: Some(source.to_owned()),
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        };
        let history = WindowHistory::from_records(
            [
                window("target", "provider-a", 10, 20),
                window("against", "provider-b", 15, 25),
            ],
            [],
        )
        .expect("history");
        let mut plan = ComparisonPlan::new(
            "Typed timestamp artifacts",
            "provider-a",
            AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            vec![Comparator::Overlap],
        )
        .with_scope_window(Some("DeviceOffline".to_owned()));
        plan.time_axis = TemporalAxis::Timestamp;

        let result = compare(&history, &plan);
        let artifacts = &result.state.prepared().expect("prepared").selected_windows;
        assert_eq!(artifacts.len(), 2);
        for artifact in artifacts {
            assert_eq!(artifact.start.axis(), TemporalAxis::Timestamp);
            assert_eq!(artifact.start.clock(), Some(clock));
            assert_eq!(
                artifact.end.as_ref().map(TemporalPoint::axis),
                Some(TemporalAxis::Timestamp)
            );
            assert_eq!(
                artifact.end.as_ref().and_then(TemporalPoint::clock),
                Some(clock)
            );
            assert_eq!(
                artifact.known_at.as_ref().map(TemporalPoint::axis),
                Some(TemporalAxis::Timestamp)
            );
            assert_eq!(
                artifact.known_at.as_ref().and_then(TemporalPoint::clock),
                Some(clock)
            );

            let json = serde_json::to_value(artifact).expect("artifact json");
            let object = json.as_object().expect("artifact object");
            assert!(!object.contains_key("startPosition"));
            assert!(!object.contains_key("endPosition"));
            assert!(!object.contains_key("knownAtPosition"));
            assert_eq!(object["start"]["axis"], "Timestamp");
            assert_eq!(object["start"]["clock"], clock);
            assert_eq!(object["end"]["clock"], clock);
            assert_eq!(object["knownAt"]["clock"], clock);
        }
    }

    #[test]
    fn coverage_rows_are_segments_and_summary_is_the_grouped_ratio() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/basic-overlap.json"
        ))
        .expect("fixture should parse");
        let result = compare(fixture.history(), fixture.plan());

        assert_eq!(result.rows.coverage.len(), 2);
        assert_eq!(result.rows.coverage[0].target_magnitude, 2);
        assert_eq!(result.rows.coverage[0].covered_magnitude, 0);
        assert_eq!(result.rows.coverage[1].target_magnitude, 2);
        assert_eq!(result.rows.coverage[1].covered_magnitude, 2);

        assert_eq!(result.coverage_summaries.len(), 1);
        let summary = &result.coverage_summaries[0];
        assert_eq!(summary.target_magnitude_exact, 4);
        assert_eq!(summary.covered_magnitude_exact, 2);
        assert_eq!(summary.coverage_ratio, 0.5);
    }

    #[test]
    fn gap_and_symmetric_difference_match_expected_rows() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 3, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 5, 7, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();
        let plan = ComparisonPlan {
            name: "Provider QA".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Gap, Comparator::SymmetricDifference],
            require_closed_windows: false,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert_eq!(result.gap_rows().len(), 1);
        assert_eq!(result.gap_rows()[0].range.start, 3);
        assert_eq!(result.gap_rows()[0].range.end, 5);
        assert_eq!(result.symmetric_difference_rows().len(), 2);
        assert_eq!(
            result.symmetric_difference_rows()[0].side,
            ComparisonSide::Target
        );
        assert_eq!(
            result.symmetric_difference_rows()[1].side,
            ComparisonSide::Against
        );
    }

    #[test]
    fn containment_emits_left_contained_and_right_rows() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 7, |w| w.source("target"))
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 5, |w| w.source("container"))
            .expect("container")
            .build();
        let plan = ComparisonPlan {
            name: "Containment".to_owned(),
            selection: ComparisonSelection::legacy(
                "target",
                AgainstSelection::Sources(vec!["container".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Containment],
            require_closed_windows: false,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);
        assert_eq!(result.containment_rows().len(), 3);
        assert_eq!(
            result.containment_rows()[0].status,
            ContainmentStatus::LeftOverhang
        );
        assert_eq!(
            result.containment_rows()[1].status,
            ContainmentStatus::Contained
        );
        assert_eq!(
            result.containment_rows()[2].status,
            ContainmentStatus::RightOverhang
        );
    }

    #[test]
    fn lead_lag_and_as_of_emit_expected_rows() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 4, |w| w.source("target"))
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 6, |w| {
                w.source("comparison")
            })
            .expect("comparison")
            .closed_window("Quote", "selection-1", 10, 11, |w| w.source("trade"))
            .expect("trade")
            .closed_window("Quote", "selection-1", 7, 20, |w| w.source("quote"))
            .expect("quote")
            .build();

        let lead_lag = compare(
            &history,
            &ComparisonPlan {
                name: "Latency QA".to_owned(),
                selection: ComparisonSelection::legacy(
                    "target",
                    AgainstSelection::Sources(vec!["comparison".to_owned()]),
                ),
                scope_window: Some("DeviceOffline".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::LeadLag {
                    transition: LeadLagTransition::Start,
                    axis: TemporalAxis::ProcessingPosition,
                    tolerance_magnitude: 5,
                }],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::ProcessingPosition,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );
        assert_eq!(lead_lag.lead_lag_rows().len(), 1);
        assert_eq!(
            lead_lag.lead_lag_rows()[0].direction,
            LeadLagDirection::TargetLeads
        );
        assert_eq!(lead_lag.lead_lag_rows()[0].delta_magnitude, Some(-2));
        assert_eq!(lead_lag.lead_lag_summaries[0].target_lead_count, 1);

        let as_of = compare(
            &history,
            &ComparisonPlan {
                name: "Quote at trade".to_owned(),
                selection: ComparisonSelection::legacy(
                    "trade",
                    AgainstSelection::Sources(vec!["quote".to_owned()]),
                ),
                scope_window: Some("Quote".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::AsOf {
                    direction: AsOfDirection::Previous,
                    axis: TemporalAxis::ProcessingPosition,
                    tolerance_magnitude: 5,
                }],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::ProcessingPosition,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );
        assert_eq!(as_of.as_of_rows().len(), 1);
        assert_eq!(as_of.as_of_rows()[0].status, AsOfMatchStatus::Matched);
        assert_eq!(as_of.as_of_rows()[0].distance_magnitude, Some(3));
    }

    #[test]
    fn as_of_previous_matches_exact_candidate() {
        let history = WindowHistoryFixture::new()
            .closed_window("Quote", "selection-1", 10, 11, |window| {
                window.source("trade")
            })
            .expect("trade")
            .closed_window("Quote", "selection-1", 10, 12, |window| {
                window.source("quote")
            })
            .expect("quote")
            .build();
        let plan = ComparisonPlan::new(
            "Exact quote at trade",
            "trade",
            AgainstSelection::Sources(vec!["quote".to_owned()]),
            vec![Comparator::AsOf {
                direction: AsOfDirection::Previous,
                axis: TemporalAxis::ProcessingPosition,
                tolerance_magnitude: 0,
            }],
        )
        .with_scope_window(Some("Quote".to_owned()));

        let result = compare(&history, &plan);

        assert_eq!(result.as_of_rows().len(), 1);
        assert_eq!(result.as_of_rows()[0].status, AsOfMatchStatus::Exact);
        assert_eq!(result.as_of_rows()[0].distance_magnitude, Some(0));
        assert!(result.as_of_rows()[0].matched_record_id.is_some());
    }

    #[test]
    fn lead_lag_and_as_of_saturate_extreme_temporal_distances() {
        let history = WindowHistoryFixture::new()
            .closed_window(
                "DeviceOffline",
                "device-1",
                i64::MAX - 1,
                i64::MAX,
                |window| window.source("target"),
            )
            .expect("target")
            .closed_window(
                "DeviceOffline",
                "device-1",
                i64::MIN,
                i64::MIN + 1,
                |window| window.source("comparison"),
            )
            .expect("comparison")
            .build();
        let plan = ComparisonPlan::new(
            "Extreme temporal distance",
            "target",
            AgainstSelection::Sources(vec!["comparison".to_owned()]),
            vec![
                Comparator::LeadLag {
                    transition: LeadLagTransition::Start,
                    axis: TemporalAxis::ProcessingPosition,
                    tolerance_magnitude: i64::MAX,
                },
                Comparator::AsOf {
                    direction: AsOfDirection::Nearest,
                    axis: TemporalAxis::ProcessingPosition,
                    tolerance_magnitude: i64::MAX,
                },
            ],
        )
        .with_scope_window(Some("DeviceOffline".to_owned()));

        let result = compare(&history, &plan);

        assert!(result.is_valid);
        assert_eq!(result.lead_lag_rows()[0].delta_magnitude, Some(i64::MAX));
        assert!(result.lead_lag_rows()[0].is_within_tolerance);
        assert_eq!(result.as_of_rows()[0].distance_magnitude, Some(i64::MAX));
        assert_eq!(result.as_of_rows()[0].status, AsOfMatchStatus::Matched);
    }

    #[test]
    fn timestamp_axis_lead_lag_and_as_of_use_event_time_ranges() {
        #[derive(Clone)]
        struct QuoteEvent {
            selection_id: &'static str,
            observed_at: i64,
            active: bool,
        }

        let mut pipeline = crate::for_events::<QuoteEvent>()
            .record_windows()
            .with_event_time(|event| event.observed_at)
            .track_window("Quote", |event| event.selection_id, |event| event.active)
            .build()
            .expect("valid quote pipeline");
        pipeline.ingest(
            QuoteEvent {
                selection_id: "selection-1",
                observed_at: 900,
                active: true,
            },
            Some("quote"),
            None,
        );
        pipeline.ingest(
            QuoteEvent {
                selection_id: "selection-1",
                observed_at: 1_000,
                active: true,
            },
            Some("trade"),
            None,
        );
        pipeline.ingest(
            QuoteEvent {
                selection_id: "selection-1",
                observed_at: 1_100,
                active: false,
            },
            Some("trade"),
            None,
        );
        pipeline.ingest(
            QuoteEvent {
                selection_id: "selection-1",
                observed_at: 1_200,
                active: false,
            },
            Some("quote"),
            None,
        );
        let history = pipeline.history();

        let lead_lag = compare(
            history,
            &ComparisonPlan {
                name: "Timestamp latency".to_owned(),
                selection: ComparisonSelection::legacy(
                    "trade",
                    AgainstSelection::Sources(vec!["quote".to_owned()]),
                ),
                scope_window: Some("Quote".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::LeadLag {
                    transition: LeadLagTransition::Start,
                    axis: TemporalAxis::Timestamp,
                    tolerance_magnitude: 150,
                }],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::Timestamp,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );

        assert_eq!(lead_lag.lead_lag_rows().len(), 1);
        assert_eq!(lead_lag.lead_lag_rows()[0].axis, TemporalAxis::Timestamp);
        assert_eq!(lead_lag.lead_lag_rows()[0].delta_magnitude, Some(100));
        assert_eq!(
            lead_lag.lead_lag_rows()[0].direction,
            LeadLagDirection::TargetLags
        );
        assert!(lead_lag.lead_lag_rows()[0].is_within_tolerance);

        let as_of = compare(
            history,
            &ComparisonPlan {
                name: "Timestamp quote".to_owned(),
                selection: ComparisonSelection::legacy(
                    "trade",
                    AgainstSelection::Sources(vec!["quote".to_owned()]),
                ),
                scope_window: Some("Quote".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::AsOf {
                    direction: AsOfDirection::Previous,
                    axis: TemporalAxis::Timestamp,
                    tolerance_magnitude: 150,
                }],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::Timestamp,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );

        assert_eq!(as_of.as_of_rows().len(), 1);
        assert_eq!(as_of.as_of_rows()[0].axis, TemporalAxis::Timestamp);
        assert_eq!(as_of.as_of_rows()[0].status, AsOfMatchStatus::Matched);
        assert_eq!(as_of.as_of_rows()[0].distance_magnitude, Some(100));
    }

    #[test]
    fn residual_against_all_cohort_requires_every_member_active() {
        let history = WindowHistoryFixture::new()
            .closed_window("SelectionPriced", "selection-1", 1, 11, |w| {
                w.source("source-a")
            })
            .expect("target")
            .closed_window("SelectionPriced", "selection-1", 1, 6, |w| {
                w.source("source-b")
            })
            .expect("b")
            .closed_window("SelectionPriced", "selection-1", 6, 11, |w| {
                w.source("source-c")
            })
            .expect("c")
            .build();

        let result = compare(
            &history,
            &ComparisonPlan {
                name: "cohort all".to_owned(),
                selection: ComparisonSelection::legacy(
                    "source-a",
                    AgainstSelection::Cohort {
                        name: "cohort".to_owned(),
                        sources: vec!["source-b".to_owned(), "source-c".to_owned()],
                        activity: CohortActivity::All,
                    },
                ),
                scope_window: Some("SelectionPriced".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::Residual],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::ProcessingPosition,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );

        let total: i64 = result
            .residual_rows()
            .iter()
            .map(|row| row.range.end - row.range.start)
            .sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn residual_against_threshold_and_none_cohorts_use_activity_rules() {
        let threshold_history = WindowHistoryFixture::new()
            .closed_window("SelectionPriced", "selection-1", 1, 11, |w| {
                w.source("source-a")
            })
            .expect("target")
            .closed_window("SelectionPriced", "selection-1", 1, 11, |w| {
                w.source("source-b")
            })
            .expect("b")
            .closed_window("SelectionPriced", "selection-1", 1, 6, |w| {
                w.source("source-c")
            })
            .expect("c")
            .closed_window("SelectionPriced", "selection-1", 6, 11, |w| {
                w.source("source-d")
            })
            .expect("d")
            .build();

        let threshold = compare(
            &threshold_history,
            &ComparisonPlan {
                name: "cohort at least".to_owned(),
                selection: ComparisonSelection::legacy(
                    "source-a",
                    AgainstSelection::Cohort {
                        name: "cohort".to_owned(),
                        sources: vec![
                            "source-b".to_owned(),
                            "source-c".to_owned(),
                            "source-d".to_owned(),
                        ],
                        activity: CohortActivity::AtLeast { count: 2 },
                    },
                ),
                scope_window: Some("SelectionPriced".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::Residual],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::ProcessingPosition,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );
        assert!(threshold.residual_rows().is_empty());

        let none_history = WindowHistoryFixture::new()
            .closed_window("SelectionPriced", "selection-1", 1, 11, |w| {
                w.source("source-a")
            })
            .expect("target")
            .closed_window("SelectionPriced", "selection-1", 1, 6, |w| {
                w.source("source-b")
            })
            .expect("b")
            .build();

        let none = compare(
            &none_history,
            &ComparisonPlan {
                name: "cohort none".to_owned(),
                selection: ComparisonSelection::legacy(
                    "source-a",
                    AgainstSelection::Cohort {
                        name: "cohort".to_owned(),
                        sources: vec!["source-b".to_owned(), "source-c".to_owned()],
                        activity: CohortActivity::None,
                    },
                ),
                scope_window: Some("SelectionPriced".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::Residual],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::ProcessingPosition,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );
        let total: i64 = none
            .residual_rows()
            .iter()
            .map(|row| row.range.end - row.range.start)
            .sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn live_open_windows_emit_provisional_row_finality() {
        let history = WindowHistoryFixture::new()
            .open_window("DeviceOffline", "device-1", 1, |w| w.source("provider-a"))
            .expect("provider-a")
            .closed_window("DeviceOffline", "device-1", 3, 5, |w| {
                w.source("provider-b")
            })
            .expect("provider-b")
            .build();
        let plan = ComparisonPlan {
            name: "Live QA".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Residual],
            require_closed_windows: false,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::ClipToHorizon,
            open_window_horizon: Some(crate::TemporalPoint::position(10)),
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare_live(&history, &plan, crate::TemporalPoint::position(10));

        assert_eq!(result.residual_rows().len(), 2);
        assert!(result.has_provisional_rows());
        assert_eq!(result.provisional_row_finalities().len(), 2);
        assert_eq!(
            result.provisional_row_finalities()[0].finality,
            ComparisonFinality::Provisional
        );
    }

    #[test]
    fn prepare_excludes_future_windows_at_known_at_position() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a").known_at_position(10)
            })
            .expect("window")
            .build();
        let plan = ComparisonPlan {
            name: "Decision audit".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: Some(crate::TemporalPoint::position(5)),
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let prepared = prepare(&history, &plan);

        assert!(prepared.normalized_windows.is_empty());
        assert_eq!(prepared.excluded_windows.len(), 1);
        assert_eq!(
            prepared.excluded_windows[0].diagnostic_code.as_deref(),
            Some("FutureWindowExcluded")
        );
    }

    #[test]
    fn scope_key_and_partition_filter_selected_windows() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a").partition("fleet-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 2, 6, |w| {
                w.source("provider-b").partition("fleet-a")
            })
            .expect("against")
            .closed_window("DeviceOffline", "device-2", 1, 5, |w| {
                w.source("provider-a").partition("fleet-a")
            })
            .expect("other key")
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a").partition("fleet-b")
            })
            .expect("other partition")
            .build();
        let plan = ComparisonPlan {
            name: "Scoped".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: Some("device-1".to_owned()),
            scope_partition: Some("fleet-a".to_owned()),
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert_eq!(result.overlap_rows().len(), 1);
        assert_eq!(result.overlap_rows()[0].key, "device-1");
        assert_eq!(
            result.overlap_rows()[0].partition.as_deref(),
            Some("fleet-a")
        );
        assert_eq!(
            result.prepared.as_ref().expect("prepared")["selectedWindows"]
                .as_array()
                .expect("selected windows")
                .len(),
            2
        );
    }

    #[test]
    fn normalization_can_reject_duplicate_windows() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("duplicate target")
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();
        let plan = ComparisonPlan {
            name: "Duplicate QA".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Reject,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert_eq!(result.overlap_rows().len(), 1);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "DuplicateWindow")
        );
        assert_eq!(result.overlap_rows()[0].target_record_ids.len(), 1);
    }

    #[test]
    fn normalization_can_coalesce_adjacent_windows() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 3, |w| {
                w.source("provider-a")
            })
            .expect("target first")
            .closed_window("DeviceOffline", "device-1", 3, 5, |w| {
                w.source("provider-a")
            })
            .expect("target second")
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();
        let plan = ComparisonPlan {
            name: "Coalesce QA".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: true,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert_eq!(result.overlap_rows().len(), 1);
        assert_eq!(result.overlap_rows()[0].range.start, 1);
        assert_eq!(result.overlap_rows()[0].range.end, 5);
        assert_eq!(result.overlap_rows()[0].target_record_ids.len(), 2);
    }

    #[test]
    fn runtime_critic_warns_for_broad_scope_and_future_leakage_risk() {
        let history = WindowHistoryFixture::new()
            .closed_window("Quote", "selection-1", 10, 11, |w| w.source("trade"))
            .expect("trade")
            .closed_window("Quote", "selection-1", 7, 20, |w| w.source("quote"))
            .expect("quote")
            .build();
        let plan = ComparisonPlan {
            name: "Runtime critic".to_owned(),
            selection: ComparisonSelection::legacy(
                "trade",
                AgainstSelection::Sources(vec!["quote".to_owned()]),
            ),
            scope_window: None,
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::AsOf {
                direction: AsOfDirection::Previous,
                axis: TemporalAxis::ProcessingPosition,
                tolerance_magnitude: 5,
            }],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert!(result.is_valid);
        assert!(
            result
                .warning_diagnostics()
                .iter()
                .any(|item| item.code == "BroadSelector")
        );
        assert!(
            result
                .warning_diagnostics()
                .iter()
                .any(|item| item.code == "FutureLeakageRisk")
        );
    }

    #[test]
    fn runtime_selector_plans_execute_locally_and_emit_warning() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 7, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();

        let result = history
            .compare("Runtime selector QA")
            .target_selector(ComparisonSelector::runtime_only(
                "dynamic-target",
                "runtime target predicate",
                |window| window.source() == Some("provider-a"),
            ))
            .against_selector(ComparisonSelector::for_source("provider-b"))
            .scope_window("DeviceOffline")
            .overlap()
            .run();

        assert!(result.is_valid);
        assert_eq!(result.overlap_rows().len(), 1);
        assert_eq!(
            result.prepared.as_ref().expect("prepared")["normalizedWindows"][0]["selectorName"],
            "dynamic-target"
        );
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "RuntimeNonSerializablePlan"
                && diagnostic.severity == DiagnosticSeverity::Warning
        }));
    }

    #[test]
    fn strict_runtime_selector_plan_blocks_materialization() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 7, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();

        let result = history
            .compare("Strict runtime selector QA")
            .target_selector(ComparisonSelector::runtime_only(
                "dynamic-target",
                "runtime target predicate",
                |window| window.source() == Some("provider-a"),
            ))
            .against_selector(ComparisonSelector::for_source("provider-b"))
            .scope_window("DeviceOffline")
            .overlap()
            .strict()
            .run();

        assert!(!result.is_valid);
        assert!(result.aligned.is_none());
        assert!(result.overlap_rows().is_empty());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "RuntimeNonSerializablePlan"
                && diagnostic.severity == DiagnosticSeverity::Error
        }));
    }

    #[test]
    fn strict_runtime_critic_blocks_rows_and_alignment() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 6, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();
        let plan = ComparisonPlan {
            name: "Strict broad".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: None,
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: true,
        };

        let result = compare(&history, &plan);

        assert!(!result.is_valid);
        assert!(result.aligned.is_none());
        assert!(result.overlap_rows().is_empty());
        assert!(
            result
                .error_diagnostics()
                .iter()
                .any(|item| item.code == "BroadSelector")
        );
    }

    #[test]
    fn runtime_critic_reports_unbounded_open_duration() {
        let history = WindowHistoryFixture::new()
            .open_window("DeviceOffline", "device-1", 1, |w| w.source("provider-a"))
            .expect("provider-a")
            .build();
        let plan = ComparisonPlan {
            name: "Open QA".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert!(!result.is_valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "OpenWindowsWithoutPolicy")
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "UnboundedOpenDuration")
        );
    }

    #[test]
    fn runtime_critic_reports_live_finality_without_horizon() {
        let history = WindowHistoryFixture::new().build();
        let plan = ComparisonPlan {
            name: "Live QA".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: false,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::ClipToHorizon,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert!(result.is_valid);
        assert!(
            result
                .warning_diagnostics()
                .iter()
                .any(|item| item.code == "LiveFinalityWithoutHorizon")
        );
    }

    #[test]
    fn runtime_critic_reports_mixed_timestamp_clocks() {
        let history = WindowHistoryFixture::new().build();
        let plan = ComparisonPlan {
            name: "Clock QA".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: false,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::Timestamp,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: Some(crate::TemporalPoint::timestamp_ticks_with_clock(
                1, "received",
            )),
            open_window_policy: OpenWindowPolicy::ClipToHorizon,
            open_window_horizon: Some(crate::TemporalPoint::timestamp_ticks_with_clock(
                1, "provider",
            )),
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let result = compare(&history, &plan);

        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "MixedClockRisk"
                && diagnostic.severity == DiagnosticSeverity::Warning
        }));
    }

    #[test]
    fn closed_windows_default_known_at_to_close_position() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("window")
            .build();
        let plan = ComparisonPlan {
            name: "Decision audit".to_owned(),
            selection: ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: Some(crate::TemporalPoint::position(4)),
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let prepared = prepare(&history, &plan);

        assert!(prepared.normalized_windows.is_empty());
        assert_eq!(prepared.excluded_windows.len(), 1);
        assert_eq!(
            prepared.excluded_windows[0].diagnostic_code.as_deref(),
            Some("FutureWindowExcluded")
        );
    }

    #[test]
    fn event_time_normalization_excludes_position_only_windows_by_policy() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();

        let result = history
            .compare("Event-time QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .normalization(
                ComparisonNormalizationPolicy::event_time().excluding_missing_event_time(),
            )
            .overlap()
            .run();

        assert!(result.is_valid);
        assert!(result.overlap_rows().is_empty());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MissingEventTime"
                    && diagnostic.severity == DiagnosticSeverity::Warning)
        );
    }

    #[test]
    fn event_time_normalization_rejects_position_only_windows_by_default() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .build();

        let result = history
            .compare("Event-time QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .normalization(ComparisonNormalizationPolicy::event_time())
            .overlap()
            .run();

        assert!(!result.is_valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MissingEventTime"
                    && diagnostic.severity == DiagnosticSeverity::Error)
        );
    }

    #[test]
    fn cohort_comparison_emits_extension_metadata_and_parsed_evidence() {
        let history = WindowHistoryFixture::new()
            .closed_window("SelectionPriced", "selection-1", 1, 11, |w| {
                w.source("source-a")
            })
            .expect("target")
            .closed_window("SelectionPriced", "selection-1", 1, 6, |w| {
                w.source("source-b")
            })
            .expect("b")
            .closed_window("SelectionPriced", "selection-1", 6, 11, |w| {
                w.source("source-c")
            })
            .expect("c")
            .build();

        let result = compare(
            &history,
            &ComparisonPlan {
                name: "cohort evidence".to_owned(),
                selection: ComparisonSelection::legacy(
                    "source-a",
                    AgainstSelection::Cohort {
                        name: "cohort".to_owned(),
                        sources: vec!["source-b".to_owned(), "source-c".to_owned()],
                        activity: CohortActivity::All,
                    },
                ),
                scope_window: Some("SelectionPriced".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![Comparator::Residual],
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: TemporalAxis::ProcessingPosition,
                null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        );

        assert!(!result.extension_metadata.is_empty());
        let evidence = result.cohort_evidence();
        assert_eq!(evidence.len(), 2);
        assert!(!evidence[0].is_active);
        assert!(!evidence[1].is_active);
    }

    #[test]
    fn cohort_evidence_counts_a_source_until_its_last_overlapping_window_leaves() {
        let history = WindowHistoryFixture::new()
            .closed_window("SelectionPriced", "selection-1", 0, 10, |window| {
                window.source("target")
            })
            .expect("target")
            .closed_window("SelectionPriced", "selection-1", 0, 6, |window| {
                window.source("source-b")
            })
            .expect("first source-b window")
            .closed_window("SelectionPriced", "selection-1", 2, 10, |window| {
                window.source("source-b")
            })
            .expect("second source-b window")
            .closed_window("SelectionPriced", "selection-1", 4, 8, |window| {
                window.source("source-c")
            })
            .expect("source-c window")
            .build();

        let result = history
            .compare("duplicate source activity")
            .target_source("target")
            .against_cohort(
                "cohort",
                ["source-b", "source-c"],
                CohortActivity::AtLeast { count: 2 },
            )
            .scope_window("SelectionPriced")
            .overlap()
            .run();

        let evidence = result.cohort_evidence();
        assert_eq!(
            evidence
                .iter()
                .map(|segment| segment.active_count)
                .collect::<Vec<_>>(),
            vec![1, 1, 2, 2, 1]
        );
        assert_eq!(
            evidence
                .iter()
                .map(|segment| segment.is_active)
                .collect::<Vec<_>>(),
            vec![false, false, true, true, false]
        );
        assert_eq!(
            evidence[3].active_sources,
            vec!["source-b".to_owned(), "source-c".to_owned()]
        );
    }
}
