using Spanfold;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonRuntime
{
    internal static ComparisonResult Run(PreparedComparison prepared)
    {
        var diagnostics = new List<ComparisonPlanDiagnostic>(prepared.Diagnostics);
        diagnostics.AddRange(RuntimePlanCritic.Criticize(prepared));

        if (HasBlockingDiagnostics(diagnostics))
        {
            return new ComparisonResult(
                prepared.Plan,
                diagnostics.ToArray(),
                prepared);
        }

        var aligned = prepared.Align();
        var cohortEvidence = CohortEvidence.Create(prepared);
        var summaries = new List<ComparatorSummary>();
        var overlapRows = new List<OverlapRow>();
        var residualRows = new List<ResidualRow>();
        var missingRows = new List<MissingRow>();
        var coverageRows = new List<CoverageRow>();
        var coverageSummaries = new List<CoverageSummary>();
        var gapRows = new List<GapRow>();
        var symmetricDifferenceRows = new List<SymmetricDifferenceRow>();
        var containmentRows = new List<ContainmentRow>();
        var leadLagRows = new List<LeadLagRow>();
        var leadLagSummaries = new List<LeadLagSummary>();
        var asOfRows = new List<AsOfRow>();

        for (var i = 0; i < prepared.Plan.Comparators.Count; i++)
        {
            var comparator = prepared.Plan.Comparators[i];
            if (!ComparisonComparatorCatalog.IsKnownDeclaration(comparator))
            {
                diagnostics.Add(new ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.UnknownComparator,
                    $"Comparator '{comparator}' is not registered.",
                    $"comparators[{i}]",
                    ComparisonPlanDiagnosticSeverity.Error));
                continue;
            }

            if (TryParseAsOf(comparator, out var asOf))
            {
                var before = asOfRows.Count;
                AddAsOfRows(prepared, asOf, asOfRows, diagnostics);
                summaries.Add(new ComparatorSummary(comparator, asOfRows.Count - before));
                continue;
            }

            if (TryParseLeadLag(comparator, out var leadLag))
            {
                var before = leadLagRows.Count;
                AddLeadLagRows(prepared, leadLag, leadLagRows, leadLagSummaries);
                summaries.Add(new ComparatorSummary(comparator, leadLagRows.Count - before));
                continue;
            }

            if (string.Equals(comparator, "overlap", StringComparison.Ordinal))
            {
                var before = overlapRows.Count;
                AddOverlapRows(cohortEvidence, aligned, overlapRows);
                summaries.Add(new ComparatorSummary(comparator, overlapRows.Count - before));
                continue;
            }

            if (string.Equals(comparator, "residual", StringComparison.Ordinal))
            {
                var before = residualRows.Count;
                AddResidualRows(cohortEvidence, aligned, residualRows);
                summaries.Add(new ComparatorSummary(comparator, residualRows.Count - before));
                continue;
            }

            if (string.Equals(comparator, "missing", StringComparison.Ordinal))
            {
                var before = missingRows.Count;
                AddMissingRows(cohortEvidence, aligned, missingRows);
                summaries.Add(new ComparatorSummary(comparator, missingRows.Count - before));
                continue;
            }

            if (string.Equals(comparator, "coverage", StringComparison.Ordinal))
            {
                var before = coverageRows.Count;
                AddCoverageRows(cohortEvidence, aligned, coverageRows, coverageSummaries);
                summaries.Add(new ComparatorSummary(comparator, coverageRows.Count - before));
                continue;
            }

            if (string.Equals(comparator, "gap", StringComparison.Ordinal))
            {
                var before = gapRows.Count;
                AddGapRows(aligned, gapRows);
                summaries.Add(new ComparatorSummary(comparator, gapRows.Count - before));
                continue;
            }

            if (string.Equals(comparator, "symmetric-difference", StringComparison.Ordinal))
            {
                var before = symmetricDifferenceRows.Count;
                AddSymmetricDifferenceRows(cohortEvidence, aligned, symmetricDifferenceRows);
                summaries.Add(new ComparatorSummary(comparator, symmetricDifferenceRows.Count - before));
                continue;
            }

            if (string.Equals(comparator, "containment", StringComparison.Ordinal))
            {
                var before = containmentRows.Count;
                AddContainmentRows(prepared, aligned, containmentRows);
                summaries.Add(new ComparatorSummary(comparator, containmentRows.Count - before));
                continue;
            }

            summaries.Add(new ComparatorSummary(comparator, RowCount: 0));
        }

        var overlapArray = overlapRows.ToArray();
        var residualArray = residualRows.ToArray();
        var missingArray = missingRows.ToArray();
        var coverageArray = coverageRows.ToArray();
        var coverageSummaryArray = coverageSummaries.ToArray();
        var gapArray = gapRows.ToArray();
        var symmetricDifferenceArray = symmetricDifferenceRows.ToArray();
        var containmentArray = containmentRows.ToArray();
        var leadLagArray = leadLagRows.ToArray();
        var leadLagSummaryArray = leadLagSummaries.ToArray();
        var asOfArray = asOfRows.ToArray();
        var rowFinalities = BuildRowFinalities(
            prepared,
            aligned,
            cohortEvidence,
            overlapArray,
            residualArray,
            missingArray,
            coverageArray,
            gapArray,
            symmetricDifferenceArray,
            containmentArray,
            leadLagArray,
            asOfArray);
        var extensionMetadata = BuildCohortMetadata(cohortEvidence, aligned);

        return new ComparisonResult(
            prepared.Plan,
            diagnostics.ToArray(),
            prepared,
            aligned,
            summaries.ToArray(),
            overlapArray,
            residualArray,
            missingArray,
            coverageArray,
            coverageSummaryArray,
            gapArray,
            symmetricDifferenceArray,
            containmentArray,
            leadLagArray,
            leadLagSummaryArray,
            asOfArray,
            rowFinalities,
            extensionMetadata);
    }

    private static void AddOverlapRows(
        CohortEvidence evidence,
        AlignedComparison aligned,
        List<OverlapRow> rows)
    {
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.TargetRecordIds.Count == 0 || !evidence.IsAgainstActive(segment))
            {
                continue;
            }

            rows.Add(new OverlapRow(
                segment.WindowName,
                segment.Key,
                segment.Partition,
                segment.Range,
                segment.TargetRecordIds,
                segment.AgainstRecordIds));
        }
    }

    private static void AddResidualRows(
        CohortEvidence evidence,
        AlignedComparison aligned,
        List<ResidualRow> rows)
    {
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.TargetRecordIds.Count == 0 || evidence.IsAgainstActive(segment))
            {
                continue;
            }

            rows.Add(new ResidualRow(
                segment.WindowName,
                segment.Key,
                segment.Partition,
                segment.Range,
                segment.TargetRecordIds));
        }
    }

    private static void AddMissingRows(
        CohortEvidence evidence,
        AlignedComparison aligned,
        List<MissingRow> rows)
    {
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.TargetRecordIds.Count != 0 || !evidence.IsAgainstActive(segment))
            {
                continue;
            }

            rows.Add(new MissingRow(
                segment.WindowName,
                segment.Key,
                segment.Partition,
                segment.Range,
                segment.AgainstRecordIds));
        }
    }

    private static void AddCoverageRows(
        CohortEvidence evidence,
        AlignedComparison aligned,
        List<CoverageRow> rows,
        List<CoverageSummary> summaries)
    {
        var summary = new Dictionary<CoverageScope, (double Target, double Covered)>();

        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.TargetRecordIds.Count == 0)
            {
                continue;
            }

            var targetMagnitude = Measure(segment.Range);
            var coveredMagnitude = evidence.IsAgainstActive(segment) ? targetMagnitude : 0d;

            rows.Add(new CoverageRow(
                segment.WindowName,
                segment.Key,
                segment.Partition,
                segment.Range,
                targetMagnitude,
                coveredMagnitude,
                segment.TargetRecordIds,
                segment.AgainstRecordIds));

            var key = new CoverageScope(segment.WindowName, segment.Key, segment.Partition);
            summary.TryGetValue(key, out var totals);
            summary[key] = (totals.Target + targetMagnitude, totals.Covered + coveredMagnitude);
        }

        foreach (var item in summary.OrderBy(static pair => pair.Key.WindowName, StringComparer.Ordinal))
        {
            summaries.Add(new CoverageSummary(
                item.Key.WindowName,
                item.Key.Key,
                item.Key.Partition,
                item.Value.Target,
                item.Value.Covered,
                item.Value.Target == 0d ? 0d : item.Value.Covered / item.Value.Target));
        }
    }

    private static void AddGapRows(AlignedComparison aligned, List<GapRow> rows)
    {
        for (var i = 0; i < aligned.Segments.Count - 1; i++)
        {
            var current = aligned.Segments[i];
            var next = aligned.Segments[i + 1];

            if (!IsSameScope(current, next) || !current.Range.End.HasValue)
            {
                continue;
            }

            var gapStart = current.Range.End.Value;
            var gapEnd = next.Range.Start;
            if (gapStart.CompareTo(gapEnd) >= 0)
            {
                continue;
            }

            rows.Add(new GapRow(
                current.WindowName,
                current.Key,
                current.Partition,
                TemporalRange.Closed(gapStart, gapEnd)));
        }
    }

    private static void AddSymmetricDifferenceRows(
        CohortEvidence evidence,
        AlignedComparison aligned,
        List<SymmetricDifferenceRow> rows)
    {
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            var hasTarget = segment.TargetRecordIds.Count > 0;
            var hasAgainst = evidence.IsAgainstActive(segment);

            if (hasTarget == hasAgainst)
            {
                continue;
            }

            rows.Add(new SymmetricDifferenceRow(
                segment.WindowName,
                segment.Key,
                segment.Partition,
                segment.Range,
                hasTarget ? ComparisonSide.Target : ComparisonSide.Against,
                segment.TargetRecordIds,
                segment.AgainstRecordIds));
        }
    }

    private static void AddContainmentRows(
        PreparedComparison prepared,
        AlignedComparison aligned,
        List<ContainmentRow> rows)
    {
        var targetRanges = new Dictionary<WindowRecordId, TemporalRange>();
        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var window = prepared.NormalizedWindows[i];
            if (window.Side == ComparisonSide.Target)
            {
                targetRanges[window.RecordId] = window.Range;
            }
        }

        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.TargetRecordIds.Count == 0)
            {
                continue;
            }

            if (segment.AgainstRecordIds.Count > 0)
            {
                rows.Add(new ContainmentRow(
                    segment.WindowName,
                    segment.Key,
                    segment.Partition,
                    segment.Range,
                    ContainmentStatus.Contained,
                    segment.TargetRecordIds,
                    segment.AgainstRecordIds));
                continue;
            }

            for (var targetIndex = 0; targetIndex < segment.TargetRecordIds.Count; targetIndex++)
            {
                var targetId = segment.TargetRecordIds[targetIndex];
                rows.Add(new ContainmentRow(
                    segment.WindowName,
                    segment.Key,
                    segment.Partition,
                    segment.Range,
                    ClassifyUncontainedSegment(targetRanges, targetId, segment.Range),
                    new[] { targetId },
                    Array.Empty<WindowRecordId>()));
            }
        }
    }

    private static ContainmentStatus ClassifyUncontainedSegment(
        Dictionary<WindowRecordId, TemporalRange> targetRanges,
        WindowRecordId targetId,
        TemporalRange segmentRange)
    {
        if (!targetRanges.TryGetValue(targetId, out var targetRange) || !segmentRange.End.HasValue)
        {
            return ContainmentStatus.NotContained;
        }

        if (segmentRange.Start.CompareTo(targetRange.Start) == 0)
        {
            return ContainmentStatus.LeftOverhang;
        }

        if (targetRange.End.HasValue && segmentRange.End.Value.CompareTo(targetRange.End.Value) == 0)
        {
            return ContainmentStatus.RightOverhang;
        }

        return ContainmentStatus.NotContained;
    }

    private static void AddLeadLagRows(
        PreparedComparison prepared,
        LeadLagOptions options,
        List<LeadLagRow> rows,
        List<LeadLagSummary> summaries)
    {
        var before = rows.Count;
        var comparisonTransitions = new Dictionary<TransitionScope, List<TransitionPoint>>();

        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var window = prepared.NormalizedWindows[i];
            if (window.Side != ComparisonSide.Against
                || window.Range.Axis != options.Axis
                || !TryGetTransitionPoint(window.Range, options.Transition, out var point))
            {
                continue;
            }

            var scope = new TransitionScope(window.Window.WindowName, window.Window.Key, window.Window.Partition, new SegmentContext(window.Segments));
            if (!comparisonTransitions.TryGetValue(scope, out var transitions))
            {
                transitions = [];
                comparisonTransitions.Add(scope, transitions);
            }

            transitions.Add(new TransitionPoint(window.RecordId, point));
        }

        foreach (var pair in comparisonTransitions)
        {
            pair.Value.Sort(static (left, right) =>
            {
                var pointComparison = left.Point.CompareTo(right.Point);
                return pointComparison != 0
                    ? pointComparison
                    : string.CompareOrdinal(left.RecordId.Value, right.RecordId.Value);
            });
        }

        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var target = prepared.NormalizedWindows[i];
            if (target.Side != ComparisonSide.Target
                || target.Range.Axis != options.Axis
                || !TryGetTransitionPoint(target.Range, options.Transition, out var targetPoint))
            {
                continue;
            }

            var scope = new TransitionScope(target.Window.WindowName, target.Window.Key, target.Window.Partition, new SegmentContext(target.Segments));
            if (!comparisonTransitions.TryGetValue(scope, out var candidates) || candidates.Count == 0)
            {
                rows.Add(new LeadLagRow(
                    target.Window.WindowName,
                    target.Window.Key,
                    target.Window.Partition,
                    options.Transition,
                    options.Axis,
                    targetPoint,
                    ComparisonPoint: null,
                    DeltaMagnitude: null,
                    options.ToleranceMagnitude,
                    IsWithinTolerance: false,
                    LeadLagDirection.MissingComparison,
                    target.RecordId,
                    ComparisonRecordId: null));
                continue;
            }

            var nearest = FindNearest(candidates, targetPoint, options.Axis);
            var delta = GetDeltaMagnitude(targetPoint, nearest.Point, options.Axis);
            var absoluteDelta = Math.Abs(delta);

            rows.Add(new LeadLagRow(
                target.Window.WindowName,
                target.Window.Key,
                target.Window.Partition,
                options.Transition,
                options.Axis,
                targetPoint,
                nearest.Point,
                delta,
                options.ToleranceMagnitude,
                absoluteDelta <= options.ToleranceMagnitude,
                GetDirection(delta),
                target.RecordId,
                nearest.RecordId));
        }

        summaries.Add(CreateLeadLagSummary(options, rows, before));
    }

    private static void AddAsOfRows(
        PreparedComparison prepared,
        AsOfOptions options,
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
            pair.Value.Sort(static (left, right) =>
            {
                var pointComparison = left.Point.CompareTo(right.Point);
                return pointComparison != 0
                    ? pointComparison
                    : string.CompareOrdinal(left.RecordId.Value, right.RecordId.Value);
            });
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
                rows.Add(CreateAsOfRow(target, options, target.Range.Start, null, null, AsOfMatchStatus.NoMatch));
                continue;
            }

            var candidate = FindAsOfCandidate(candidates, target.Range.Start, options, out var ambiguous, out var futureRejected);
            if (!candidate.HasValue)
            {
                var future = futureRejected.HasValue
                    ? GetAbsoluteDistance(target.Range.Start, futureRejected.Value.Point, options.Axis)
                    : (long?)null;
                rows.Add(CreateAsOfRow(target, options, target.Range.Start, null, future, futureRejected.HasValue
                    ? AsOfMatchStatus.FutureRejected
                    : AsOfMatchStatus.NoMatch));
                continue;
            }

            var distance = GetAbsoluteDistance(target.Range.Start, candidate.Value.Point, options.Axis);
            if (distance > options.ToleranceMagnitude)
            {
                rows.Add(CreateAsOfRow(target, options, target.Range.Start, null, distance, AsOfMatchStatus.NoMatch));
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

            rows.Add(CreateAsOfRow(
                target,
                options,
                target.Range.Start,
                candidate.Value,
                distance,
                ambiguous
                    ? AsOfMatchStatus.Ambiguous
                    : distance == 0
                        ? AsOfMatchStatus.Exact
                        : AsOfMatchStatus.Matched));
        }
    }

    private static AsOfRow CreateAsOfRow(
        NormalizedWindowRecord target,
        AsOfOptions options,
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

    private static TransitionPoint? FindAsOfCandidate(
        List<TransitionPoint> candidates,
        TemporalPoint targetPoint,
        AsOfOptions options,
        out bool ambiguous,
        out TransitionPoint? futureRejected)
    {
        ambiguous = false;
        futureRejected = null;
        TransitionPoint? best = null;
        long? bestDistance = null;

        for (var i = 0; i < candidates.Count; i++)
        {
            var candidate = candidates[i];
            var comparison = candidate.Point.CompareTo(targetPoint);
            if (options.Direction == AsOfDirection.Previous && comparison > 0)
            {
                futureRejected ??= candidate;
                continue;
            }

            if (options.Direction == AsOfDirection.Next && comparison < 0)
            {
                continue;
            }

            var distance = GetAbsoluteDistance(targetPoint, candidate.Point, options.Axis);
            if (!bestDistance.HasValue || distance < bestDistance.Value)
            {
                best = candidate;
                bestDistance = distance;
                ambiguous = false;
                continue;
            }

            if (distance == bestDistance.Value)
            {
                ambiguous = true;
                if (best.HasValue && string.CompareOrdinal(candidate.RecordId.Value, best.Value.RecordId.Value) < 0)
                {
                    best = candidate;
                }
            }
        }

        return best;
    }

    private static bool TryGetTransitionPoint(
        TemporalRange range,
        LeadLagTransition transition,
        out TemporalPoint point)
    {
        if (transition == LeadLagTransition.Start)
        {
            point = range.Start;
            return true;
        }

        if (range.End.HasValue)
        {
            point = range.End.Value;
            return true;
        }

        point = default;
        return false;
    }

    private static TransitionPoint FindNearest(
        List<TransitionPoint> candidates,
        TemporalPoint targetPoint,
        TemporalAxis axis)
    {
        var best = candidates[0];
        var bestDistance = GetAbsoluteDistance(targetPoint, best.Point, axis);

        for (var i = 1; i < candidates.Count; i++)
        {
            var candidate = candidates[i];
            var distance = GetAbsoluteDistance(targetPoint, candidate.Point, axis);
            if (distance < bestDistance
                || (distance == bestDistance
                    && string.CompareOrdinal(candidate.RecordId.Value, best.RecordId.Value) < 0))
            {
                best = candidate;
                bestDistance = distance;
            }
        }

        return best;
    }

    private static LeadLagSummary CreateLeadLagSummary(
        LeadLagOptions options,
        List<LeadLagRow> rows,
        int startIndex)
    {
        var targetLeadCount = 0;
        var targetLagCount = 0;
        var equalCount = 0;
        var missingCount = 0;
        var outsideToleranceCount = 0;
        long? minimumDelta = null;
        long? maximumDelta = null;

        for (var i = startIndex; i < rows.Count; i++)
        {
            var row = rows[i];
            if (!row.IsWithinTolerance)
            {
                outsideToleranceCount++;
            }

            if (row.Direction == LeadLagDirection.TargetLeads)
            {
                targetLeadCount++;
            }
            else if (row.Direction == LeadLagDirection.TargetLags)
            {
                targetLagCount++;
            }
            else if (row.Direction == LeadLagDirection.Equal)
            {
                equalCount++;
            }
            else if (row.Direction == LeadLagDirection.MissingComparison)
            {
                missingCount++;
            }

            if (row.DeltaMagnitude.HasValue)
            {
                minimumDelta = !minimumDelta.HasValue || row.DeltaMagnitude.Value < minimumDelta.Value
                    ? row.DeltaMagnitude.Value
                    : minimumDelta;
                maximumDelta = !maximumDelta.HasValue || row.DeltaMagnitude.Value > maximumDelta.Value
                    ? row.DeltaMagnitude.Value
                    : maximumDelta;
            }
        }

        return new LeadLagSummary(
            options.Transition,
            options.Axis,
            options.ToleranceMagnitude,
            rows.Count - startIndex,
            targetLeadCount,
            targetLagCount,
            equalCount,
            missingCount,
            outsideToleranceCount,
            minimumDelta,
            maximumDelta);
    }

    private static LeadLagDirection GetDirection(long delta)
    {
        if (delta < 0)
        {
            return LeadLagDirection.TargetLeads;
        }

        return delta > 0
            ? LeadLagDirection.TargetLags
            : LeadLagDirection.Equal;
    }

    private static long GetDeltaMagnitude(
        TemporalPoint targetPoint,
        TemporalPoint comparisonPoint,
        TemporalAxis axis)
    {
        return axis == TemporalAxis.Timestamp
            ? SaturatingSubtract(targetPoint.Timestamp.Ticks, comparisonPoint.Timestamp.Ticks)
            : SaturatingSubtract(targetPoint.Position, comparisonPoint.Position);
    }

    private static long GetAbsoluteDistance(
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

    private static bool TryParseLeadLag(string comparator, out LeadLagOptions options)
    {
        options = default;

        var parts = comparator.Split(':');
        if (parts.Length != 4 || !string.Equals(parts[0], "lead-lag", StringComparison.Ordinal))
        {
            return false;
        }

        if (!Enum.TryParse(parts[1], ignoreCase: false, out LeadLagTransition transition)
            || !Enum.TryParse(parts[2], ignoreCase: false, out TemporalAxis axis)
            || axis == TemporalAxis.Unknown
            || !long.TryParse(parts[3], System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out var toleranceMagnitude)
            || toleranceMagnitude < 0)
        {
            return false;
        }

        options = new LeadLagOptions(transition, axis, toleranceMagnitude);
        return true;
    }

    private static bool TryParseAsOf(string comparator, out AsOfOptions options)
    {
        options = default;

        var parts = comparator.Split(':');
        if (parts.Length != 4 || !string.Equals(parts[0], "asof", StringComparison.Ordinal))
        {
            return false;
        }

        if (!Enum.TryParse(parts[1], ignoreCase: false, out AsOfDirection direction)
            || !Enum.TryParse(parts[2], ignoreCase: false, out TemporalAxis axis)
            || axis == TemporalAxis.Unknown
            || !long.TryParse(parts[3], System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out var toleranceMagnitude)
            || toleranceMagnitude < 0)
        {
            return false;
        }

        options = new AsOfOptions(direction, axis, toleranceMagnitude);
        return true;
    }

    private static ComparisonRowFinality[] BuildRowFinalities(
        PreparedComparison prepared,
        AlignedComparison aligned,
        CohortEvidence cohortEvidence,
        IReadOnlyList<OverlapRow> overlapRows,
        IReadOnlyList<ResidualRow> residualRows,
        IReadOnlyList<MissingRow> missingRows,
        IReadOnlyList<CoverageRow> coverageRows,
        IReadOnlyList<GapRow> gapRows,
        IReadOnlyList<SymmetricDifferenceRow> symmetricDifferenceRows,
        IReadOnlyList<ContainmentRow> containmentRows,
        IReadOnlyList<LeadLagRow> leadLagRows,
        IReadOnlyList<AsOfRow> asOfRows)
    {
        var provisionalRecordIds = prepared.NormalizedWindows
            .Where(static window => window.Range.EndStatus == TemporalRangeEndStatus.OpenAtHorizon)
            .Select(static window => window.RecordId)
            .ToHashSet();

        var finalities = new List<ComparisonRowFinality>(
            overlapRows.Count
            + residualRows.Count
            + missingRows.Count
            + coverageRows.Count
            + gapRows.Count
            + symmetricDifferenceRows.Count
            + containmentRows.Count
            + leadLagRows.Count
            + asOfRows.Count);

        for (var i = 0; i < overlapRows.Count; i++)
        {
            var row = overlapRows[i];
            AddRowFinality(finalities, provisionalRecordIds, "overlap", row, row.TargetRecordIds, row.AgainstRecordIds);
        }

        for (var i = 0; i < residualRows.Count; i++)
        {
            var row = residualRows[i];
            if (cohortEvidence.HasCohort
                && TryGetAlignedAgainstIds(aligned, row, out var againstRecordIds))
            {
                AddRowFinality(finalities, provisionalRecordIds, "residual", row, row.TargetRecordIds, againstRecordIds);
            }
            else
            {
                AddRowFinality(finalities, provisionalRecordIds, "residual", row, row.TargetRecordIds);
            }
        }

        for (var i = 0; i < missingRows.Count; i++)
        {
            AddRowFinality(finalities, provisionalRecordIds, "missing", missingRows[i], missingRows[i].AgainstRecordIds);
        }

        for (var i = 0; i < coverageRows.Count; i++)
        {
            var row = coverageRows[i];
            AddRowFinality(finalities, provisionalRecordIds, "coverage", row, row.TargetRecordIds, row.AgainstRecordIds);
        }

        for (var i = 0; i < gapRows.Count; i++)
        {
            AddRowFinality(finalities, provisionalRecordIds, "gap", gapRows[i]);
        }

        for (var i = 0; i < symmetricDifferenceRows.Count; i++)
        {
            var row = symmetricDifferenceRows[i];
            AddRowFinality(finalities, provisionalRecordIds, "symmetricDifference", row, row.TargetRecordIds, row.AgainstRecordIds);
        }

        for (var i = 0; i < containmentRows.Count; i++)
        {
            var row = containmentRows[i];
            AddRowFinality(finalities, provisionalRecordIds, "containment", row, row.TargetRecordIds, row.ContainerRecordIds);
        }

        for (var i = 0; i < leadLagRows.Count; i++)
        {
            var row = leadLagRows[i];
            AddRowFinality(finalities, provisionalRecordIds, "leadLag", row, row.TargetRecordId, row.ComparisonRecordId);
        }

        for (var i = 0; i < asOfRows.Count; i++)
        {
            var row = asOfRows[i];
            AddRowFinality(finalities, provisionalRecordIds, "asOf", row, row.TargetRecordId, row.MatchedRecordId);
        }

        return finalities.ToArray();
    }

    private static void AddRowFinality(
        List<ComparisonRowFinality> finalities,
        HashSet<WindowRecordId> provisionalRecordIds,
        string rowType,
        object row,
        params IReadOnlyList<WindowRecordId>[] recordIdGroups)
    {
        var finality = HasProvisionalRecord(provisionalRecordIds, recordIdGroups)
            ? ComparisonFinality.Provisional
            : ComparisonFinality.Final;

        finalities.Add(new ComparisonRowFinality(
            rowType,
            ComparisonRowIdentity.Create(rowType, row),
            finality,
            finality == ComparisonFinality.Provisional
                ? "Depends on at least one open window clipped to the evaluation horizon."
                : "All contributing windows were closed when the row was produced."));
    }

    private static void AddRowFinality(
        List<ComparisonRowFinality> finalities,
        HashSet<WindowRecordId> provisionalRecordIds,
        string rowType,
        object row,
        WindowRecordId firstRecordId,
        WindowRecordId? secondRecordId)
    {
        var finality = provisionalRecordIds.Contains(firstRecordId)
            || (secondRecordId.HasValue && provisionalRecordIds.Contains(secondRecordId.Value))
                ? ComparisonFinality.Provisional
                : ComparisonFinality.Final;

        finalities.Add(new ComparisonRowFinality(
            rowType,
            ComparisonRowIdentity.Create(rowType, row),
            finality,
            finality == ComparisonFinality.Provisional
                ? "Depends on at least one open window clipped to the evaluation horizon."
                : "All contributing windows were closed when the row was produced."));
    }

    private static bool HasProvisionalRecord(
        HashSet<WindowRecordId> provisionalRecordIds,
        IReadOnlyList<WindowRecordId>[] recordIdGroups)
    {
        for (var groupIndex = 0; groupIndex < recordIdGroups.Length; groupIndex++)
        {
            var group = recordIdGroups[groupIndex];
            for (var idIndex = 0; idIndex < group.Count; idIndex++)
            {
                if (provisionalRecordIds.Contains(group[idIndex]))
                {
                    return true;
                }
            }
        }

        return false;
    }

    private static bool TryGetAlignedAgainstIds(
        AlignedComparison aligned,
        ResidualRow row,
        out IReadOnlyList<WindowRecordId> againstRecordIds)
    {
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (string.Equals(segment.WindowName, row.WindowName, StringComparison.Ordinal)
                && EqualityComparer<object>.Default.Equals(segment.Key, row.Key)
                && EqualityComparer<object?>.Default.Equals(segment.Partition, row.Partition)
                && segment.Range == row.Range
                && RecordIdsEqual(segment.TargetRecordIds, row.TargetRecordIds))
            {
                againstRecordIds = segment.AgainstRecordIds;
                return true;
            }
        }

        againstRecordIds = [];
        return false;
    }

    private static bool RecordIdsEqual(
        IReadOnlyList<WindowRecordId> left,
        IReadOnlyList<WindowRecordId> right)
    {
        if (left.Count != right.Count)
        {
            return false;
        }

        for (var i = 0; i < left.Count; i++)
        {
            if (left[i] != right[i])
            {
                return false;
            }
        }

        return true;
    }

    private static bool HasBlockingDiagnostics(IReadOnlyList<ComparisonPlanDiagnostic> diagnostics)
    {
        for (var i = 0; i < diagnostics.Count; i++)
        {
            if (diagnostics[i].Severity == ComparisonPlanDiagnosticSeverity.Error)
            {
                return true;
            }
        }

        return false;
    }

    private static double Measure(TemporalRange range)
    {
        return range.Axis == TemporalAxis.Timestamp
            ? range.GetTimeDuration().Ticks
            : range.GetPositionLength();
    }

    private static bool IsSameScope(AlignedSegment first, AlignedSegment second)
    {
        return string.Equals(first.WindowName, second.WindowName, StringComparison.Ordinal)
            && EqualityComparer<object>.Default.Equals(first.Key, second.Key)
            && EqualityComparer<object?>.Default.Equals(first.Partition, second.Partition)
            && new SegmentContext(first.Segments).Equals(new SegmentContext(second.Segments));
    }

    private static IReadOnlyList<ComparisonExtensionMetadata> BuildCohortMetadata(
        CohortEvidence evidence,
        AlignedComparison aligned)
    {
        if (!evidence.HasCohort)
        {
            return [];
        }

        var metadata = new List<ComparisonExtensionMetadata>();
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.AgainstRecordIds.Count == 0 && segment.TargetRecordIds.Count == 0)
            {
                continue;
            }

            metadata.Add(new ComparisonExtensionMetadata(
                "spanfold.cohort",
                "segment[" + i.ToString(System.Globalization.CultureInfo.InvariantCulture) + "]",
                evidence.Describe(segment)));
        }

        return metadata.ToArray();
    }

    private sealed class CohortEvidence
    {
        private readonly ComparisonSelector? cohort;
        private readonly Dictionary<WindowRecordId, object?> sourcesByRecordId;

        private CohortEvidence(
            ComparisonSelector? cohort,
            Dictionary<WindowRecordId, object?> sourcesByRecordId)
        {
            this.cohort = cohort;
            this.sourcesByRecordId = sourcesByRecordId;
        }

        public bool HasCohort => this.cohort is not null;

        public static CohortEvidence Create(PreparedComparison prepared)
        {
            var cohort = prepared.Plan.Against.Count == 1
                && prepared.Plan.Against[0].CohortActivity is not null
                    ? prepared.Plan.Against[0]
                    : (ComparisonSelector?)null;
            var sourcesByRecordId = new Dictionary<WindowRecordId, object?>();

            for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
            {
                var window = prepared.NormalizedWindows[i];
                sourcesByRecordId[window.RecordId] = window.Window.Source;
            }

            return new CohortEvidence(cohort, sourcesByRecordId);
        }

        public bool IsAgainstActive(AlignedSegment segment)
        {
            if (this.cohort is null)
            {
                return segment.AgainstRecordIds.Count > 0;
            }

            var activeSources = ActiveSources(segment);

            return this.cohort.Value.CohortActivity!.IsActive(
                activeSources.Count,
                this.cohort.Value.CohortSources.Count);
        }

        public string Describe(AlignedSegment segment)
        {
            if (this.cohort is null)
            {
                return string.Empty;
            }

            var activeSources = ActiveSources(segment);
            var active = this.cohort.Value.CohortActivity!.IsActive(
                activeSources.Count,
                this.cohort.Value.CohortSources.Count);

            return "rule="
                + this.cohort.Value.CohortActivity!.Name
                + "; required="
                + RequiredCount().ToString(System.Globalization.CultureInfo.InvariantCulture)
                + "; activeCount="
                + activeSources.Count.ToString(System.Globalization.CultureInfo.InvariantCulture)
                + "; isActive="
                + (active ? "true" : "false")
                + "; activeSources="
                + string.Join(",", activeSources.Select(static source => source?.ToString() ?? "<null>"));
        }

        private int RequiredCount()
        {
            var activity = this.cohort!.Value.CohortActivity!;
            return activity.RequiredActiveCount(this.cohort.Value.CohortSources.Count);
        }

        private List<object?> ActiveSources(AlignedSegment segment)
        {
            var activeSources = new List<object?>();

            for (var i = 0; i < segment.AgainstRecordIds.Count; i++)
            {
                if (!this.sourcesByRecordId.TryGetValue(segment.AgainstRecordIds[i], out var source))
                {
                    continue;
                }

                if (!ContainsSource(activeSources, source))
                {
                    activeSources.Add(source);
                }
            }

            return activeSources;
        }

        private static bool ContainsSource(List<object?> sources, object? source)
        {
            for (var i = 0; i < sources.Count; i++)
            {
                if (EqualityComparer<object?>.Default.Equals(sources[i], source))
                {
                    return true;
                }
            }

            return false;
        }
    }

    private sealed record CoverageScope(string WindowName, object Key, object? Partition);

    private readonly record struct LeadLagOptions(
        LeadLagTransition Transition,
        TemporalAxis Axis,
        long ToleranceMagnitude);

    private readonly record struct AsOfOptions(
        AsOfDirection Direction,
        TemporalAxis Axis,
        long ToleranceMagnitude);

    private sealed record TransitionScope(
        string WindowName,
        object Key,
        object? Partition,
        SegmentContext Segments);

    private readonly record struct TransitionPoint(WindowRecordId RecordId, TemporalPoint Point);
}
