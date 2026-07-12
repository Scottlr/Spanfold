namespace Spanfold;

/// <summary>
/// Describes one aligned temporal segment and the normalized windows active within it.
/// </summary>
/// <remarks>
/// Segments use half-open temporal ranges. Target and comparison record IDs
/// provide lineage back to the selected recorded windows.
/// </remarks>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The aligned segment range.</param>
/// <param name="TargetRecordIds">The target window IDs active for the segment.</param>
/// <param name="AgainstRecordIds">The comparison window IDs active for the segment.</param>
/// <param name="Segments">The segment context shared by the aligned segment.</param>
public sealed record AlignedSegment(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    IReadOnlyList<WindowRecordId> TargetRecordIds,
    IReadOnlyList<WindowRecordId> AgainstRecordIds,
    IReadOnlyList<WindowSegment>? Segments = null)
{
    /// <summary>
    /// Gets the segment context shared by the aligned segment.
    /// </summary>
    public IReadOnlyList<WindowSegment> Segments { get; } = Materialize(Segments);

    private static IReadOnlyList<T> Materialize<T>(IReadOnlyList<T>? values)
    {
        return values switch
        {
            null => [],
            T[] array => array.ToArray(),
            _ => values.ToArray()
        };
    }
}
