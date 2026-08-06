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
            if (!ComparisonComparatorDeclarationParser.TryParse(comparator, out var declaration))
            {
                diagnostics.Add(new ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.UnknownComparator,
                    $"Comparator '{comparator}' is not registered.",
                    $"comparators[{i}]",
                    ComparisonPlanDiagnosticSeverity.Error));
                continue;
            }

            switch (declaration)
            {
                case ComparisonComparatorDeclaration.AsOf asOf:
                    var asOfBefore = asOfRows.Count;
                    AsOfComparison.AddRows(prepared, asOf, asOfRows, diagnostics);
                    summaries.Add(new ComparatorSummary(comparator, asOfRows.Count - asOfBefore));
                    continue;
                case ComparisonComparatorDeclaration.LeadLag leadLag:
                    var leadLagBefore = leadLagRows.Count;
                    AddLeadLagRows(prepared, leadLag, leadLagRows, leadLagSummaries);
                    summaries.Add(new ComparatorSummary(comparator, leadLagRows.Count - leadLagBefore));
                    continue;
                case ComparisonComparatorDeclaration.BuiltIn builtIn:
                    switch (builtIn.Kind)
                    {
                        case ComparisonComparatorKind.Overlap:
                            var overlapBefore = overlapRows.Count;
                            AddOverlapRows(cohortEvidence, aligned, overlapRows);
                            summaries.Add(new ComparatorSummary(comparator, overlapRows.Count - overlapBefore));
                            continue;
                        case ComparisonComparatorKind.Residual:
                            var residualBefore = residualRows.Count;
                            AddResidualRows(cohortEvidence, aligned, residualRows);
                            summaries.Add(new ComparatorSummary(comparator, residualRows.Count - residualBefore));
                            continue;
                        case ComparisonComparatorKind.Missing:
                            var missingBefore = missingRows.Count;
                            AddMissingRows(cohortEvidence, aligned, missingRows);
                            summaries.Add(new ComparatorSummary(comparator, missingRows.Count - missingBefore));
                            continue;
                        case ComparisonComparatorKind.Coverage:
                            var coverageBefore = coverageRows.Count;
                            AddCoverageRows(cohortEvidence, aligned, coverageRows, coverageSummaries);
                            summaries.Add(new ComparatorSummary(comparator, coverageRows.Count - coverageBefore));
                            continue;
                        case ComparisonComparatorKind.Gap:
                            var gapBefore = gapRows.Count;
                            AddGapRows(aligned, gapRows);
                            summaries.Add(new ComparatorSummary(comparator, gapRows.Count - gapBefore));
                            continue;
                        case ComparisonComparatorKind.SymmetricDifference:
                            var symmetricBefore = symmetricDifferenceRows.Count;
                            AddSymmetricDifferenceRows(cohortEvidence, aligned, symmetricDifferenceRows);
                            summaries.Add(new ComparatorSummary(comparator, symmetricDifferenceRows.Count - symmetricBefore));
                            continue;
                        case ComparisonComparatorKind.Containment:
                            var containmentBefore = containmentRows.Count;
                            AddContainmentRows(prepared, aligned, containmentRows);
                            summaries.Add(new ComparatorSummary(comparator, containmentRows.Count - containmentBefore));
                            continue;
                    }

                    break;
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
        var rowFinalities = ComparisonRowFinalityBuilder.Build(
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
        var cohortEvidenceMetadata = cohortEvidence.BuildMetadata(aligned);

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
            cohortEvidenceMetadata: cohortEvidenceMetadata);
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
        var summary = new Dictionary<CoverageScope, (long Target, long Covered)>();

        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.TargetRecordIds.Count == 0)
            {
                continue;
            }

            var targetMagnitudeExact = Measure(segment.Range);
            var targetMagnitude = (double)targetMagnitudeExact;
            var coveredMagnitudeExact = evidence.IsAgainstActive(segment) ? targetMagnitudeExact : 0L;
            var coveredMagnitude = (double)coveredMagnitudeExact;

            rows.Add(new CoverageRow(
                segment.WindowName,
                segment.Key,
                segment.Partition,
                segment.Range,
                targetMagnitude,
                coveredMagnitude,
                segment.TargetRecordIds,
                segment.AgainstRecordIds,
                targetMagnitudeExact,
                coveredMagnitudeExact));

            var key = new CoverageScope(segment.WindowName, segment.Key, segment.Partition);
            summary.TryGetValue(key, out var totals);
            summary[key] = (totals.Target + targetMagnitudeExact, totals.Covered + coveredMagnitudeExact);
        }

        foreach (var item in summary.OrderBy(static pair => pair.Key.WindowName, StringComparer.Ordinal))
        {
            summaries.Add(new CoverageSummary(
                item.Key.WindowName,
                item.Key.Key,
                item.Key.Partition,
                item.Value.Target,
                item.Value.Covered,
                item.Value.Target == 0L ? 0d : (double)item.Value.Covered / item.Value.Target,
                item.Value.Target,
                item.Value.Covered));
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

            var boundaryRecordIds = current.TargetRecordIds
                .Concat(current.AgainstRecordIds)
                .Concat(next.TargetRecordIds)
                .Concat(next.AgainstRecordIds)
                .Distinct()
                .ToArray();
            rows.Add(new GapRow(
                current.WindowName,
                current.Key,
                current.Partition,
                TemporalRange.Closed(gapStart, gapEnd),
                boundaryRecordIds));
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
        ComparisonComparatorDeclaration.LeadLag options,
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
            TemporalTransitionSearch.Sort(pair.Value);
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

            var nearest = TemporalTransitionSearch.FindNearest(candidates, targetPoint, options.Axis);
            var delta = TemporalTransitionSearch.GetDeltaMagnitude(targetPoint, nearest.Point, options.Axis);
            var absoluteDelta = delta == long.MinValue ? long.MaxValue : Math.Abs(delta);

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

    private static LeadLagSummary CreateLeadLagSummary(
        ComparisonComparatorDeclaration.LeadLag options,
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

    private static long Measure(TemporalRange range)
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

    private sealed record CoverageScope(string WindowName, object Key, object? Partition);
}
