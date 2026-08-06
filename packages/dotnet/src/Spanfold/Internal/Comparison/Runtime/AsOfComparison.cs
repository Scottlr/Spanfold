using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal static class AsOfComparison
{
    internal static void AddRows(
        PreparedComparison prepared,
        ComparisonComparatorDeclaration.AsOf options,
        List<AsOfRow> rows,
        List<ComparisonPlanDiagnostic> diagnostics)
    {
        var comparisonTransitions = new Dictionary<TransitionScope, List<TransitionPoint>>();

        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var window = prepared.NormalizedWindows[i];
            if (window.Side != ComparisonSide.Against || window.Range.Axis != options.Axis)
            {
                continue;
            }

            var scope = new TransitionScope(window.Window.WindowName, window.Window.Key, window.Window.Partition, new SegmentContext(window.Segments));
            if (!comparisonTransitions.TryGetValue(scope, out var transitions))
            {
                transitions = [];
                comparisonTransitions.Add(scope, transitions);
            }

            transitions.Add(new TransitionPoint(window.RecordId, window.Range.Start));
        }

        foreach (var pair in comparisonTransitions)
        {
            TemporalTransitionSearch.Sort(pair.Value);
        }

        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var target = prepared.NormalizedWindows[i];
            if (target.Side != ComparisonSide.Target || target.Range.Axis != options.Axis)
            {
                continue;
            }

            var scope = new TransitionScope(target.Window.WindowName, target.Window.Key, target.Window.Partition, new SegmentContext(target.Segments));
            if (!comparisonTransitions.TryGetValue(scope, out var candidates) || candidates.Count == 0)
            {
                rows.Add(CreateRow(target, options, target.Range.Start, null, null, AsOfMatchStatus.NoMatch));
                continue;
            }

            var candidate = FindCandidate(candidates, target.Range.Start, options, out var ambiguous, out var futureRejected);
            if (!candidate.HasValue)
            {
                var future = futureRejected.HasValue
                    ? TemporalTransitionSearch.GetAbsoluteDistance(target.Range.Start, futureRejected.Value.Point, options.Axis)
                    : (long?)null;
                rows.Add(CreateRow(target, options, target.Range.Start, null, future, futureRejected.HasValue
                    ? AsOfMatchStatus.FutureRejected
                    : AsOfMatchStatus.NoMatch));
                continue;
            }

            var distance = TemporalTransitionSearch.GetAbsoluteDistance(target.Range.Start, candidate.Value.Point, options.Axis);
            if (distance > options.ToleranceMagnitude)
            {
                rows.Add(CreateRow(target, options, target.Range.Start, null, distance, AsOfMatchStatus.NoMatch));
                continue;
            }

            if (ambiguous)
            {
                diagnostics.Add(new ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.AmbiguousAsOfMatch,
                    "As-of lookup found multiple equally eligible comparison transitions; the selected match is deterministic.",
                    $"asof[{target.RecordId}]",
                    ComparisonPlanDiagnosticSeverity.Warning));
            }

            rows.Add(CreateRow(
                target,
                options,
                target.Range.Start,
                candidate.Value,
                distance,
                GetMatchStatus(ambiguous, distance)));
        }
    }

    private static AsOfMatchStatus GetMatchStatus(bool ambiguous, long distance)
    {
        if (ambiguous)
        {
            return AsOfMatchStatus.Ambiguous;
        }

        return distance == 0
            ? AsOfMatchStatus.Exact
            : AsOfMatchStatus.Matched;
    }

    private static AsOfRow CreateRow(
        NormalizedWindowRecord target,
        ComparisonComparatorDeclaration.AsOf options,
        TemporalPoint targetPoint,
        TransitionPoint? match,
        long? distance,
        AsOfMatchStatus status)
    {
        return new AsOfRow(
            target.Window.WindowName,
            target.Window.Key,
            target.Window.Partition,
            options.Axis,
            options.Direction,
            targetPoint,
            match?.Point,
            distance,
            options.ToleranceMagnitude,
            status,
            target.RecordId,
            match?.RecordId);
    }

    private static TransitionPoint? FindCandidate(
        List<TransitionPoint> candidates,
        TemporalPoint targetPoint,
        ComparisonComparatorDeclaration.AsOf options,
        out bool ambiguous,
        out TransitionPoint? futureRejected)
    {
        ambiguous = false;
        futureRejected = null;

        var lowerBound = TemporalTransitionSearch.LowerBound(candidates, targetPoint);
        if (options.Direction == AsOfDirection.Previous)
        {
            var hasExactPoint = lowerBound < candidates.Count
                && candidates[lowerBound].Point.CompareTo(targetPoint) == 0;
            var upperBound = hasExactPoint
                ? TemporalTransitionSearch.UpperBound(candidates, targetPoint, lowerBound)
                : lowerBound;
            if (upperBound == 0)
            {
                futureRejected = candidates[0];
                return null;
            }

            var runStart = hasExactPoint
                ? lowerBound
                : TemporalTransitionSearch.LowerBound(candidates, candidates[upperBound - 1].Point);
            return FindClosestPrevious(candidates, targetPoint, options.Axis, runStart, upperBound, out ambiguous);
        }

        if (options.Direction == AsOfDirection.Next)
        {
            return lowerBound < candidates.Count
                ? FindClosestNext(candidates, targetPoint, options.Axis, lowerBound, out ambiguous)
                : null;
        }

        return TemporalTransitionSearch.FindNearest(candidates, targetPoint, options.Axis, lowerBound, out ambiguous);
    }

    private static TransitionPoint FindClosestPrevious(
        List<TransitionPoint> candidates,
        TemporalPoint targetPoint,
        TemporalAxis axis,
        int runStart,
        int upperBound,
        out bool ambiguous)
    {
        var point = candidates[upperBound - 1].Point;
        var distance = TemporalTransitionSearch.GetAbsoluteDistance(targetPoint, point, axis);
        if (distance == long.MaxValue)
        {
            ambiguous = upperBound > 1;
            return TemporalTransitionSearch.FindSmallestRecordId(candidates, 0, upperBound);
        }

        ambiguous = upperBound - runStart > 1;
        return candidates[runStart];
    }

    private static TransitionPoint FindClosestNext(
        List<TransitionPoint> candidates,
        TemporalPoint targetPoint,
        TemporalAxis axis,
        int lowerBound,
        out bool ambiguous)
    {
        var point = candidates[lowerBound].Point;
        var runEnd = TemporalTransitionSearch.UpperBound(candidates, point, lowerBound);
        var distance = TemporalTransitionSearch.GetAbsoluteDistance(targetPoint, point, axis);
        if (distance == long.MaxValue)
        {
            ambiguous = candidates.Count - lowerBound > 1;
            return TemporalTransitionSearch.FindSmallestRecordId(candidates, lowerBound, candidates.Count);
        }

        ambiguous = runEnd - lowerBound > 1;
        return candidates[lowerBound];
    }
}
