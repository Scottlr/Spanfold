using System.Globalization;

using Spanfold;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonAligner
{
    internal static AlignedComparison Align(PreparedComparison prepared)
    {
        var segments = new List<AlignedSegment>();
        if (prepared.NormalizedWindows.Count == 0)
        {
            return new AlignedComparison(prepared, []);
        }

        var windows = new SortableNormalizedWindow[prepared.NormalizedWindows.Count];
        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var window = prepared.NormalizedWindows[i];
            windows[i] = new SortableNormalizedWindow(
                window,
                StableObjectValue(window.Window.Key),
                StableObjectValue(window.Window.Source),
                StableObjectValue(window.Window.Partition),
                StableSegments(window.Segments),
                new SegmentContext(window.Segments));
        }

        Array.Sort(windows, static (left, right) => Compare(left, right));

        var groups = new Dictionary<AlignmentGroupKey, List<SortableNormalizedWindow>>(
            new AlignmentGroupKeyComparer(prepared.KeyComparers));
        for (var i = 0; i < windows.Length; i++)
        {
            var window = windows[i];
            var key = new AlignmentGroupKey(
                window.Window.Window.WindowName,
                window.Window.Window.Key,
                window.Window.Window.Partition,
                window.SegmentContext);
            if (!groups.TryGetValue(key, out var group))
            {
                group = [];
                groups.Add(key, group);
            }

            group.Add(window);
        }

        var orderedGroups = groups.Values.Select(static group => group.ToArray()).ToArray();
        Array.Sort(orderedGroups, static (left, right) => Compare(left[0], right[0]));
        for (var i = 0; i < orderedGroups.Length; i++)
        {
            var group = orderedGroups[i];
            AddSegments(CreateScope(group[0]), group, 0, group.Length, segments);
        }

        return new AlignedComparison(prepared, segments.ToArray());
    }

    private static void AddSegments(
        AlignmentScope scope,
        SortableNormalizedWindow[] windows,
        int startIndex,
        int count,
        List<AlignedSegment> segments)
    {
        var boundaries = new List<TemporalPoint>(count * 2);
        var starts = new List<int>(count);
        var ends = new List<int>(count);
        for (var i = 0; i < count; i++)
        {
            var range = windows[startIndex + i].Window.Range;
            if (!range.HasEnd)
            {
                continue;
            }

            boundaries.Add(range.Start);
            boundaries.Add(range.End!.Value);
            starts.Add(i);
            ends.Add(i);
        }

        starts.Sort((left, right) => CompareStarts(windows, startIndex, left, right));
        ends.Sort((left, right) => CompareEnds(windows, startIndex, left, right));

        boundaries.Sort(static (left, right) => left.CompareTo(right));

        var unique = new List<TemporalPoint>(boundaries.Count);
        for (var i = 0; i < boundaries.Count; i++)
        {
            if (unique.Count == 0 || boundaries[i].CompareTo(unique[^1]) != 0)
            {
                unique.Add(boundaries[i]);
            }
        }

        var activeTarget = new SortedSet<int>();
        var activeAgainst = new SortedSet<int>();
        var startIndexCursor = 0;
        var endIndexCursor = 0;

        for (var i = 0; i < unique.Count - 1; i++)
        {
            var start = unique[i];
            var end = unique[i + 1];
            if (start.CompareTo(end) >= 0)
            {
                continue;
            }

            while (endIndexCursor < ends.Count
                && windows[startIndex + ends[endIndexCursor]].Window.Range.End!.Value.CompareTo(start) <= 0)
            {
                var index = ends[endIndexCursor++];
                activeTarget.Remove(index);
                activeAgainst.Remove(index);
            }

            while (startIndexCursor < starts.Count
                && windows[startIndex + starts[startIndexCursor]].Window.Range.Start.CompareTo(start) <= 0)
            {
                var index = starts[startIndexCursor++];
                var window = windows[startIndex + index].Window;
                if (window.Range.End!.Value.CompareTo(start) <= 0)
                {
                    continue;
                }

                if (window.Side == ComparisonSide.Target)
                {
                    activeTarget.Add(index);
                }
                else
                {
                    activeAgainst.Add(index);
                }
            }

            if (activeTarget.Count == 0 && activeAgainst.Count == 0)
            {
                continue;
            }

            var targetIds = activeTarget
                .Select(index => windows[startIndex + index].Window.RecordId)
                .ToArray();
            var againstIds = activeAgainst
                .Select(index => windows[startIndex + index].Window.RecordId)
                .ToArray();

            segments.Add(new AlignedSegment(
                scope.WindowName,
                scope.Key,
                scope.Partition,
                TemporalRange.Closed(start, end),
                targetIds.ToArray(),
                againstIds.ToArray(),
                scope.Segments));
        }
    }

    private static int CompareStarts(
        SortableNormalizedWindow[] windows,
        int startIndex,
        int left,
        int right)
    {
        var result = windows[startIndex + left].Window.Range.Start.CompareTo(
            windows[startIndex + right].Window.Range.Start);
        return result != 0 ? result : left.CompareTo(right);
    }

    private static int CompareEnds(
        SortableNormalizedWindow[] windows,
        int startIndex,
        int left,
        int right)
    {
        var result = windows[startIndex + left].Window.Range.End!.Value.CompareTo(
            windows[startIndex + right].Window.Range.End!.Value);
        return result != 0 ? result : left.CompareTo(right);
    }

    private static bool Covers(TemporalRange range, TemporalPoint start, TemporalPoint end)
    {
        return range.HasEnd
            && range.Start.CompareTo(start) <= 0
            && end.CompareTo(range.End!.Value) <= 0;
    }

    private static string StableObjectValue(object? value)
    {
        return CanonicalValueFormatter.Format(value);
    }

    private static int Compare(SortableNormalizedWindow left, SortableNormalizedWindow right)
    {
        var result = string.Compare(left.Window.Window.WindowName, right.Window.Window.WindowName, StringComparison.Ordinal);
        if (result != 0)
        {
            return result;
        }

        result = string.Compare(left.KeySort, right.KeySort, StringComparison.Ordinal);
        if (result != 0)
        {
            return result;
        }

        result = string.Compare(left.PartitionSort, right.PartitionSort, StringComparison.Ordinal);
        if (result != 0)
        {
            return result;
        }

        result = string.Compare(left.SegmentSort, right.SegmentSort, StringComparison.Ordinal);
        if (result != 0)
        {
            return result;
        }

        result = string.Compare(left.SourceSort, right.SourceSort, StringComparison.Ordinal);
        if (result != 0)
        {
            return result;
        }

        result = left.Window.Window.StartPosition.CompareTo(right.Window.Window.StartPosition);
        if (result != 0)
        {
            return result;
        }

        result = (left.Window.Window.EndPosition ?? long.MaxValue).CompareTo(right.Window.Window.EndPosition ?? long.MaxValue);
        if (result != 0)
        {
            return result;
        }

        result = left.Window.Side.CompareTo(right.Window.Side);
        if (result != 0)
        {
            return result;
        }

        return string.Compare(left.Window.SelectorName, right.Window.SelectorName, StringComparison.Ordinal);
    }

    private static AlignmentScope CreateScope(SortableNormalizedWindow window)
    {
        return new AlignmentScope(
            window.Window.Window.WindowName,
            window.Window.Window.Key,
            window.Window.Window.Partition,
            window.Window.Segments);
    }

    private static string StableSegments(IReadOnlyList<WindowSegment> segments)
    {
        if (segments.Count == 0)
        {
            return string.Empty;
        }

        var builder = new System.Text.StringBuilder();
        for (var i = 0; i < segments.Count; i++)
        {
            var segment = segments[i];
            builder
                .Append(segment.ParentName ?? string.Empty)
                .Append('/')
                .Append(segment.Name)
                .Append('=')
                .Append(StableObjectValue(segment.Value))
                .Append(';');
        }

        return builder.ToString();
    }

    private sealed record AlignmentScope(
        string WindowName,
        object Key,
        object? Partition,
        IReadOnlyList<WindowSegment> Segments);

    private readonly record struct AlignmentGroupKey(
        string WindowName,
        object Key,
        object? Partition,
        SegmentContext Segments);

    private sealed class AlignmentGroupKeyComparer(
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
        : IEqualityComparer<AlignmentGroupKey>
    {
        public bool Equals(AlignmentGroupKey x, AlignmentGroupKey y)
        {
            if (!string.Equals(x.WindowName, y.WindowName, StringComparison.Ordinal))
            {
                return false;
            }

            var comparer = keyComparers.TryGetValue(x.WindowName, out var configured)
                ? configured
                : EqualityComparer<object>.Default;
            return comparer.Equals(x.Key, y.Key)
                && EqualityComparer<object?>.Default.Equals(x.Partition, y.Partition)
                && x.Segments.Equals(y.Segments);
        }

        public int GetHashCode(AlignmentGroupKey obj)
        {
            var comparer = keyComparers.TryGetValue(obj.WindowName, out var configured)
                ? configured
                : EqualityComparer<object>.Default;
            return HashCode.Combine(
                StringComparer.Ordinal.GetHashCode(obj.WindowName),
                comparer.GetHashCode(obj.Key),
                obj.Partition,
                obj.Segments);
        }
    }

    private readonly record struct SortableNormalizedWindow(
        NormalizedWindowRecord Window,
        string KeySort,
        string SourceSort,
        string PartitionSort,
        string SegmentSort,
        SegmentContext SegmentContext);
}
