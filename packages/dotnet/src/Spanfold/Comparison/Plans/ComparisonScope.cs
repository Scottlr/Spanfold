namespace Spanfold;

/// <summary>
/// Describes the temporal scope for a comparison plan.
/// </summary>
/// <remarks>
/// Scope selects the window family and temporal axis for the question. The
/// normalization policy must use the same axis before preparation can produce
/// comparable ranges.
/// </remarks>
/// <param name="WindowName">Optional window name restriction.</param>
/// <param name="TimeAxis">The temporal axis used by the comparison.</param>
/// <param name="SegmentFilters">The required segment values.</param>
/// <param name="TagFilters">The required tag values.</param>
public sealed record ComparisonScope(
    string? WindowName,
    TemporalAxis TimeAxis,
    IReadOnlyList<WindowSegmentFilter>? SegmentFilters = null,
    IReadOnlyList<WindowTagFilter>? TagFilters = null)
{
    /// <summary>
    /// Gets the required segment values for this scope.
    /// </summary>
    public IReadOnlyList<WindowSegmentFilter> SegmentFilters { get; } = Materialize(SegmentFilters);

    /// <summary>
    /// Gets the required tag values for this scope.
    /// </summary>
    public IReadOnlyList<WindowTagFilter> TagFilters { get; } = Materialize(TagFilters);

    /// <summary>
    /// Creates an unrestricted scope on the processing-position axis.
    /// </summary>
    /// <returns>A comparison scope for all recorded windows.</returns>
    public static ComparisonScope All()
    {
        return new ComparisonScope(WindowName: null, TemporalAxis.ProcessingPosition);
    }

    /// <summary>
    /// Creates a scope restricted to one window name.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <param name="timeAxis">The temporal axis used by the comparison.</param>
    /// <returns>A comparison scope for one window.</returns>
    public static ComparisonScope Window(string windowName, TemporalAxis timeAxis = TemporalAxis.ProcessingPosition)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);

        return new ComparisonScope(windowName, timeAxis);
    }

    /// <summary>
    /// Adds a required segment value to the scope.
    /// </summary>
    /// <param name="name">The segment dimension name.</param>
    /// <param name="value">The required segment value.</param>
    /// <returns>A new scope with the segment filter appended.</returns>
    public ComparisonScope Segment(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        var filters = new WindowSegmentFilter[SegmentFilters.Count + 1];
        for (var i = 0; i < SegmentFilters.Count; i++)
        {
            filters[i] = SegmentFilters[i];
        }

        filters[^1] = new WindowSegmentFilter(name, value);
        return new ComparisonScope(WindowName, TimeAxis, filters, TagFilters);
    }

    /// <summary>
    /// Adds a required tag value to the scope.
    /// </summary>
    /// <param name="name">The tag name.</param>
    /// <param name="value">The required tag value.</param>
    /// <returns>A new scope with the tag filter appended.</returns>
    public ComparisonScope Tag(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        var filters = new WindowTagFilter[TagFilters.Count + 1];
        for (var i = 0; i < TagFilters.Count; i++)
        {
            filters[i] = TagFilters[i];
        }

        filters[^1] = new WindowTagFilter(name, value);
        return new ComparisonScope(WindowName, TimeAxis, SegmentFilters, filters);
    }

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
