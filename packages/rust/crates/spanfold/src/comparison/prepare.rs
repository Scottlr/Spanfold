//! Selection, scope preparation, normalization, and deduplication.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::window_normalization::{
    NormalizedWindowEvidence, RawWindowRef, WindowNormalizationFailure, WindowNormalizationRequest,
};
use crate::{TemporalPoint, TemporalRange, WindowHistory, WindowSegment, WindowTag};

use super::diagnostics::{ComparisonDiagnostic, DiagnosticSeverity, push_diagnostic_once};
use super::plan::{
    ComparisonDuplicateWindowPolicy, ComparisonNullTimestampPolicy, ComparisonPlan, ComparisonScope,
};
use super::rows::ComparisonSide;

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
    pub start: TemporalPoint,
    /// End temporal point when the source window is closed.
    pub end: Option<TemporalPoint>,
    /// Known-at temporal point, when supplied.
    #[serde(rename = "knownAt")]
    pub known_at: Option<TemporalPoint>,
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
    pub range: TemporalRange,
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

pub(super) fn prepare_internal(
    history: &WindowHistory,
    plan: &ComparisonPlan,
    live_horizon_override: Option<TemporalPoint>,
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
                previous.range = TemporalRange::new(previous.range.start(), window.range.end())
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
