namespace Spanfold;

/// <summary>
/// Describes one aligned temporal segment and the normalized windows active within it.
/// </summary>
/// <remarks>
/// Segments use half-open temporal ranges. Target and comparison record IDs
/// provide lineage back to the selected recorded windows.
/// </remarks>
public sealed record AlignedSegment
{
    internal AlignedSegment(
        string windowName,
        object key,
        object? partition,
        TemporalRange range,
        IReadOnlyList<WindowRecordId> targetRecordIds,
        IReadOnlyList<WindowRecordId> againstRecordIds,
        IReadOnlyList<WindowSegment>? segments = null)
    {
        WindowName = windowName;
        Key = key;
        Partition = partition;
        Range = range;
        TargetRecordIds = Array.AsReadOnly(targetRecordIds.ToArray());
        AgainstRecordIds = Array.AsReadOnly(againstRecordIds.ToArray());
        Segments = Materialize(segments);
    }

    /// <summary>Gets the configured window name.</summary>
    public string WindowName { get; }
    /// <summary>Gets the logical window key.</summary>
    public object Key { get; }
    /// <summary>Gets the optional partition identity.</summary>
    public object? Partition { get; }
    /// <summary>Gets the aligned segment range.</summary>
    public TemporalRange Range { get; }
    /// <summary>Gets target record IDs active for the segment.</summary>
    public IReadOnlyList<WindowRecordId> TargetRecordIds { get; }
    /// <summary>Gets comparison record IDs active for the segment.</summary>
    public IReadOnlyList<WindowRecordId> AgainstRecordIds { get; }

    /// <summary>
    /// Gets the segment context shared by the aligned segment.
    /// </summary>
    public IReadOnlyList<WindowSegment> Segments { get; }

    private static IReadOnlyList<T> Materialize<T>(IReadOnlyList<T>? values)
    {
        return values switch
        {
            null => [],
            _ => Array.AsReadOnly(values.ToArray())
        };
    }
}
