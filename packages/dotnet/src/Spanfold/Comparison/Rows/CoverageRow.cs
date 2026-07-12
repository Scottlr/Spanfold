namespace Spanfold;

/// <summary>
/// Describes target coverage for one aligned segment.
/// </summary>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The covered temporal range.</param>
/// <param name="TargetMagnitude">The target denominator magnitude for the segment.</param>
/// <param name="CoveredMagnitude">The comparison-covered numerator magnitude for the segment.</param>
/// <param name="TargetRecordIds">The target window IDs active for the segment.</param>
/// <param name="AgainstRecordIds">The comparison window IDs active for the segment.</param>
public sealed record CoverageRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    double TargetMagnitude,
    double CoveredMagnitude,
    IReadOnlyList<WindowRecordId> TargetRecordIds,
    IReadOnlyList<WindowRecordId> AgainstRecordIds,
    long? ExactTargetMagnitude = null,
    long? ExactCoveredMagnitude = null)
{
    /// <summary>Gets the exact target magnitude when supplied by the runtime.</summary>
    public long TargetMagnitudeExact => ExactTargetMagnitude ?? checked((long)TargetMagnitude);

    /// <summary>Gets the exact covered magnitude when supplied by the runtime.</summary>
    public long CoveredMagnitudeExact => ExactCoveredMagnitude ?? checked((long)CoveredMagnitude);
}
