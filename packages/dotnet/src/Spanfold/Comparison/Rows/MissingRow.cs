namespace Spanfold;

/// <summary>
/// Describes an aligned segment where comparison windows are active without target coverage.
/// </summary>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The missing temporal range.</param>
/// <param name="AgainstRecordIds">The comparison window IDs active for the missing segment.</param>
public sealed record MissingRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    IReadOnlyList<WindowRecordId> AgainstRecordIds)
{
    /// <summary>Gets the comparison record IDs active for the missing segment.</summary>
    public IReadOnlyList<WindowRecordId> AgainstRecordIds { get; } = Array.AsReadOnly(AgainstRecordIds.ToArray());
}
