namespace Spanfold;

/// <summary>
/// Builds a read-only query over window snapshot records.
/// </summary>
public sealed class WindowSnapshotQuery
{
    private readonly WindowHistorySnapshot snapshot;
    private readonly WindowHistoryQuery query;
    private readonly Dictionary<WindowRecordId, WindowSnapshotRecord> recordsById;

    internal WindowSnapshotQuery(WindowHistorySnapshot snapshot)
    {
        this.snapshot = snapshot;
        this.query = new WindowHistoryQuery(snapshot.Records.Select(static record => record.Window));
        this.recordsById = new Dictionary<WindowRecordId, WindowSnapshotRecord>(snapshot.Records.Count);

        for (var i = 0; i < snapshot.Records.Count; i++)
        {
            var record = snapshot.Records[i];
            if (!this.recordsById.TryAdd(record.Window.Id, record))
            {
                throw new InvalidOperationException(
                    $"Window record identity collision detected for '{record.Window.Id}'.");
            }
        }
    }

    /// <summary>
    /// Restricts the query to one configured window name.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <returns>The current query.</returns>
    public WindowSnapshotQuery Window(string windowName)
    {
        this.query.Window(windowName);
        return this;
    }

    /// <summary>
    /// Restricts the query to one logical window key.
    /// </summary>
    /// <param name="key">The logical window key.</param>
    /// <returns>The current query.</returns>
    public WindowSnapshotQuery Key(object key)
    {
        this.query.Key(key);
        return this;
    }

    /// <summary>
    /// Restricts the query to one source identity.
    /// </summary>
    /// <param name="source">The source identity.</param>
    /// <returns>The current query.</returns>
    public WindowSnapshotQuery Source(object source)
    {
        this.query.Source(source);
        return this;
    }

    /// <summary>
    /// Restricts the query to one lane identity.
    /// </summary>
    /// <param name="lane">The lane identity.</param>
    /// <returns>The current query.</returns>
    public WindowSnapshotQuery Lane(object lane)
    {
        this.query.Lane(lane);
        return this;
    }

    /// <summary>
    /// Restricts the query to one partition identity.
    /// </summary>
    /// <param name="partition">The partition identity.</param>
    /// <returns>The current query.</returns>
    public WindowSnapshotQuery Partition(object partition)
    {
        this.query.Partition(partition);
        return this;
    }

    /// <summary>
    /// Restricts the query to windows carrying a segment value.
    /// </summary>
    /// <param name="name">The segment dimension name.</param>
    /// <param name="value">The required segment value.</param>
    /// <returns>The current query.</returns>
    public WindowSnapshotQuery Segment(string name, object? value)
    {
        this.query.Segment(name, value);
        return this;
    }

    /// <summary>
    /// Restricts the query to windows carrying a tag value.
    /// </summary>
    /// <param name="name">The tag name.</param>
    /// <param name="value">The required tag value.</param>
    /// <returns>The current query.</returns>
    public WindowSnapshotQuery Tag(string name, object? value)
    {
        this.query.Tag(name, value);
        return this;
    }

    /// <summary>
    /// Materializes all matching snapshot records.
    /// </summary>
    /// <returns>Matching snapshot records in deterministic order.</returns>
    public IReadOnlyList<WindowSnapshotRecord> Windows()
    {
        return Matching(static _ => true);
    }

    /// <summary>
    /// Materializes matching records whose source windows were no longer active at the snapshot horizon.
    /// </summary>
    /// <returns>Matching final snapshot records in deterministic order.</returns>
    public IReadOnlyList<WindowSnapshotRecord> ClosedWindows()
    {
        return Matching(static record => record.Finality == ComparisonFinality.Final);
    }

    /// <summary>
    /// Materializes matching records whose source windows were active at the snapshot horizon.
    /// </summary>
    /// <returns>Matching provisional snapshot records in deterministic order.</returns>
    public IReadOnlyList<WindowSnapshotRecord> OpenWindows()
    {
        return Matching(static record => record.Finality == ComparisonFinality.Provisional);
    }

    /// <summary>
    /// Gets the latest matching snapshot record.
    /// </summary>
    /// <returns>The latest matching snapshot record, or null when no record matches.</returns>
    public WindowSnapshotRecord? LatestWindow()
    {
        var windows = Windows();
        return windows.Count == 0 ? null : windows[^1];
    }

    private IReadOnlyList<WindowSnapshotRecord> Matching(Func<WindowSnapshotRecord, bool> predicate)
    {
        var matchingWindows = this.query.Windows();
        var matches = new List<WindowSnapshotRecord>(matchingWindows.Count);

        for (var i = 0; i < matchingWindows.Count; i++)
        {
            var window = matchingWindows[i];
            if (this.recordsById.TryGetValue(window.Id, out var record) && predicate(record))
            {
                matches.Add(record);
            }
        }

        return matches.ToArray();
    }
}
