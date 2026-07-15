namespace Spanfold.Comparison;

/// <summary>
/// Describes a validation issue found in a comparison plan.
/// </summary>
/// <param name="Code">The stable validation code.</param>
/// <param name="Message">A readable diagnostic message.</param>
/// <param name="Path">The plan path related to the diagnostic.</param>
/// <param name="Severity">The diagnostic severity.</param>
public sealed record ComparisonPlanDiagnostic(
    ComparisonPlanValidationCode Code,
    string Message,
    string Path,
    ComparisonPlanDiagnosticSeverity Severity = ComparisonPlanDiagnosticSeverity.Error);
