namespace Spanfold.Comparison;

/// <summary>
/// Describes a recorded window excluded during comparison preparation.
/// </summary>
/// <param name="Window">The excluded recorded window.</param>
/// <param name="Reason">The readable exclusion reason.</param>
/// <param name="DiagnosticCode">The optional diagnostic code related to the exclusion.</param>
public sealed record ExcludedWindowRecord(
    WindowRecord Window,
    string Reason,
    ComparisonPlanValidationCode? DiagnosticCode = null);
