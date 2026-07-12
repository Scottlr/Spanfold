namespace Spanfold;

/// <summary>
/// Describes a target-only or comparison-only disagreement segment.
/// </summary>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The disagreement temporal range.</param>
/// <param name="Side">The side that was active for the disagreement.</param>
/// <param name="TargetRecordIds">The target window IDs active for the segment.</param>
/// <param name="AgainstRecordIds">The comparison window IDs active for the segment.</param>
public sealed record SymmetricDifferenceRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    ComparisonSide Side,
    IReadOnlyList<WindowRecordId> TargetRecordIds,
    IReadOnlyList<WindowRecordId> AgainstRecordIds)
{
    /// <summary>Gets the target record IDs active for the segment.</summary>
    public IReadOnlyList<WindowRecordId> TargetRecordIds { get; } = Array.AsReadOnly(TargetRecordIds.ToArray());

    /// <summary>Gets the comparison record IDs active for the segment.</summary>
    public IReadOnlyList<WindowRecordId> AgainstRecordIds { get; } = Array.AsReadOnly(AgainstRecordIds.ToArray());
}
