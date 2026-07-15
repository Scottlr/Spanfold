namespace Spanfold.Comparison;

/// <summary>
/// Describes how strongly a comparison plan diagnostic should be treated.
/// </summary>
public enum ComparisonPlanDiagnosticSeverity
{
    /// <summary>
    /// Indicates an informational diagnostic.
    /// </summary>
    Info = 0,

    /// <summary>
    /// Indicates a diagnostic that should be reviewed but may not block execution.
    /// </summary>
    Warning = 1,

    /// <summary>
    /// Indicates a diagnostic that blocks safe execution.
    /// </summary>
    Error = 2
}
