using Spanfold.Internal.Recording;
using Spanfold.Internal.Keys;

namespace Spanfold;

/// <summary>
/// Stores recorded open and closed windows and exposes window queries.
/// </summary>
/// <remarks>
/// History is append-oriented from pipeline ingestion. Query and comparison
/// APIs return materialized snapshots so callers can inspect the current
/// recorded state without mutating the active runtime.
/// </remarks>
public sealed class WindowHistory
{
    private readonly bool enabled;
    private readonly IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers;
    private readonly Dictionary<WindowRecordingKey, OpenWindow> openWindows;
    private readonly List<ClosedWindow> closedWindows;
    private readonly List<WindowAnnotation> annotations;

    internal IReadOnlyDictionary<string, IEqualityComparer<object>> KeyComparers => this.keyComparers;

    internal IEqualityComparer<object> GetKeyComparer(string windowName)
    {
        return this.keyComparers.TryGetValue(windowName, out var comparer)
            ? comparer
            : EqualityComparer<object>.Default;
    }

    internal WindowHistory(bool enabled)
        : this(enabled, new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal))
    {
    }

    internal WindowHistory(
        bool enabled,
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        this.enabled = enabled;
        this.keyComparers = keyComparers;
        this.openWindows = [];
        this.closedWindows = [];
        this.annotations = [];
    }

    /// <summary>Creates an immutable history snapshot from materialized window records.</summary>
    /// <remarks>
    /// This import boundary is intended for persisted history, tools, and test
    /// fixtures. Normal applications should prefer pipeline-owned recording.
    /// </remarks>
    public static WindowHistory FromRecords(
        IEnumerable<ClosedWindow> closedWindows,
        IEnumerable<OpenWindow> openWindows)
    {
        return FromRecords(
            closedWindows,
            openWindows,
            new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal));
    }

    /// <summary>Creates an immutable history snapshot from materialized window records.</summary>
    /// <remarks>
    /// This import boundary is intended for persisted history, tools, and test
    /// fixtures. The comparer map restores the configured logical key identity
    /// for each imported window family.
    /// </remarks>
    /// <param name="closedWindows">The materialized closed windows.</param>
    /// <param name="openWindows">The materialized open windows.</param>
    /// <param name="keyComparers">The logical key comparer for each configured window name.</param>
    /// <returns>An imported history using the supplied key identity policy.</returns>
    public static WindowHistory FromRecords(
        IEnumerable<ClosedWindow> closedWindows,
        IEnumerable<OpenWindow> openWindows,
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        ArgumentNullException.ThrowIfNull(closedWindows);
        ArgumentNullException.ThrowIfNull(openWindows);
        ArgumentNullException.ThrowIfNull(keyComparers);

        var comparerCopy = new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal);
        foreach (var pair in keyComparers)
        {
            ArgumentException.ThrowIfNullOrWhiteSpace(pair.Key);
            ArgumentNullException.ThrowIfNull(pair.Value);
            comparerCopy.Add(pair.Key, pair.Value);
        }

        var history = new WindowHistory(enabled: true, comparerCopy);
        history.closedWindows.AddRange(closedWindows);
        foreach (var window in openWindows)
        {
            history.openWindows.Add(
                new WindowRecordingKey(
                    window.WindowName,
                    window.Key,
                    window.Source,
                    window.Partition,
                    new SegmentContext(window.Segments)),
                window);
        }

        return history;
    }

    /// <summary>
    /// Gets closed windows recorded by the pipeline.
    /// </summary>
    public IReadOnlyList<ClosedWindow> ClosedWindows => Array.AsReadOnly(this.closedWindows.ToArray());

    /// <summary>
    /// Gets all recorded windows, including closed windows and currently open windows.
    /// </summary>
    public IReadOnlyList<WindowRecord> Windows
    {
        get
        {
            var windows = new WindowRecord[this.closedWindows.Count + this.openWindows.Count];
            var index = 0;

            foreach (var window in this.closedWindows)
            {
                windows[index] = window;
                index++;
            }

            foreach (var window in this.openWindows.Values)
            {
                windows[index] = window;
                index++;
            }

            return Array.AsReadOnly(windows);
        }
    }

    /// <summary>
    /// Gets currently open windows recorded by the pipeline.
    /// </summary>
    public IReadOnlyList<OpenWindow> OpenWindows
    {
        get
        {
            var windows = new OpenWindow[this.openWindows.Count];
            var index = 0;

            foreach (var window in this.openWindows.Values)
            {
                windows[index] = window;
                index++;
            }

            return Array.AsReadOnly(windows);
        }
    }

    /// <summary>
    /// Gets annotations attached to recorded windows.
    /// </summary>
    public IReadOnlyList<WindowAnnotation> Annotations => Array.AsReadOnly(this.annotations.ToArray());

    /// <summary>
    /// Removes closed windows whose end position is at or before a retention boundary.
    /// </summary>
    /// <remarks>
    /// This is an explicit history-drain operation for long-running pipelines.
    /// Open windows and annotations are retained; callers should persist or
    /// export any required evidence before trimming.
    /// </remarks>
    /// <param name="endPositionExclusive">The exclusive processing-position retention boundary.</param>
    /// <returns>The number of closed windows removed.</returns>
    public int TrimClosedBefore(long endPositionExclusive)
    {
        if (endPositionExclusive < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(endPositionExclusive));
        }

        var removed = this.closedWindows.RemoveAll(
            window => window.EndPosition.HasValue
                && window.EndPosition.Value <= endPositionExclusive);
        return removed;
    }

    /// <summary>
    /// Starts a read-only query over recorded windows.
    /// </summary>
    /// <remarks>
    /// Use this API for direct state-history inspection when no cross-source
    /// comparator rows are needed.
    /// </remarks>
    /// <returns>A window history query builder.</returns>
    public WindowHistoryQuery Query()
    {
        return new WindowHistoryQuery(this);
    }

    /// <summary>
    /// Evaluates recorded windows at an explicit horizon.
    /// </summary>
    /// <remarks>
    /// Windows active at the horizon are clipped to the horizon and marked
    /// provisional in the returned snapshot. The underlying history is not
    /// mutated.
    /// </remarks>
    /// <param name="horizon">The horizon used to evaluate the history.</param>
    /// <returns>A read-only snapshot of the recorded history at the horizon.</returns>
    public WindowHistorySnapshot SnapshotAt(TemporalPoint horizon)
    {
        return WindowHistorySnapshot.Create(this, horizon);
    }

    /// <summary>
    /// Attaches external metadata to a recorded window.
    /// </summary>
    /// <remarks>
    /// Annotation is append-only. It does not mutate, split, or revise the
    /// source window. The annotation target uses the window start identity so
    /// metadata attached while a window is open remains associated after the
    /// window closes.
    /// </remarks>
    /// <param name="window">The window to annotate.</param>
    /// <param name="name">The annotation name.</param>
    /// <param name="value">The annotation value.</param>
    /// <param name="knownAt">When the annotation became known, if supplied.</param>
    /// <returns>The appended annotation.</returns>
    public WindowAnnotation Annotate(
        WindowRecord window,
        string name,
        object? value,
        TemporalPoint? knownAt = null)
    {
        ArgumentNullException.ThrowIfNull(window);

        return Annotate(WindowAnnotationTarget.From(window), name, value, knownAt);
    }

    /// <summary>
    /// Attaches external metadata to a window annotation target.
    /// </summary>
    /// <param name="target">The stable window start identity to annotate.</param>
    /// <param name="name">The annotation name.</param>
    /// <param name="value">The annotation value.</param>
    /// <param name="knownAt">When the annotation became known, if supplied.</param>
    /// <returns>The appended annotation.</returns>
    public WindowAnnotation Annotate(
        WindowAnnotationTarget target,
        string name,
        object? value,
        TemporalPoint? knownAt = null)
    {
        ArgumentNullException.ThrowIfNull(target);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        if (knownAt is { Axis: TemporalAxis.Unknown })
        {
            throw new ArgumentException("Annotation known-at point must use a known temporal axis.", nameof(knownAt));
        }

        var revision = 1;
        for (var i = 0; i < this.annotations.Count; i++)
        {
            var annotation = this.annotations[i];
            if (annotation.Target == target
                && string.Equals(annotation.Name, name, StringComparison.Ordinal))
            {
                revision++;
            }
        }

        var appended = new WindowAnnotation(target, name, value, knownAt, revision);
        this.annotations.Add(appended);
        return appended;
    }

    /// <summary>
    /// Gets annotations attached to a recorded window.
    /// </summary>
    /// <param name="window">The recorded window.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsFor(WindowRecord window)
    {
        ArgumentNullException.ThrowIfNull(window);

        return AnnotationsFor(WindowAnnotationTarget.From(window));
    }

    /// <summary>
    /// Gets annotations for a recorded window that were known at or before a horizon.
    /// </summary>
    /// <remarks>
    /// Annotations without a comparable known-at point are excluded from this
    /// point-in-time-safe view.
    /// </remarks>
    /// <param name="window">The recorded window.</param>
    /// <param name="horizon">The known-at horizon.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsKnownAt(
        WindowRecord window,
        TemporalPoint horizon)
    {
        ArgumentNullException.ThrowIfNull(window);

        return AnnotationsKnownAt(WindowAnnotationTarget.From(window), horizon);
    }

    /// <summary>
    /// Gets annotations attached to a window annotation target.
    /// </summary>
    /// <param name="target">The stable window start identity.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsFor(WindowAnnotationTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        var matches = new List<WindowAnnotation>();
        for (var i = 0; i < this.annotations.Count; i++)
        {
            if (this.annotations[i].Target == target)
            {
                matches.Add(this.annotations[i]);
            }
        }

        return matches.ToArray();
    }

    /// <summary>
    /// Gets annotations for a window target that were known at or before a horizon.
    /// </summary>
    /// <remarks>
    /// Annotations without a comparable known-at point are excluded from this
    /// point-in-time-safe view.
    /// </remarks>
    /// <param name="target">The stable window start identity.</param>
    /// <param name="horizon">The known-at horizon.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsKnownAt(
        WindowAnnotationTarget target,
        TemporalPoint horizon)
    {
        ArgumentNullException.ThrowIfNull(target);

        if (horizon.Axis == TemporalAxis.Unknown)
        {
            throw new ArgumentException("Annotation known-at horizon must use a known temporal axis.", nameof(horizon));
        }

        var matches = new List<WindowAnnotation>();
        for (var i = 0; i < this.annotations.Count; i++)
        {
            var annotation = this.annotations[i];
            if (annotation.Target == target && IsKnownAtOrBefore(annotation, horizon))
            {
                matches.Add(annotation);
            }
        }

        return matches.ToArray();
    }

    private static bool IsKnownAtOrBefore(
        WindowAnnotation annotation,
        TemporalPoint horizon)
    {
        return annotation.KnownAt is { } knownAt
            && knownAt.Axis == horizon.Axis
            && (knownAt.Axis != TemporalAxis.Timestamp
                || string.Equals(knownAt.Clock, horizon.Clock, StringComparison.Ordinal))
            && knownAt.CompareTo(horizon) <= 0;
    }

    /// <summary>
    /// Gets recorded windows for a configured window name.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <returns>Recorded windows with the supplied window name.</returns>
    public IReadOnlyList<WindowRecord> ForWindow(string windowName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);

        return Windows
            .Where(window => string.Equals(window.WindowName, windowName, StringComparison.Ordinal))
            .ToArray();
    }

    /// <summary>
    /// Gets recorded windows that contain a required segment value.
    /// </summary>
    /// <param name="name">The segment dimension name.</param>
    /// <param name="value">The required segment value.</param>
    /// <returns>Recorded windows that contain the required segment value.</returns>
    public IReadOnlyList<WindowRecord> WithSegment(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return Windows
            .Where(window => HasSegment(window, name, value))
            .ToArray();
    }

    /// <summary>
    /// Gets recorded windows that contain a required tag value.
    /// </summary>
    /// <param name="name">The tag name.</param>
    /// <param name="value">The required tag value.</param>
    /// <returns>Recorded windows that contain the required tag value.</returns>
    public IReadOnlyList<WindowRecord> WithTag(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return Windows
            .Where(window => HasTag(window, name, value))
            .ToArray();
    }

    /// <summary>
    /// Finds overlapping closed windows within the same window scope.
    /// </summary>
    /// <returns>The overlapping window pairs.</returns>
    public IReadOnlyList<WindowOverlap> FindOverlaps()
    {
        return WindowHistoryAnalytics.FindOverlaps(this.closedWindows, this.keyComparers);
    }

    /// <summary>
    /// Finds target-source window segments that are not covered by comparison sources.
    /// </summary>
    /// <param name="targetSource">The source whose unique residual segments should be returned.</param>
    /// <returns>The residual segments for the target source.</returns>
    public IReadOnlyList<WindowResidualSegment> FindResiduals(object targetSource)
    {
        ArgumentNullException.ThrowIfNull(targetSource);
        return WindowHistoryAnalytics.FindResiduals(this.closedWindows, targetSource, this.keyComparers);
    }

    internal void Record<TEvent>(
        IReadOnlyList<WindowEmission<TEvent>> emissions,
        long processingPosition,
        DateTimeOffset? eventTime,
        string? eventTimeClock = null)
    {
        if (!this.enabled)
        {
            return;
        }

        var undo = new List<Action>();
        try
        {
            foreach (var emission in emissions)
            {
                var key = new WindowRecordingKey(
                    emission.WindowName,
                    emission.Key,
                    emission.Source,
                    emission.Partition,
                    new SegmentContext(emission.Segments));

                if (emission.Kind == WindowTransitionKind.Opened)
                {
                    var opened = new OpenWindow(
                        emission.WindowName,
                        emission.Key,
                        processingPosition,
                        emission.Source,
                        emission.Partition,
                        eventTime,
                        emission.Segments,
                        emission.Tags,
                        eventTimeClock);
                    var existed = this.openWindows.TryGetValue(key, out var previous);
                    undo.Add(() =>
                    {
                        if (existed)
                        {
                            this.openWindows[key] = previous!;
                        }
                        else
                        {
                            this.openWindows.Remove(key);
                        }
                    });
                    this.openWindows[key] = opened;
                    continue;
                }

                if (!this.TryRemoveOpenWindow(key, out var removedKey, out var open))
                {
                    continue;
                }

                undo.Add(() => this.openWindows[removedKey] = open);
                var closed = new ClosedWindow(
                    open.WindowName,
                    open.Key,
                    open.StartPosition,
                    processingPosition,
                    open.Source,
                    open.Partition,
                    open.StartTime,
                    eventTime,
                    open.Segments,
                    open.Tags,
                    emission.BoundaryReason,
                    emission.BoundaryChanges,
                    open.TimestampClock);
                this.closedWindows.Add(closed);
                undo.Add(() => this.closedWindows.RemoveAt(this.closedWindows.Count - 1));
            }
        }
        catch
        {
            for (var i = undo.Count - 1; i >= 0; i--)
            {
                undo[i]();
            }

            throw;
        }
    }

    private bool TryRemoveOpenWindow(
        WindowRecordingKey key,
        out WindowRecordingKey removedKey,
        out OpenWindow open)
    {
        removedKey = key;
        open = null!;
        if (this.openWindows.Remove(key, out open!))
        {
            return true;
        }

        if (!this.keyComparers.TryGetValue(key.WindowName, out var comparer))
        {
            return false;
        }

        foreach (var candidate in this.openWindows.Keys)
        {
            if (!string.Equals(candidate.WindowName, key.WindowName, StringComparison.Ordinal)
                || !comparer.Equals(candidate.Key, key.Key)
                || !EqualityComparer<object?>.Default.Equals(candidate.Source, key.Source)
                || !EqualityComparer<object?>.Default.Equals(candidate.Partition, key.Partition)
                || (candidate.SegmentContext is not null
                    && key.SegmentContext is not null
                    && !Equals(candidate.SegmentContext, key.SegmentContext)))
            {
                continue;
            }

            removedKey = candidate;
            return this.openWindows.Remove(candidate, out open!);
        }

        return false;
    }

    private bool HasWindowForSource(string windowName, object source)
    {
        foreach (var window in Windows)
        {
            if (string.Equals(window.WindowName, windowName, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(window.Source, source))
            {
                return true;
            }
        }

        return false;
    }

    private static bool HasSegment(WindowRecord window, string name, object? value)
    {
        for (var i = 0; i < window.Segments.Count; i++)
        {
            var segment = window.Segments[i];
            if (string.Equals(segment.Name, name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(segment.Value, value))
            {
                return true;
            }
        }

        return false;
    }

    private static bool HasTag(WindowRecord window, string name, object? value)
    {
        for (var i = 0; i < window.Tags.Count; i++)
        {
            var tag = window.Tags[i];
            if (string.Equals(tag.Name, name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(tag.Value, value))
            {
                return true;
            }
        }

        return false;
    }

}
