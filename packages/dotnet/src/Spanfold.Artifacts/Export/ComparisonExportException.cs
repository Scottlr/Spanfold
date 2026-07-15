namespace Spanfold.Artifacts;

/// <summary>
/// Represents a comparison export failure with stable diagnostics.
/// </summary>
/// <remarks>
/// Export failures are deterministic and usually indicate that an artifact
/// contains runtime-only selectors or other data that cannot be represented as
/// portable JSON.
/// </remarks>
/// <param name="message">The readable failure message.</param>
/// <param name="diagnostics">The diagnostics that explain why export failed.</param>
public sealed class ComparisonExportException(
    string message,
    IEnumerable<ComparisonPlanDiagnostic> diagnostics)
    : InvalidOperationException(message)
{
    /// <summary>
    /// Gets the stable diagnostics that explain why export failed.
    /// </summary>
    public IReadOnlyList<ComparisonPlanDiagnostic> Diagnostics { get; } =
        MaterializeDiagnostics(diagnostics);

    private static IReadOnlyList<ComparisonPlanDiagnostic> MaterializeDiagnostics(
        IEnumerable<ComparisonPlanDiagnostic> diagnostics)
    {
        ArgumentNullException.ThrowIfNull(diagnostics);

        return diagnostics.ToArray();
    }
}
