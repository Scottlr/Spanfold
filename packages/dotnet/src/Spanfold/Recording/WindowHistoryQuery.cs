using System.Globalization;
using Spanfold.Internal.Keys;

namespace Spanfold;

/// <summary>
/// Builds a read-only query over recorded window history.
/// </summary>
/// <remarks>
/// Queries materialize deterministic snapshots of the current recorded history.
/// They do not mutate open runtime state and do not run comparison alignment or
/// comparators. Use comparison plans when the question is about overlap,
/// residual, missing, coverage, or another cross-source row family.
/// </remarks>
public sealed class WindowHistoryQuery
{
    private readonly WindowHistory history;
    private readonly IReadOnlyList<WindowRecord>? windows;
    private readonly List<ValueFilter> segmentFilters = [];
    private readonly List<ValueFilter> tagFilters = [];
    private string? windowName;
    private object? key;
    private object? source;
    private object? partition;
    private bool hasKey;
    private bool hasSource;
    private bool hasPartition;

    internal WindowHistoryQuery(WindowHistory history)
    {
        this.history = history;
    }

    internal WindowHistoryQuery(IEnumerable<WindowRecord> windows)
    {
        this.history = null!;
        this.windows = windows as WindowRecord[] ?? windows.ToArray();
    }

    /// <summary>
    /// Restricts the query to one configured window name.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <returns>The current query.</returns>
    public WindowHistoryQuery Window(string windowName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);

        this.windowName = windowName;
        return this;
    }

    /// <summary>
    /// Restricts the query to one logical window key.
    /// </summary>
    /// <param name="key">The logical window key.</param>
    /// <returns>The current query.</returns>
    public WindowHistoryQuery Key(object key)
    {
        ArgumentNullException.ThrowIfNull(key);

        this.key = key;
        this.hasKey = true;
        return this;
    }

    /// <summary>
    /// Restricts the query to one source identity.
    /// </summary>
    /// <param name="source">The source identity.</param>
    /// <returns>The current query.</returns>
    public WindowHistoryQuery Source(object source)
    {
        ArgumentNullException.ThrowIfNull(source);

        this.source = source;
        this.hasSource = true;
        return this;
    }

    /// <summary>
    /// Restricts the query to one lane identity.
    /// </summary>
    /// <remarks>
    /// Lane is a readability alias for source. Spanfold records this identity in
    /// <see cref="WindowRecord.Source" />.
    /// </remarks>
    /// <param name="lane">The lane identity.</param>
    /// <returns>The current query.</returns>
    public WindowHistoryQuery Lane(object lane)
    {
        return Source(lane);
    }

    /// <summary>
    /// Restricts the query to one partition identity.
    /// </summary>
    /// <param name="partition">The partition identity.</param>
    /// <returns>The current query.</returns>
    public WindowHistoryQuery Partition(object partition)
    {
        ArgumentNullException.ThrowIfNull(partition);

        this.partition = partition;
        this.hasPartition = true;
        return this;
    }

    /// <summary>
    /// Restricts the query to windows carrying a segment value.
    /// </summary>
    /// <param name="name">The segment dimension name.</param>
    /// <param name="value">The required segment value.</param>
    /// <returns>The current query.</returns>
    public WindowHistoryQuery Segment(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        this.segmentFilters.Add(new ValueFilter(name, value));
        return this;
    }

    /// <summary>
    /// Restricts the query to windows carrying a tag value.
    /// </summary>
    /// <param name="name">The tag name.</param>
    /// <param name="value">The required tag value.</param>
    /// <returns>The current query.</returns>
    public WindowHistoryQuery Tag(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        this.tagFilters.Add(new ValueFilter(name, value));
        return this;
    }

    /// <summary>
    /// Materializes matching windows, including closed and currently open windows.
    /// </summary>
    /// <returns>Matching windows in deterministic order.</returns>
    public IReadOnlyList<WindowRecord> Windows()
    {
        var windows = this.windows ?? this.history.Windows;
        var matches = new List<WindowRecord>(windows.Count);

        for (var i = 0; i < windows.Count; i++)
        {
            if (Matches(windows[i]))
            {
                matches.Add(windows[i]);
            }
        }

        Sort(matches);
        return matches.ToArray();
    }

    /// <summary>
    /// Materializes matching closed windows.
    /// </summary>
    /// <returns>Matching closed windows in deterministic order.</returns>
    public IReadOnlyList<ClosedWindow> ClosedWindows()
    {
        var windows = this.history.ClosedWindows;
        var matches = new List<ClosedWindow>(windows.Count);

        for (var i = 0; i < windows.Count; i++)
        {
            if (Matches(windows[i]))
            {
                matches.Add(windows[i]);
            }
        }

        Sort(matches);
        return matches.ToArray();
    }

    /// <summary>
    /// Materializes matching currently open windows.
    /// </summary>
    /// <returns>Matching open windows in deterministic order.</returns>
    public IReadOnlyList<OpenWindow> OpenWindows()
    {
        var windows = this.history.OpenWindows;
        var matches = new List<OpenWindow>(windows.Count);

        for (var i = 0; i < windows.Count; i++)
        {
            if (Matches(windows[i]))
            {
                matches.Add(windows[i]);
            }
        }

        Sort(matches);
        return matches.ToArray();
    }

    /// <summary>
    /// Gets the latest matching window by start position and effective end position.
    /// </summary>
    /// <returns>The latest matching window, or null when no window matches.</returns>
    public WindowRecord? LatestWindow()
    {
        var matches = Windows();
        return matches.Count == 0 ? null : matches[^1];
    }

    /// <summary>
    /// Materializes matching snapshot records at an explicit horizon.
    /// </summary>
    /// <param name="horizon">The horizon used to evaluate recorded history.</param>
    /// <returns>Matching snapshot records in deterministic order.</returns>
    public IReadOnlyList<WindowSnapshotRecord> WindowsAt(TemporalPoint horizon)
    {
        return CreateSnapshotQuery(horizon).Windows();
    }

    /// <summary>
    /// Materializes matching records whose source windows were no longer active at an explicit horizon.
    /// </summary>
    /// <param name="horizon">The horizon used to evaluate recorded history.</param>
    /// <returns>Matching final snapshot records in deterministic order.</returns>
    public IReadOnlyList<WindowSnapshotRecord> ClosedWindowsAt(TemporalPoint horizon)
    {
        return CreateSnapshotQuery(horizon).ClosedWindows();
    }

    /// <summary>
    /// Materializes matching records whose source windows were active at an explicit horizon.
    /// </summary>
    /// <param name="horizon">The horizon used to evaluate recorded history.</param>
    /// <returns>Matching provisional snapshot records in deterministic order.</returns>
    public IReadOnlyList<WindowSnapshotRecord> OpenWindowsAt(TemporalPoint horizon)
    {
        return CreateSnapshotQuery(horizon).OpenWindows();
    }

    /// <summary>
    /// Gets the latest matching snapshot record at an explicit horizon.
    /// </summary>
    /// <param name="horizon">The horizon used to evaluate recorded history.</param>
    /// <returns>The latest matching snapshot record, or null when no record matches.</returns>
    public WindowSnapshotRecord? LatestWindowAt(TemporalPoint horizon)
    {
        return CreateSnapshotQuery(horizon).LatestWindow();
    }

    private bool Matches(WindowRecord window)
    {
        if (this.windowName is not null
            && !string.Equals(window.WindowName, this.windowName, StringComparison.Ordinal))
        {
            return false;
        }

        if (this.hasKey && !EqualityComparer<object>.Default.Equals(window.Key, this.key))
        {
            return false;
        }

        if (this.hasSource && !EqualityComparer<object?>.Default.Equals(window.Source, this.source))
        {
            return false;
        }

        if (this.hasPartition && !EqualityComparer<object?>.Default.Equals(window.Partition, this.partition))
        {
            return false;
        }

        for (var i = 0; i < this.segmentFilters.Count; i++)
        {
            if (!HasSegment(window, this.segmentFilters[i]))
            {
                return false;
            }
        }

        for (var i = 0; i < this.tagFilters.Count; i++)
        {
            if (!HasTag(window, this.tagFilters[i]))
            {
                return false;
            }
        }

        return true;
    }

    private WindowSnapshotQuery CreateSnapshotQuery(TemporalPoint horizon)
    {
        if (this.windows is not null)
        {
            throw new InvalidOperationException("Horizon queries require a live WindowHistory.");
        }

        var query = this.history.SnapshotAt(horizon).Query();

        if (this.windowName is not null)
        {
            query.Window(this.windowName);
        }

        if (this.hasKey)
        {
            query.Key(this.key!);
        }

        if (this.hasSource)
        {
            query.Source(this.source!);
        }

        if (this.hasPartition)
        {
            query.Partition(this.partition!);
        }

        for (var i = 0; i < this.segmentFilters.Count; i++)
        {
            query.Segment(this.segmentFilters[i].Name, this.segmentFilters[i].Value);
        }

        for (var i = 0; i < this.tagFilters.Count; i++)
        {
            query.Tag(this.tagFilters[i].Name, this.tagFilters[i].Value);
        }

        return query;
    }

    private static bool HasSegment(WindowRecord window, ValueFilter filter)
    {
        for (var i = 0; i < window.Segments.Count; i++)
        {
            var segment = window.Segments[i];
            if (string.Equals(segment.Name, filter.Name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(segment.Value, filter.Value))
            {
                return true;
            }
        }

        return false;
    }

    private static bool HasTag(WindowRecord window, ValueFilter filter)
    {
        for (var i = 0; i < window.Tags.Count; i++)
        {
            var tag = window.Tags[i];
            if (string.Equals(tag.Name, filter.Name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(tag.Value, filter.Value))
            {
                return true;
            }
        }

        return false;
    }

    private static void Sort<TWindow>(List<TWindow> windows)
        where TWindow : WindowRecord
    {
        windows.Sort(CompareWindows);
    }

    internal static int CompareWindows(WindowRecord left, WindowRecord right)
    {
        var comparison = string.Compare(left.WindowName, right.WindowName, StringComparison.Ordinal);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.Compare(StableObjectValue(left.Key), StableObjectValue(right.Key), StringComparison.Ordinal);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.Compare(StableObjectValue(left.Source), StableObjectValue(right.Source), StringComparison.Ordinal);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.Compare(StableObjectValue(left.Partition), StableObjectValue(right.Partition), StringComparison.Ordinal);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.StartPosition.CompareTo(right.StartPosition);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = (left.EndPosition ?? long.MaxValue).CompareTo(right.EndPosition ?? long.MaxValue);
        if (comparison != 0)
        {
            return comparison;
        }

        return string.Compare(left.Id.Value, right.Id.Value, StringComparison.Ordinal);
    }

    private static string StableObjectValue(object? value)
    {
        return CanonicalValueFormatter.Format(value);
    }

    private readonly record struct ValueFilter(string Name, object? Value);
}
