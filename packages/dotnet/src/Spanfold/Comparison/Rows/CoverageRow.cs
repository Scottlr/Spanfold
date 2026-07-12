namespace Spanfold;

/// <summary>
/// Describes target coverage for one aligned segment.
/// </summary>
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
