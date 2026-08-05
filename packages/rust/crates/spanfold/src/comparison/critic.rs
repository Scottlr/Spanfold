//! Runtime diagnostics derived from a configured plan and prepared evidence.

use crate::TemporalAxis;

use super::diagnostics::push_diagnostic_once;
use super::{
    Comparator, ComparisonDiagnostic, ComparisonPlan, DiagnosticSeverity, OpenWindowPolicy,
    PreparedComparison,
};

pub(super) fn runtime_critic_diagnostics(
    plan: &ComparisonPlan,
    prepared: &PreparedComparison,
    live_horizon_override: Option<crate::TemporalPoint>,
) -> Vec<ComparisonDiagnostic> {
    let severity = if plan.strict {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    };
    let mut diagnostics = Vec::new();

    if !plan.is_serializable() {
        push_diagnostic_once(
            &mut diagnostics,
            "RuntimeNonSerializablePlan",
            severity.clone(),
        );
    }
    if plan.scope_window.is_none() {
        push_diagnostic_once(&mut diagnostics, "BroadSelector", severity.clone());
    }
    if plan.known_at.is_none()
        && plan
            .comparators
            .iter()
            .any(|item| matches!(item, Comparator::AsOf { .. }))
    {
        push_diagnostic_once(&mut diagnostics, "FutureLeakageRisk", severity.clone());
    }
    if plan.open_window_policy == OpenWindowPolicy::ClipToHorizon
        && plan.open_window_horizon.is_none()
        && live_horizon_override.is_none()
    {
        push_diagnostic_once(
            &mut diagnostics,
            "LiveFinalityWithoutHorizon",
            severity.clone(),
        );
    }
    if prepared
        .excluded_windows
        .iter()
        .any(|window| window.diagnostic_code.as_deref() == Some("OpenWindowsWithoutPolicy"))
    {
        push_diagnostic_once(&mut diagnostics, "UnboundedOpenDuration", severity.clone());
    }
    if let (Some(horizon), Some(known_at)) =
        (plan.open_window_horizon.as_ref(), plan.known_at.as_ref())
        && horizon.axis() == TemporalAxis::Timestamp
        && known_at.axis() == TemporalAxis::Timestamp
        && horizon.clock() != known_at.clock()
    {
        push_diagnostic_once(&mut diagnostics, "MixedClockRisk", severity);
    }

    diagnostics
}
