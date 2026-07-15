using Spanfold.Comparison;
using Spanfold.Artifacts.Internal;

namespace Spanfold.Artifacts;

/// <summary>
/// Provides deterministic diagnostic text for comparison artifacts.
/// </summary>
/// <remarks>
/// Explain output is deterministic diagnostic text for humans and agents. It is
/// not generated natural-language interpretation and is only built when these
/// methods are called.
/// </remarks>
public static class ComparisonExplainExtensions
{
    /// <summary>
    /// Renders deterministic diagnostic text for a comparison plan.
    /// </summary>
    /// <param name="plan">The comparison plan to explain.</param>
    /// <param name="format">The output text format.</param>
    /// <returns>Diagnostic text describing the plan.</returns>
    public static string Explain(
        this ComparisonPlan plan,
        ComparisonExplanationFormat format = ComparisonExplanationFormat.Markdown)
    {
        ArgumentNullException.ThrowIfNull(plan);

        return ComparisonExplainer.Explain(plan, format);
    }

    /// <summary>
    /// Renders deterministic diagnostic text for a prepared comparison.
    /// </summary>
    /// <param name="prepared">The prepared comparison to explain.</param>
    /// <param name="format">The output text format.</param>
    /// <returns>Diagnostic text describing preparation and normalization.</returns>
    public static string Explain(
        this PreparedComparison prepared,
        ComparisonExplanationFormat format = ComparisonExplanationFormat.Markdown)
    {
        ArgumentNullException.ThrowIfNull(prepared);

        return ComparisonExplainer.Explain(prepared, format);
    }

    /// <summary>
    /// Renders deterministic diagnostic text for an aligned comparison.
    /// </summary>
    /// <param name="aligned">The aligned comparison to explain.</param>
    /// <param name="format">The output text format.</param>
    /// <returns>Diagnostic text describing alignment segments and lineage.</returns>
    public static string Explain(
        this AlignedComparison aligned,
        ComparisonExplanationFormat format = ComparisonExplanationFormat.Markdown)
    {
        ArgumentNullException.ThrowIfNull(aligned);

        return ComparisonExplainer.Explain(aligned, format);
    }

    /// <summary>
    /// Renders deterministic diagnostic text for a comparison result.
    /// </summary>
    /// <param name="result">The comparison result to explain.</param>
    /// <param name="format">The output text format.</param>
    /// <returns>Diagnostic text describing diagnostics, rows, summaries, and lineage.</returns>
    public static string Explain(
        this ComparisonResult result,
        ComparisonExplanationFormat format = ComparisonExplanationFormat.Markdown)
    {
        ArgumentNullException.ThrowIfNull(result);

        return ComparisonExplainer.Explain(result, format);
    }
}
