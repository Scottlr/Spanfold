using System.Globalization;
using Spanfold.Internal.Keys;

namespace Spanfold;

/// <summary>
/// Provides small grouping and summarization helpers for recorded window history.
/// </summary>
public static class WindowSummaryExtensions
{
    /// <summary>
    /// Summarizes recorded windows by one segment dimension.
    /// </summary>
    /// <param name="windows">The windows to summarize.</param>
    /// <param name="name">The segment dimension name.</param>
    /// <returns>Summaries ordered by segment value.</returns>
    public static IReadOnlyList<WindowGroupSummary> SummarizeBySegment(
        this IEnumerable<WindowRecord> windows,
        string name)
    {
        return Summarize(
            windows,
            WindowGroupKind.Segment,
            name,
            static window => window.Segments,
            static window => window.IsClosed ? ComparisonFinality.Final : ComparisonFinality.Provisional,
            CreateRange);
    }

    /// <summary>
    /// Summarizes recorded windows by one tag.
    /// </summary>
    /// <param name="windows">The windows to summarize.</param>
    /// <param name="name">The tag name.</param>
    /// <returns>Summaries ordered by tag value.</returns>
    public static IReadOnlyList<WindowGroupSummary> SummarizeByTag(
        this IEnumerable<WindowRecord> windows,
        string name)
    {
        return Summarize(
            windows,
            WindowGroupKind.Tag,
            name,
            static window => window.Tags,
            static window => window.IsClosed ? ComparisonFinality.Final : ComparisonFinality.Provisional,
            CreateRange);
    }

    /// <summary>
    /// Summarizes snapshot records by one segment dimension.
    /// </summary>
    /// <param name="records">The snapshot records to summarize.</param>
    /// <param name="name">The segment dimension name.</param>
    /// <returns>Summaries ordered by segment value.</returns>
    public static IReadOnlyList<WindowGroupSummary> SummarizeBySegment(
        this IEnumerable<WindowSnapshotRecord> records,
        string name)
    {
        return Summarize(
            records,
            WindowGroupKind.Segment,
            name,
            static record => record.Window.Segments,
            static record => record.Finality,
            static record => record.Range);
    }

    /// <summary>
    /// Summarizes snapshot records by one tag.
    /// </summary>
    /// <param name="records">The snapshot records to summarize.</param>
    /// <param name="name">The tag name.</param>
    /// <returns>Summaries ordered by tag value.</returns>
    public static IReadOnlyList<WindowGroupSummary> SummarizeByTag(
        this IEnumerable<WindowSnapshotRecord> records,
        string name)
    {
        return Summarize(
            records,
            WindowGroupKind.Tag,
            name,
            static record => record.Window.Tags,
            static record => record.Finality,
            static record => record.Range);
    }

    private static IReadOnlyList<WindowGroupSummary> Summarize<TRecord, TValue>(
        IEnumerable<TRecord> records,
        WindowGroupKind groupKind,
        string name,
        Func<TRecord, IReadOnlyList<TValue>> values,
        Func<TRecord, ComparisonFinality> finality,
        Func<TRecord, TemporalRange?> range)
        where TValue : notnull
    {
        ArgumentNullException.ThrowIfNull(records);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        var groups = new Dictionary<ValueKey, SummaryAccumulator>();

        foreach (var record in records)
        {
            var matchingValues = ValuesForName(values(record), name);

            for (var i = 0; i < matchingValues.Count; i++)
            {
                var value = matchingValues[i];
                var key = new ValueKey(value);
                if (!groups.TryGetValue(key, out var accumulator))
                {
                    accumulator = new SummaryAccumulator(groupKind, name, value);
                    groups.Add(key, accumulator);
                }

                accumulator.Add(finality(record), range(record));
            }
        }

        var summaries = groups.Values
            .Select(static accumulator => accumulator.ToSummary())
            .ToList();

        summaries.Sort(static (left, right) => string.Compare(
            StableObjectValue(left.Value),
            StableObjectValue(right.Value),
            StringComparison.Ordinal));

        return summaries.ToArray();
    }

    private static TemporalRange? CreateRange(WindowRecord window)
    {
        if (!window.EndPosition.HasValue)
        {
            return null;
        }

        return TemporalRange.Closed(
            TemporalPoint.ForPosition(window.StartPosition),
            TemporalPoint.ForPosition(window.EndPosition.Value));
    }

    private static IReadOnlyList<object?> ValuesForName<TValue>(
        IReadOnlyList<TValue> values,
        string name)
        where TValue : notnull
    {
        var matches = new List<object?>();

        for (var i = 0; i < values.Count; i++)
        {
            var value = values[i] switch
            {
                WindowSegment segment when string.Equals(segment.Name, name, StringComparison.Ordinal) => segment.Value,
                WindowTag tag when string.Equals(tag.Name, name, StringComparison.Ordinal) => tag.Value,
                _ => Unmatched.Value
            };

            if (!ReferenceEquals(value, Unmatched.Value) && !Contains(matches, value))
            {
                matches.Add(value);
            }
        }

        return matches;
    }

    private static bool Contains(List<object?> values, object? candidate)
    {
        for (var i = 0; i < values.Count; i++)
        {
            if (EqualityComparer<object?>.Default.Equals(values[i], candidate))
            {
                return true;
            }
        }

        return false;
    }

    private static string StableObjectValue(object? value)
    {
        return CanonicalValueFormatter.Format(value);
    }

    private sealed class SummaryAccumulator(
        WindowGroupKind groupKind,
        string name,
        object? value)
    {
        private int recordCount;
        private int finalCount;
        private int provisionalCount;
        private int measuredPositionCount;
        private long totalPositionLength;
        private int measuredTimeCount;
        private TimeSpan totalTimeDuration;

        public void Add(ComparisonFinality finality, TemporalRange? range)
        {
            this.recordCount++;

            if (finality == ComparisonFinality.Final)
            {
                this.finalCount++;
            }
            else
            {
                this.provisionalCount++;
            }

            if (range is not { HasEnd: true } measured)
            {
                return;
            }

            if (measured.Axis == TemporalAxis.ProcessingPosition)
            {
                this.measuredPositionCount++;
                this.totalPositionLength += measured.GetPositionLength();
            }
            else if (measured.Axis == TemporalAxis.Timestamp)
            {
                this.measuredTimeCount++;
                this.totalTimeDuration += measured.GetTimeDuration();
            }
        }

        public WindowGroupSummary ToSummary()
        {
            return new WindowGroupSummary(
                groupKind,
                name,
                value,
                this.recordCount,
                this.finalCount,
                this.provisionalCount,
                this.measuredPositionCount,
                this.totalPositionLength,
                this.measuredTimeCount,
                this.totalTimeDuration);
        }
    }

    private sealed class Unmatched
    {
        public static readonly Unmatched Value = new();
    }

    private readonly record struct ValueKey(object? Value);
}
