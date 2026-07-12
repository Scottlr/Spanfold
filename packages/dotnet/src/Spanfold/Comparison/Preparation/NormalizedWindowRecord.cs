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
public sealed record NormalizedWindowRecord
{
    internal NormalizedWindowRecord(
        WindowRecord window,
        WindowRecordId recordId,
        string selectorName,
        ComparisonSide side,
        TemporalRange range,
        IReadOnlyList<WindowSegment>? segments = null)
    {
        Window = window;
        RecordId = recordId;
        SelectorName = selectorName;
        Side = side;
        Range = range;
        Segments = Materialize(segments);
    }

    /// <summary>Gets the source recorded window.</summary>
    public WindowRecord Window { get; }
    /// <summary>Gets the source window identifier.</summary>
    public WindowRecordId RecordId { get; }
    /// <summary>Gets the selector that matched the window.</summary>
    public string SelectorName { get; }
    /// <summary>Gets the comparison side.</summary>
    public ComparisonSide Side { get; }
    /// <summary>Gets the normalized temporal range.</summary>
    public TemporalRange Range { get; }

    /// <summary>
    /// Gets the segment context preserved from the source window.
    /// </summary>
    public IReadOnlyList<WindowSegment> Segments { get; }

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
