using Spanfold.Internal.Comparison;

namespace Spanfold;

/// <summary>
/// Represents the current prepared comparison state.
/// </summary>
/// <remarks>
/// This artifact carries the plan, validation diagnostics, selected windows,
/// excluded windows, and normalized windows ready for alignment. Preparation is
/// the first stage that enumerates recorded history and turns open or timestamp
/// records into deterministic temporal ranges according to the normalization
/// policy.
/// </remarks>
/// <param name="Plan">The comparison plan.</param>
/// <param name="Diagnostics">The validation diagnostics.</param>
/// <param name="SelectedWindows">The recorded windows selected by the plan.</param>
/// <param name="ExcludedWindows">The recorded windows excluded during preparation.</param>
/// <param name="NormalizedWindows">The normalized windows ready for alignment.</param>
public sealed record PreparedComparison
{
    internal PreparedComparison(
        ComparisonPlan plan,
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics,
        IReadOnlyList<WindowRecord> selectedWindows,
        IReadOnlyList<ExcludedWindowRecord> excludedWindows,
        IReadOnlyList<NormalizedWindowRecord> normalizedWindows)
    {
        Plan = plan;
        Diagnostics = diagnostics;
        SelectedWindows = selectedWindows;
        ExcludedWindows = excludedWindows;
        NormalizedWindows = normalizedWindows;
    }

    /// <summary>Gets the comparison plan.</summary>
    public ComparisonPlan Plan { get; }
    /// <summary>Gets validation diagnostics.</summary>
    public IReadOnlyList<ComparisonPlanDiagnostic> Diagnostics { get; }
    /// <summary>Gets selected source windows.</summary>
    public IReadOnlyList<WindowRecord> SelectedWindows { get; }
    /// <summary>Gets windows excluded during preparation.</summary>
    public IReadOnlyList<ExcludedWindowRecord> ExcludedWindows { get; }
    /// <summary>Gets normalized windows ready for alignment.</summary>
    public IReadOnlyList<NormalizedWindowRecord> NormalizedWindows { get; }

    /// <summary>
    /// Aligns normalized windows into reusable temporal segments.
    /// </summary>
    /// <remarks>
    /// Alignment preserves comparison scope boundaries and emits half-open
    /// segments in deterministic order by window name, key, partition, and
    /// temporal position.
    /// </remarks>
    /// <returns>The aligned comparison.</returns>
    public AlignedComparison Align()
    {
        return ComparisonAligner.Align(this);
    }
}
