namespace Spanfold.Comparison;

/// <summary>
/// Describes target coverage for one aligned target-active segment.
/// </summary>
/// <remarks>
/// A segment is normally wholly covered or wholly uncovered, so its covered
/// magnitude is normally zero or the complete target magnitude. Use
/// <see cref="CoverageSummary" /> for grouped aggregate coverage.
/// </remarks>
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
    /// <summary>Gets the target record IDs active for the segment.</summary>
    public IReadOnlyList<WindowRecordId> TargetRecordIds { get; } = Array.AsReadOnly(TargetRecordIds.ToArray());

    /// <summary>Gets the comparison record IDs active for the segment.</summary>
    public IReadOnlyList<WindowRecordId> AgainstRecordIds { get; } = Array.AsReadOnly(AgainstRecordIds.ToArray());

    /// <summary>Gets the exact target magnitude when supplied by the runtime.</summary>
    public long TargetMagnitudeExact => ExactTargetMagnitude ?? checked((long)TargetMagnitude);

    /// <summary>Gets the exact covered magnitude when supplied by the runtime.</summary>
    public long CoveredMagnitudeExact => ExactCoveredMagnitude ?? checked((long)CoveredMagnitude);
}
