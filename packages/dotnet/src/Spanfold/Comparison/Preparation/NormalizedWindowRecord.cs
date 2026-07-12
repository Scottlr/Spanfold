namespace Spanfold;

/// <summary>
/// Describes a recorded window after comparison normalization.
/// </summary>
/// <param name="Window">The source recorded window.</param>
/// <param name="RecordId">The source window identifier.</param>
/// <param name="SelectorName">The selector that matched the window.</param>
/// <param name="Side">The comparison side.</param>
/// <param name="Range">The normalized temporal range.</param>
/// <param name="Segments">The segment context preserved from the source window.</param>
public sealed record NormalizedWindowRecord(
    WindowRecord Window,
    WindowRecordId RecordId,
    string SelectorName,
    ComparisonSide Side,
    TemporalRange Range,
    IReadOnlyList<WindowSegment>? Segments = null)
{
    /// <summary>
    /// Gets the segment context preserved from the source window.
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
