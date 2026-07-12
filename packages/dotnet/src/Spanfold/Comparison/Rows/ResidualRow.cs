namespace Spanfold;

/// <summary>
/// Describes an aligned segment where target windows are active without comparison coverage.
/// </summary>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The residual temporal range.</param>
/// <param name="TargetRecordIds">The target window IDs active for the residual segment.</param>
public sealed record ResidualRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    IReadOnlyList<WindowRecordId> TargetRecordIds)
{
    /// <summary>Gets the target record IDs active for the residual segment.</summary>
    public IReadOnlyList<WindowRecordId> TargetRecordIds { get; } = Array.AsReadOnly(TargetRecordIds.ToArray());
}
