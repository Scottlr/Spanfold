//! Comparison execution, comparator dispatch, and canonical result materialization.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::{ComparisonExtensionMetadata, TemporalPoint, WindowHistory};

use super::Comparator;
use super::align::{self, AlignedComparison};
use super::comparators::{
    build_as_of_rows, build_containment_rows, build_coverage_rows, build_gap_rows,
    build_lead_lag_rows, build_missing_rows, build_overlap_rows, build_residual_rows,
    build_symmetric_difference_rows,
};
use super::critic::runtime_critic_diagnostics;
use super::diagnostics::{ComparisonDiagnostic, DiagnosticSeverity};
use super::finality::build_row_state;
use super::plan::ComparisonPlan;
use super::prepare::prepare_internal;
use super::rows::{
    ComparatorSummary, ComparisonResult, ComparisonRows, CoverageRow, CoverageSummary,
    LeadLagSummary, RowAccumulator,
};
use super::selector::{AgainstSelection, CohortActivity};
use super::state::{ComparisonResultState, ComparisonRowState};

struct ResultArtifacts {
    comparator_summaries: Vec<ComparatorSummary>,
    coverage_summaries: Vec<CoverageSummary>,
    lead_lag_summaries: Vec<LeadLagSummary>,
    extension_metadata: Vec<ComparisonExtensionMetadata>,
    rows: ComparisonRows,
    state: ComparisonResultState,
}

pub(super) fn execute_compare(
    history: &WindowHistory,
    plan: &ComparisonPlan,
    live_horizon_override: Option<TemporalPoint>,
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
        result.known_at = plan
            .known_at
            .as_ref()
            .map(align::row_point_from_temporal_point);
        result.evaluation_horizon = live_horizon_override
            .as_ref()
            .or(plan.open_window_horizon.as_ref())
            .map(align::row_point_from_temporal_point);
        return result;
    }

    let groups = align::group_normalized_windows(&prepared);
    let aligned = align::align_grouped(&prepared, &groups);
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
    result.known_at = plan
        .known_at
        .as_ref()
        .map(align::row_point_from_temporal_point);
    result.evaluation_horizon = live_horizon_override
        .as_ref()
        .or(plan.open_window_horizon.as_ref())
        .map(align::row_point_from_temporal_point);
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
    result.known_at = plan
        .known_at
        .as_ref()
        .map(align::row_point_from_temporal_point);
    result.evaluation_horizon = plan
        .open_window_horizon
        .as_ref()
        .map(align::row_point_from_temporal_point);
    result
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
