namespace Spanfold;

/// <summary>
/// Describes a currently open recorded window.
/// </summary>
public sealed record OpenWindow : WindowRecord
{
    /// <summary>
    /// Creates a currently open recorded window.
    /// </summary>
    /// <param name="WindowName">The configured window name.</param>
    /// <param name="Key">The logical key for the window.</param>
    /// <param name="StartPosition">The processing position where the window opened.</param>
    /// <param name="Source">Optional source identity supplied when the window opened.</param>
    /// <param name="Partition">Optional partition identity supplied when the window opened.</param>
    /// <param name="StartTime">Optional event timestamp where the window opened.</param>
    /// <param name="Segments">Analytical segment values attached to this window.</param>
    /// <param name="Tags">Descriptive non-boundary metadata attached to this window.</param>
    public OpenWindow(
        string WindowName,
        object Key,
        long StartPosition,
        object? Source = null,
        object? Partition = null,
        DateTimeOffset? StartTime = null,
        IReadOnlyList<WindowSegment>? Segments = null,
        IReadOnlyList<WindowTag>? Tags = null)
        : base(
        WindowName,
        Key,
        StartPosition,
        null,
        Source,
        Partition,
        StartTime,
        null,
        Segments,
        Tags,
        null,
        null)
    {
    }
}
