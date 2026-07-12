namespace Spanfold;

/// <summary>
/// Represents recorded window history evaluated at an explicit horizon.
/// </summary>
/// <remarks>
/// A snapshot is a read-only view. It does not mutate open windows or rewrite
/// closed windows. Windows active at the horizon are clipped to the horizon and
/// marked provisional.
/// </remarks>
public sealed class WindowHistorySnapshot
{
    internal WindowHistorySnapshot(
        TemporalPoint horizon,
        IReadOnlyList<WindowSnapshotRecord> records,
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        Horizon = horizon;
        Records = Array.AsReadOnly(records.ToArray());
        KeyComparers = new Dictionary<string, IEqualityComparer<object>>(keyComparers, StringComparer.Ordinal);
    }

    /// <summary>
    /// Gets the horizon used to evaluate the recorded windows.
    /// </summary>
    public TemporalPoint Horizon { get; }

    /// <summary>
    /// Gets the snapshot records in deterministic order.
    /// </summary>
    public IReadOnlyList<WindowSnapshotRecord> Records { get; }

    internal IReadOnlyDictionary<string, IEqualityComparer<object>> KeyComparers { get; }

    /// <summary>
    /// Starts a read-only query over the snapshot records.
    /// </summary>
    /// <returns>A snapshot query builder.</returns>
    public WindowSnapshotQuery Query()
    {
        return new WindowSnapshotQuery(this);
    }

    internal static WindowHistorySnapshot Create(
        WindowHistory history,
        TemporalPoint horizon)
    {
        if (horizon.Axis == TemporalAxis.Unknown)
        {
            throw new ArgumentException("Snapshot horizon must use a known temporal axis.", nameof(horizon));
        }

        var windows = history.Windows;
        var records = new List<WindowSnapshotRecord>(windows.Count);

        for (var i = 0; i < windows.Count; i++)
        {
            if (TryCreateRecord(windows[i], horizon, out var record))
            {
                records.Add(record);
            }
        }

        records.Sort(static (left, right) => WindowHistoryQuery.CompareWindows(left.Window, right.Window));
        return new WindowHistorySnapshot(horizon, records.ToArray(), history.KeyComparers);
    }

    private static bool TryCreateRecord(
        WindowRecord window,
        TemporalPoint horizon,
        out WindowSnapshotRecord record)
    {
        record = default!;

        if (!TryGetStart(window, horizon.Axis, out var start))
        {
            return false;
        }

        if (start.CompareTo(horizon) > 0)
        {
            return false;
        }

        if (TryGetEnd(window, horizon.Axis, out var end)
            && end.CompareTo(horizon) <= 0)
        {
            record = new WindowSnapshotRecord(
                window,
                TemporalRange.Closed(start, end),
                ComparisonFinality.Final);
            return true;
        }

        record = new WindowSnapshotRecord(
            window,
            TemporalRange.WithEffectiveEnd(start, horizon, TemporalRangeEndStatus.OpenAtHorizon),
            ComparisonFinality.Provisional);
        return true;
    }

    private static bool TryGetStart(
        WindowRecord window,
        TemporalAxis axis,
        out TemporalPoint start)
    {
        if (axis == TemporalAxis.ProcessingPosition)
        {
            start = TemporalPoint.ForPosition(window.StartPosition);
            return true;
        }

        if (axis == TemporalAxis.Timestamp && window.StartTime.HasValue)
        {
            start = TemporalPoint.ForTimestamp(window.StartTime.Value, window.TimestampClock);
            return true;
        }

        start = default;
        return false;
    }

    private static bool TryGetEnd(
        WindowRecord window,
        TemporalAxis axis,
        out TemporalPoint end)
    {
        if (axis == TemporalAxis.ProcessingPosition && window.EndPosition.HasValue)
        {
            end = TemporalPoint.ForPosition(window.EndPosition.Value);
            return true;
        }

        if (axis == TemporalAxis.Timestamp && window.EndTime.HasValue)
        {
            end = TemporalPoint.ForTimestamp(window.EndTime.Value, window.TimestampClock);
            return true;
        }

        end = default;
        return false;
    }
}
