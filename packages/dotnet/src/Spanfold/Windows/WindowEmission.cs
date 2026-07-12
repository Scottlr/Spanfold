namespace Spanfold;

/// <summary>
/// Describes a window open or close transition produced by ingestion.
/// </summary>
/// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical key whose window changed state.</param>
/// <param name="Event">The event that produced the transition.</param>
/// <param name="Kind">The transition kind.</param>
/// <param name="Source">Optional source identity supplied during ingestion.</param>
/// <param name="Partition">Optional partition identity supplied during ingestion.</param>
/// <param name="Segments">Analytical segment values attached to the emitted window.</param>
/// <param name="Tags">Descriptive non-boundary metadata attached to the emitted window.</param>
/// <param name="BoundaryReason">The reason a closed boundary was emitted, when known.</param>
/// <param name="BoundaryChanges">The segment changes that caused the boundary.</param>
public sealed record WindowEmission<TEvent>(
    string WindowName,
    object Key,
    TEvent Event,
    WindowTransitionKind Kind,
    object? Source = null,
    object? Partition = null,
    IReadOnlyList<WindowSegment>? Segments = null,
    IReadOnlyList<WindowTag>? Tags = null,
    WindowBoundaryReason? BoundaryReason = null,
    IReadOnlyList<WindowBoundaryChange>? BoundaryChanges = null)
{
    /// <summary>
    /// Gets analytical segment values attached to the emitted window.
    /// </summary>
    public IReadOnlyList<WindowSegment> Segments { get; } = Materialize(Segments);

    /// <summary>
    /// Gets descriptive non-boundary metadata attached to the emitted window.
    /// </summary>
    public IReadOnlyList<WindowTag> Tags { get; } = Materialize(Tags);

    /// <summary>
    /// Gets the reason a closed boundary was emitted, when known.
    /// </summary>
    public WindowBoundaryReason? BoundaryReason { get; } = BoundaryReason;

    /// <summary>
    /// Gets the segment changes that caused the boundary.
    /// </summary>
    public IReadOnlyList<WindowBoundaryChange> BoundaryChanges { get; } = Materialize(BoundaryChanges);

    private static IReadOnlyList<T> Materialize<T>(IReadOnlyList<T>? values)
    {
        return values switch
        {
            null => [],
            _ => Array.AsReadOnly(values.ToArray())
        };
    }
}
