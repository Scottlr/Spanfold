//! Comparison diagnostic DTOs, catalog, and constructors.

use serde::Serialize;

/// Diagnostic severity.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DiagnosticSeverity {
    /// Warning-level diagnostic.
    Warning,
    /// Error-level diagnostic.
    Error,
}

/// Structured comparison diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonDiagnostic {
    /// Diagnostic code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: DiagnosticSeverity,
}

impl ComparisonDiagnostic {
    /// Returns an actionable remediation hint for this diagnostic code.
    #[must_use]
    pub fn message(&self) -> &'static str {
        match self.code.as_str() {
            "MissingName" => "set a non-empty comparison plan name",
            "MissingTarget" => "configure a target source or selector",
            "MissingAgainst" => "configure at least one comparison source or selector",
            "MissingComparator" => "configure at least one comparator",
            "FutureWindowExcluded" => "advance known-at or provide an earlier-available window",
            "MissingEventTime" => {
                "provide event timestamps or choose processing-position normalization"
            }
            "TemporalAxisMismatch" => "align the plan axis with the recorded window axis",
            "SelfComparison" => "make target and comparison selectors disjoint",
            "RuntimeNonSerializablePlan" => "use serializable selectors for portable execution",
            _ => "inspect the prepared artifact and plan fields for the invalid contract",
        }
    }
}

pub(super) fn plan_diagnostic(code: &str, severity: DiagnosticSeverity) -> ComparisonDiagnostic {
    ComparisonDiagnostic {
        code: code.to_owned(),
        severity,
    }
}

pub(super) fn push_diagnostic_once(
    diagnostics: &mut Vec<ComparisonDiagnostic>,
    code: &str,
    severity: DiagnosticSeverity,
) {
    if diagnostics.iter().any(|diagnostic| diagnostic.code == code) {
        return;
    }
    diagnostics.push(ComparisonDiagnostic {
        code: code.to_owned(),
        severity,
    });
}
