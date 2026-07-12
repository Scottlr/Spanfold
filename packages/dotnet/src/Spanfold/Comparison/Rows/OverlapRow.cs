namespace Spanfold;

/// <summary>
/// Describes an aligned segment where target and comparison windows overlap.
/// </summary>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The overlapping temporal range.</param>
/// <param name="TargetRecordIds">The target window IDs active for the overlap.</param>
/// <param name="AgainstRecordIds">The comparison window IDs active for the overlap.</param>
public sealed record OverlapRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    IReadOnlyList<WindowRecordId> TargetRecordIds,
    IReadOnlyList<WindowRecordId> AgainstRecordIds)
{
    /// <summary>Gets the target record IDs active for the overlap.</summary>
    public IReadOnlyList<WindowRecordId> TargetRecordIds { get; } = Array.AsReadOnly(TargetRecordIds.ToArray());

    /// <summary>Gets the comparison record IDs active for the overlap.</summary>
    public IReadOnlyList<WindowRecordId> AgainstRecordIds { get; } = Array.AsReadOnly(AgainstRecordIds.ToArray());
}
