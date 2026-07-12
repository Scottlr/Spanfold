using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Recording;

/// <summary>
/// Owns closed-window interval analytics independently of history storage.
/// </summary>
internal static class WindowHistoryAnalytics
{
    internal static IReadOnlyList<WindowOverlap> FindOverlaps(IReadOnlyList<ClosedWindow> windows)
    {
        var overlaps = new List<WindowOverlap>();
        var scopes = windows
            .GroupBy(static window => new Scope(window.WindowName, window.Key, window.Partition, new SegmentContext(window.Segments)))
            .ToArray();

        for (var scopeIndex = 0; scopeIndex < scopes.Length; scopeIndex++)
        {
            var scoped = scopes[scopeIndex]
                .OrderBy(static window => window.StartPosition)
                .ThenBy(static window => window.EndPosition)
                .ToArray();
            for (var i = 0; i < scoped.Length; i++)
            {
                var first = scoped[i];
                var firstEnd = End(first);
                for (var j = i + 1; j < scoped.Length; j++)
                {
                    var second = scoped[j];
                    if (second.StartPosition >= firstEnd)
                    {
                        break;
                    }

                    if (Overlaps(first, second))
                    {
                        overlaps.Add(new WindowOverlap(first, second));
                    }
                }
            }
        }

        return overlaps.ToArray();
    }

    internal static IReadOnlyList<WindowResidualSegment> FindResiduals(
        IReadOnlyList<ClosedWindow> windows,
        object targetSource)
    {
        var residuals = new List<WindowResidualSegment>();
        foreach (var target in windows)
        {
            if (!EqualityComparer<object?>.Default.Equals(target.Source, targetSource))
            {
                continue;
            }

            var segments = new List<PositionSegment> { new(target.StartPosition, End(target)) };
            foreach (var comparison in windows)
            {
                if (ReferenceEquals(target, comparison)
                    || EqualityComparer<object?>.Default.Equals(comparison.Source, targetSource)
                    || !SameScope(target, comparison)
                    || !Overlaps(target, comparison))
                {
                    continue;
                }

                Subtract(segments, comparison);
            }

            foreach (var segment in segments)
            {
                if (segment.Start < segment.End)
                {
                    residuals.Add(new WindowResidualSegment(
                        target.WindowName,
                        target.Key,
                        targetSource,
                        segment.Start,
                        segment.End,
                        target.Partition));
                }
            }
        }

        return residuals.ToArray();
    }

    private static bool SameScope(ClosedWindow first, ClosedWindow second)
    {
        return string.Equals(first.WindowName, second.WindowName, StringComparison.Ordinal)
            && EqualityComparer<object>.Default.Equals(first.Key, second.Key)
            && EqualityComparer<object?>.Default.Equals(first.Partition, second.Partition)
            && new SegmentContext(first.Segments).Equals(new SegmentContext(second.Segments));
    }

    private static bool Overlaps(ClosedWindow first, ClosedWindow second)
    {
        return first.StartPosition < End(second) && second.StartPosition < End(first);
    }

    private static void Subtract(List<PositionSegment> segments, ClosedWindow comparison)
    {
        for (var i = segments.Count - 1; i >= 0; i--)
        {
            var segment = segments[i];
            var overlapStart = Math.Max(segment.Start, comparison.StartPosition);
            var overlapEnd = Math.Min(segment.End, End(comparison));
            if (overlapStart >= overlapEnd)
            {
                continue;
            }

            segments.RemoveAt(i);
            if (segment.Start < overlapStart)
            {
                segments.Insert(i, new PositionSegment(segment.Start, overlapStart));
                i++;
            }

            if (overlapEnd < segment.End)
            {
                segments.Insert(i, new PositionSegment(overlapEnd, segment.End));
            }
        }
    }

    private static long End(ClosedWindow window) => window.EndPosition!.Value;

    private sealed record Scope(string WindowName, object Key, object? Partition, SegmentContext Segments);
    private readonly record struct PositionSegment(long Start, long End);
}
