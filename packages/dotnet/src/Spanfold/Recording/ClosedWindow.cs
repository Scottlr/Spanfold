namespace Spanfold;

/// <summary>
/// Describes a closed recorded window.
/// </summary>
public sealed record ClosedWindow : WindowRecord
{
    /// <summary>
    /// Creates a closed recorded window.
    /// </summary>
    /// <param name="WindowName">The configured window name.</param>
    /// <param name="Key">The logical key for the window.</param>
    /// <param name="StartPosition">The processing position where the window opened.</param>
    /// <param name="EndPosition">The processing position where the window closed.</param>
    /// <param name="Source">Optional source identity supplied when the window opened.</param>
    /// <param name="Partition">Optional partition identity supplied when the window opened.</param>
    /// <param name="StartTime">Optional event timestamp where the window opened.</param>
    /// <param name="EndTime">Optional event timestamp where the window closed.</param>
    /// <param name="Segments">Analytical segment values attached to this window.</param>
    /// <param name="Tags">Descriptive non-boundary metadata attached to this window.</param>
    /// <param name="BoundaryReason">The reason this window closed, when known.</param>
    /// <param name="BoundaryChanges">The segment changes that caused this window to close.</param>
    /// <param name="TimestampClock">Optional identity of the clock that produced the timestamps.</param>
    public ClosedWindow(
        string WindowName,
        object Key,
        long StartPosition,
        long EndPosition,
        object? Source = null,
        object? Partition = null,
        DateTimeOffset? StartTime = null,
        DateTimeOffset? EndTime = null,
        IReadOnlyList<WindowSegment>? Segments = null,
        IReadOnlyList<WindowTag>? Tags = null,
        WindowBoundaryReason? BoundaryReason = null,
        IReadOnlyList<WindowBoundaryChange>? BoundaryChanges = null,
        string? TimestampClock = null)
        : base(
        WindowName,
        Key,
        StartPosition,
        EndPosition,
        Source,
        Partition,
        StartTime,
        EndTime,
        Segments,
        Tags,
        BoundaryReason,
        BoundaryChanges,
        TimestampClock)
    {
    }
}
