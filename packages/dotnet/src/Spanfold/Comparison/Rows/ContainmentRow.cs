namespace Spanfold;

/// <summary>
/// Describes whether target windows are contained by comparison windows for one segment.
/// </summary>
/// <remarks>
/// Containment is directional: target records are the windows being checked,
/// and comparison records are the expected container windows. For example,
/// "job running" target windows can be checked against "worker online"
/// container windows.
/// </remarks>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The checked temporal range.</param>
/// <param name="Status">The containment status for the range.</param>
/// <param name="TargetRecordIds">The target window IDs active for the range.</param>
/// <param name="ContainerRecordIds">The comparison window IDs that contain the range.</param>
public sealed record ContainmentRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    ContainmentStatus Status,
    IReadOnlyList<WindowRecordId> TargetRecordIds,
    IReadOnlyList<WindowRecordId> ContainerRecordIds)
{
    /// <summary>Gets the target record IDs active for the range.</summary>
    public IReadOnlyList<WindowRecordId> TargetRecordIds { get; } = Array.AsReadOnly(TargetRecordIds.ToArray());

    /// <summary>Gets the comparison record IDs containing the range.</summary>
    public IReadOnlyList<WindowRecordId> ContainerRecordIds { get; } = Array.AsReadOnly(ContainerRecordIds.ToArray());
}
