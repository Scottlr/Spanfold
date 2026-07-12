namespace Spanfold;

/// <summary>
/// Summarizes target coverage within one comparison scope.
/// </summary>
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
