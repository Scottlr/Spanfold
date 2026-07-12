namespace Spanfold;

/// <summary>
/// Describes the common shape of an open or closed span.
/// </summary>
public abstract record WindowRecord
{
    /// <summary>Initializes a validated recorded window.</summary>
    protected WindowRecord(
        string windowName,
        object key,
        long startPosition,
        long? endPosition,
        object? source = null,
        object? partition = null,
        DateTimeOffset? startTime = null,
        DateTimeOffset? endTime = null,
        IReadOnlyList<WindowSegment>? segments = null,
        IReadOnlyList<WindowTag>? tags = null,
        WindowBoundaryReason? boundaryReason = null,
        IReadOnlyList<WindowBoundaryChange>? boundaryChanges = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);
        ArgumentNullException.ThrowIfNull(key);
        if (startPosition < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(startPosition));
        }

        if (endPosition.HasValue && endPosition.Value < startPosition)
        {
            throw new ArgumentException("End position must be greater than or equal to start position.", nameof(endPosition));
        }

        if (endTime.HasValue && !startTime.HasValue)
        {
            throw new ArgumentException(
                "An end timestamp requires a start timestamp.",
                nameof(endTime));
        }

        if (!endPosition.HasValue && endTime.HasValue)
        {
            throw new ArgumentException("Open windows cannot have an end timestamp.", nameof(endTime));
        }

        if (startTime.HasValue && endTime.HasValue && endTime.Value < startTime.Value)
        {
            throw new ArgumentException("End timestamp must not precede start timestamp.", nameof(endTime));
        }

        WindowName = windowName;
        Key = key;
        StartPosition = startPosition;
        EndPosition = endPosition;
        Source = source;
        Partition = partition;
        StartTime = startTime;
        EndTime = endTime;
        Segments = Materialize(segments);
        Tags = Materialize(tags);
        BoundaryReason = boundaryReason;
        BoundaryChanges = Materialize(boundaryChanges);
    }

    /// <summary>Gets the configured window name.</summary>
    public string WindowName { get; }
    /// <summary>Gets the logical key.</summary>
    public object Key { get; }
    /// <summary>Gets the processing position where the window started.</summary>
    public long StartPosition { get; }
    /// <summary>Gets the processing position where the window ended.</summary>
    public long? EndPosition { get; }
    /// <summary>Gets the optional source identity.</summary>
    public object? Source { get; }
    /// <summary>Gets the optional partition identity.</summary>
    public object? Partition { get; }
    /// <summary>Gets the optional opening timestamp.</summary>
    public DateTimeOffset? StartTime { get; }
    /// <summary>Gets the optional closing timestamp.</summary>
    public DateTimeOffset? EndTime { get; }

    private WindowRecordId? cachedId;
    /// <summary>
    /// Gets analytical segment values attached to this window.
    /// </summary>
    public IReadOnlyList<WindowSegment> Segments { get; }

    /// <summary>
    /// Gets descriptive non-boundary metadata attached to this window.
    /// </summary>
    public IReadOnlyList<WindowTag> Tags { get; }

    /// <summary>
    /// Gets the reason this window closed, when known.
    /// </summary>
    public WindowBoundaryReason? BoundaryReason { get; }

    /// <summary>
    /// Gets the segment changes that caused this window to close.
    /// </summary>
    public IReadOnlyList<WindowBoundaryChange> BoundaryChanges { get; }

    /// <summary>
    /// Gets the deterministic identity for this recorded window.
    /// </summary>
    /// <remarks>
    /// The identity is stable for the same recorded window data in a
    /// deterministic replay. It is not a distributed global identifier.
    /// </remarks>
    public WindowRecordId Id => this.cachedId ??= WindowRecordId.From(this);

    /// <summary>
    /// Gets whether this window has an end position.
    /// </summary>
    public bool IsClosed => EndPosition.HasValue;

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
