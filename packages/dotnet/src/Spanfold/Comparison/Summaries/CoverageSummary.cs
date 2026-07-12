namespace Spanfold;

/// <summary>
/// Summarizes target coverage within one comparison scope.
/// </summary>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="TargetMagnitude">The denominator magnitude.</param>
/// <param name="CoveredMagnitude">The covered numerator magnitude.</param>
/// <param name="CoverageRatio">The covered numerator divided by the denominator.</param>
public sealed record CoverageSummary(
    string WindowName,
    object Key,
    object? Partition,
    double TargetMagnitude,
    double CoveredMagnitude,
    double CoverageRatio,
    long? ExactTargetMagnitude = null,
    long? ExactCoveredMagnitude = null)
{
    /// <summary>Gets the exact target magnitude when supplied by the runtime.</summary>
    public long TargetMagnitudeExact => ExactTargetMagnitude ?? checked((long)TargetMagnitude);

    /// <summary>Gets the exact covered magnitude when supplied by the runtime.</summary>
    public long CoveredMagnitudeExact => ExactCoveredMagnitude ?? checked((long)CoveredMagnitude);
}
