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

        var groupStart = 0;
        for (var i = 1; i <= windows.Length; i++)
        {
            if (i < windows.Length && IsSameScope(windows[groupStart], windows[i]))
            {
                continue;
            }

            AddSegments(CreateScope(windows[groupStart]), windows, groupStart, i - groupStart, segments);
            groupStart = i;
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
        for (var i = 0; i < count; i++)
        {
            var range = windows[startIndex + i].Window.Range;
            if (!range.HasEnd)
            {
                continue;
            }

            boundaries.Add(range.Start);
            boundaries.Add(range.End!.Value);
        }

        boundaries.Sort(static (left, right) => left.CompareTo(right));

        var unique = new List<TemporalPoint>(boundaries.Count);
        for (var i = 0; i < boundaries.Count; i++)
        {
            if (unique.Count == 0 || boundaries[i].CompareTo(unique[^1]) != 0)
            {
                unique.Add(boundaries[i]);
            }
        }

        for (var i = 0; i < unique.Count - 1; i++)
        {
            var start = unique[i];
            var end = unique[i + 1];
            if (start.CompareTo(end) >= 0)
            {
                continue;
            }

            var targetIds = new List<WindowRecordId>();
            var againstIds = new List<WindowRecordId>();

            for (var windowIndex = 0; windowIndex < count; windowIndex++)
            {
                var window = windows[startIndex + windowIndex].Window;
                if (!Covers(window.Range, start, end))
                {
                    continue;
                }

                if (window.Side == ComparisonSide.Target)
                {
                    targetIds.Add(window.RecordId);
                }
                else
                {
                    againstIds.Add(window.RecordId);
                }
            }

            if (targetIds.Count == 0 && againstIds.Count == 0)
            {
                continue;
            }

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

    private static bool IsSameScope(SortableNormalizedWindow first, SortableNormalizedWindow second)
    {
        return string.Equals(first.Window.Window.WindowName, second.Window.Window.WindowName, StringComparison.Ordinal)
            && string.Equals(first.KeySort, second.KeySort, StringComparison.Ordinal)
            && string.Equals(first.PartitionSort, second.PartitionSort, StringComparison.Ordinal)
            && first.SegmentContext.Equals(second.SegmentContext)
            && EqualityComparer<object>.Default.Equals(first.Window.Window.Key, second.Window.Window.Key)
            && EqualityComparer<object?>.Default.Equals(first.Window.Window.Partition, second.Window.Window.Partition);
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

    private readonly record struct SortableNormalizedWindow(
        NormalizedWindowRecord Window,
        string KeySort,
        string SourceSort,
        string PartitionSort,
        string SegmentSort,
        SegmentContext SegmentContext);
}
