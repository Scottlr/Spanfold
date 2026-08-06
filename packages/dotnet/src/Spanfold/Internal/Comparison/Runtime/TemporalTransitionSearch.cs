using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal static class TemporalTransitionSearch
{
    internal static void Sort(List<TransitionPoint> transitions)
    {
        transitions.Sort(static (left, right) =>
        {
            var pointComparison = left.Point.CompareTo(right.Point);
            return pointComparison != 0
                ? pointComparison
                : string.CompareOrdinal(left.RecordId.Value, right.RecordId.Value);
        });
    }

    internal static TransitionPoint FindNearest(
        List<TransitionPoint> candidates,
        TemporalPoint targetPoint,
        TemporalAxis axis)
    {
        return FindNearest(candidates, targetPoint, axis, LowerBound(candidates, targetPoint), out _);
    }

    internal static TransitionPoint FindNearest(
        List<TransitionPoint> candidates,
        TemporalPoint targetPoint,
        TemporalAxis axis,
        int lowerBound,
        out bool ambiguous)
    {
        if (lowerBound < candidates.Count && candidates[lowerBound].Point.CompareTo(targetPoint) == 0)
        {
            var runEnd = UpperBound(candidates, targetPoint, lowerBound);
            ambiguous = runEnd - lowerBound > 1;
            return candidates[lowerBound];
        }

        TransitionPoint? previous = null;
        var previousRunStart = 0;
        if (lowerBound > 0)
        {
            var previousPoint = candidates[lowerBound - 1].Point;
            previousRunStart = LowerBound(candidates, previousPoint);
            previous = candidates[previousRunStart];
        }

        TransitionPoint? next = lowerBound < candidates.Count
            ? candidates[lowerBound]
            : null;

        var previousDistance = previous.HasValue
            ? GetAbsoluteDistance(targetPoint, previous.Value.Point, axis)
            : long.MaxValue;
        var nextDistance = next.HasValue
            ? GetAbsoluteDistance(targetPoint, next.Value.Point, axis)
            : long.MaxValue;
        var bestDistance = Math.Min(previousDistance, nextDistance);

        // Saturation can make every candidate on an eligible side equally distant.
        if (bestDistance == long.MaxValue)
        {
            ambiguous = candidates.Count > 1;
            return FindSmallestRecordId(candidates, 0, candidates.Count);
        }

        if (!previous.HasValue)
        {
            var nextCandidate = next.GetValueOrDefault();
            var nextRunEnd = UpperBound(candidates, nextCandidate.Point, lowerBound);
            ambiguous = nextRunEnd - lowerBound > 1;
            return nextCandidate;
        }

        if (!next.HasValue)
        {
            ambiguous = lowerBound - previousRunStart > 1;
            return previous.Value;
        }

        if (previousDistance < nextDistance)
        {
            ambiguous = lowerBound - previousRunStart > 1;
            return previous.Value;
        }

        if (nextDistance < previousDistance)
        {
            var nextRunEnd = UpperBound(candidates, next.Value.Point, lowerBound);
            ambiguous = nextRunEnd - lowerBound > 1;
            return next.Value;
        }

        var previousRunCount = lowerBound - previousRunStart;
        var nextRunCount = UpperBound(candidates, next.Value.Point, lowerBound) - lowerBound;
        ambiguous = previousRunCount + nextRunCount > 1;
        return string.CompareOrdinal(previous.Value.RecordId.Value, next.Value.RecordId.Value) <= 0
            ? previous.Value
            : next.Value;
    }

    internal static int LowerBound(List<TransitionPoint> candidates, TemporalPoint point)
    {
        var lower = 0;
        var upper = candidates.Count;
        while (lower < upper)
        {
            var middle = lower + ((upper - lower) / 2);
            if (candidates[middle].Point.CompareTo(point) < 0)
            {
                lower = middle + 1;
            }
            else
            {
                upper = middle;
            }
        }

        return lower;
    }

    internal static int UpperBound(
        List<TransitionPoint> candidates,
        TemporalPoint point,
        int lowerBound)
    {
        var lower = lowerBound;
        var upper = candidates.Count;
        while (lower < upper)
        {
            var middle = lower + ((upper - lower) / 2);
            if (candidates[middle].Point.CompareTo(point) <= 0)
            {
                lower = middle + 1;
            }
            else
            {
                upper = middle;
            }
        }

        return lower;
    }

    internal static TransitionPoint FindSmallestRecordId(
        List<TransitionPoint> candidates,
        int startIndex,
        int endIndex)
    {
        var best = candidates[startIndex];
        for (var i = startIndex + 1; i < endIndex; i++)
        {
            var candidate = candidates[i];
            if (string.CompareOrdinal(candidate.RecordId.Value, best.RecordId.Value) < 0)
            {
                best = candidate;
            }
        }

        return best;
    }

    internal static long GetDeltaMagnitude(
        TemporalPoint targetPoint,
        TemporalPoint comparisonPoint,
        TemporalAxis axis)
    {
        return axis == TemporalAxis.Timestamp
            ? SaturatingSubtract(targetPoint.Timestamp.Ticks, comparisonPoint.Timestamp.Ticks)
            : SaturatingSubtract(targetPoint.Position, comparisonPoint.Position);
    }

    internal static long GetAbsoluteDistance(
        TemporalPoint targetPoint,
        TemporalPoint comparisonPoint,
        TemporalAxis axis)
    {
        var delta = GetDeltaMagnitude(targetPoint, comparisonPoint, axis);
        return delta == long.MinValue ? long.MaxValue : Math.Abs(delta);
    }

    private static long SaturatingSubtract(long left, long right)
    {
        if (right > 0 && left < long.MinValue + right)
        {
            return long.MinValue;
        }

        if (right < 0 && left > long.MaxValue + right)
        {
            return long.MaxValue;
        }

        return left - right;
    }
}

internal sealed record TransitionScope(
    string WindowName,
    object Key,
    object? Partition,
    SegmentContext Segments);

internal readonly record struct TransitionPoint(WindowRecordId RecordId, TemporalPoint Point);
