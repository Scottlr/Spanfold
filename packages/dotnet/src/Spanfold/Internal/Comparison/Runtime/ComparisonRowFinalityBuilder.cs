using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonRowFinalityBuilder
{
    internal static ComparisonRowFinality[] Build(
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
            Add(finalities, provisionalRecordIds, ComparisonRowKind.Overlap, row, row.TargetRecordIds, row.AgainstRecordIds);
        }

        for (var i = 0; i < residualRows.Count; i++)
        {
            var row = residualRows[i];
            if (cohortEvidence.HasCohort
                && TryGetAlignedAgainstIds(aligned, row, out var againstRecordIds))
            {
                Add(finalities, provisionalRecordIds, ComparisonRowKind.Residual, row, row.TargetRecordIds, againstRecordIds);
            }
            else
            {
                Add(finalities, provisionalRecordIds, ComparisonRowKind.Residual, row, row.TargetRecordIds);
            }
        }

        for (var i = 0; i < missingRows.Count; i++)
        {
            Add(finalities, provisionalRecordIds, ComparisonRowKind.Missing, missingRows[i], missingRows[i].AgainstRecordIds);
        }

        for (var i = 0; i < coverageRows.Count; i++)
        {
            var row = coverageRows[i];
            Add(finalities, provisionalRecordIds, ComparisonRowKind.Coverage, row, row.TargetRecordIds, row.AgainstRecordIds);
        }

        for (var i = 0; i < gapRows.Count; i++)
        {
            Add(finalities, provisionalRecordIds, ComparisonRowKind.Gap, gapRows[i], gapRows[i].BoundaryRecordIds);
        }

        for (var i = 0; i < symmetricDifferenceRows.Count; i++)
        {
            var row = symmetricDifferenceRows[i];
            Add(finalities, provisionalRecordIds, ComparisonRowKind.SymmetricDifference, row, row.TargetRecordIds, row.AgainstRecordIds);
        }

        for (var i = 0; i < containmentRows.Count; i++)
        {
            var row = containmentRows[i];
            Add(finalities, provisionalRecordIds, ComparisonRowKind.Containment, row, row.TargetRecordIds, row.ContainerRecordIds);
        }

        for (var i = 0; i < leadLagRows.Count; i++)
        {
            var row = leadLagRows[i];
            Add(finalities, provisionalRecordIds, ComparisonRowKind.LeadLag, row, row.TargetRecordId, row.ComparisonRecordId);
        }

        for (var i = 0; i < asOfRows.Count; i++)
        {
            var row = asOfRows[i];
            Add(finalities, provisionalRecordIds, ComparisonRowKind.AsOf, row, row.TargetRecordId, row.MatchedRecordId);
        }

        return finalities.ToArray();
    }

    private static void Add(
        List<ComparisonRowFinality> finalities,
        HashSet<WindowRecordId> provisionalRecordIds,
        ComparisonRowKind kind,
        object row,
        params IReadOnlyList<WindowRecordId>[] recordIdGroups)
    {
        var finality = HasProvisionalRecord(provisionalRecordIds, recordIdGroups)
            ? ComparisonFinality.Provisional
            : ComparisonFinality.Final;

        finalities.Add(Create(kind, row, finality));
    }

    private static void Add(
        List<ComparisonRowFinality> finalities,
        HashSet<WindowRecordId> provisionalRecordIds,
        ComparisonRowKind kind,
        object row,
        WindowRecordId firstRecordId,
        WindowRecordId? secondRecordId)
    {
        var finality = provisionalRecordIds.Contains(firstRecordId)
            || (secondRecordId.HasValue && provisionalRecordIds.Contains(secondRecordId.Value))
                ? ComparisonFinality.Provisional
                : ComparisonFinality.Final;

        finalities.Add(Create(kind, row, finality));
    }

    private static ComparisonRowFinality Create(
        ComparisonRowKind kind,
        object row,
        ComparisonFinality finality)
    {
        return new ComparisonRowFinality(
            new ComparisonRowReference(kind, ComparisonRowIdentity.Create(kind, row)),
            finality,
            finality == ComparisonFinality.Provisional
                ? "Depends on at least one open window clipped to the evaluation horizon."
                : "All contributing windows were closed when the row was produced.");
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
}
