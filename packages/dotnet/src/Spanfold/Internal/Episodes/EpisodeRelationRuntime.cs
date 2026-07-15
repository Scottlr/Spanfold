using Spanfold.Episodes;
using Spanfold.Internal.Analysis;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Episodes;

internal static class EpisodeRelationRuntime
{
    internal static EpisodeComparisonResult Run(
        WindowHistory history,
        EpisodeComparisonPlan plan)
    {
        ValidatePlan(plan);

        var targetPlan = new EpisodeFormationPlan(
            plan.TargetName,
            plan.Target.WithName(plan.TargetName),
            plan.Scope,
            plan.Normalization,
            plan.Formation);
        var againstPlan = new EpisodeFormationPlan(
            plan.AgainstName,
            plan.Against.WithName(plan.AgainstName),
            plan.Scope,
            plan.Normalization,
            plan.Formation);
        var targetSet = EpisodeFormationRuntime.Run(history, targetPlan);
        var againstSet = EpisodeFormationRuntime.Run(history, againstPlan);

        EnsureDisjointLineage(targetSet, againstSet, plan.TargetName, plan.AgainstName);

        var targetEdges = CreateEdges(targetSet.Episodes.Count);
        var againstEdges = CreateEdges(againstSet.Episodes.Count);
        for (var targetIndex = 0; targetIndex < targetSet.Episodes.Count; targetIndex++)
        {
            var target = targetSet.Episodes[targetIndex];
            for (var againstIndex = 0; againstIndex < againstSet.Episodes.Count; againstIndex++)
            {
                var against = againstSet.Episodes[againstIndex];
                if (!IsCompatible(target, against, targetSet.KeyComparers)
                    || !HasFragmentEdge(target, against, plan.Relation.ToleranceMagnitude))
                {
                    continue;
                }

                targetEdges[targetIndex].Add(againstIndex);
                againstEdges[againstIndex].Add(targetIndex);
            }
        }

        var evaluationHorizon = plan.Normalization.KnownAt
            ?? plan.Normalization.OpenWindowHorizon;
        var relations = BuildRelations(
            targetSet,
            againstSet,
            targetEdges,
            againstEdges,
            plan.Relation,
            evaluationHorizon);
        relations.Sort(CompareRelations);

        return new EpisodeComparisonResult(
            plan,
            targetSet,
            againstSet,
            relations,
            evaluationHorizon);
    }

    private static void ValidatePlan(EpisodeComparisonPlan plan)
    {
        if (plan.Scope.TimeAxis != plan.Normalization.TimeAxis
            || plan.Formation.TimeAxis != plan.Normalization.TimeAxis
            || plan.Relation.TimeAxis != plan.Normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode comparison plan components must use one temporal axis.");
        }
    }

    private static List<int>[] CreateEdges(int count)
    {
        var edges = new List<int>[count];
        for (var i = 0; i < count; i++)
        {
            edges[i] = [];
        }

        return edges;
    }

    private static void EnsureDisjointLineage(
        EpisodeSet targetSet,
        EpisodeSet againstSet,
        string targetName,
        string againstName)
    {
        var targetRecordIds = new HashSet<WindowRecordId>();
        for (var episodeIndex = 0; episodeIndex < targetSet.Episodes.Count; episodeIndex++)
        {
            var fragments = targetSet.Episodes[episodeIndex].Fragments;
            for (var fragmentIndex = 0; fragmentIndex < fragments.Count; fragmentIndex++)
            {
                targetRecordIds.Add(fragments[fragmentIndex].RecordId);
            }
        }

        for (var episodeIndex = 0; episodeIndex < againstSet.Episodes.Count; episodeIndex++)
        {
            var fragments = againstSet.Episodes[episodeIndex].Fragments;
            for (var fragmentIndex = 0; fragmentIndex < fragments.Count; fragmentIndex++)
            {
                var recordId = fragments[fragmentIndex].RecordId;
                if (targetRecordIds.Contains(recordId))
                {
                    throw new InvalidOperationException(
                        $"Window record '{recordId}' belongs to both episode selectors '{targetName}' and '{againstName}'.");
                }
            }
        }
    }

    private static bool IsCompatible(
        Episode target,
        Episode against,
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        if (!string.Equals(target.WindowName, against.WindowName, StringComparison.Ordinal)
            || !EqualityComparer<object?>.Default.Equals(target.Partition, against.Partition)
            || target.TimeAxis != against.TimeAxis
            || !string.Equals(
                target.Envelope.Start.Clock,
                against.Envelope.Start.Clock,
                StringComparison.Ordinal))
        {
            return false;
        }

        var comparer = keyComparers.TryGetValue(target.WindowName, out var configured)
            ? configured
            : EqualityComparer<object>.Default;
        return comparer.Equals(target.Key, against.Key);
    }

    private static bool HasFragmentEdge(
        Episode target,
        Episode against,
        long toleranceMagnitude)
    {
        for (var targetIndex = 0; targetIndex < target.Fragments.Count; targetIndex++)
        {
            for (var againstIndex = 0; againstIndex < against.Fragments.Count; againstIndex++)
            {
                if (GapMagnitude(
                    target.Fragments[targetIndex].Range,
                    against.Fragments[againstIndex].Range) <= toleranceMagnitude)
                {
                    return true;
                }
            }
        }

        return false;
    }

    private static List<EpisodeRelation> BuildRelations(
        EpisodeSet targetSet,
        EpisodeSet againstSet,
        List<int>[] targetEdges,
        List<int>[] againstEdges,
        EpisodeRelationPolicy policy,
        TemporalPoint? evaluationHorizon)
    {
        var relations = new List<EpisodeRelation>();
        var visitedTargets = new bool[targetSet.Episodes.Count];
        var visitedAgainst = new bool[againstSet.Episodes.Count];

        for (var i = 0; i < targetSet.Episodes.Count; i++)
        {
            if (!visitedTargets[i])
            {
                relations.Add(TraverseComponent(
                    new GraphNode(IsTarget: true, i),
                    targetSet,
                    againstSet,
                    targetEdges,
                    againstEdges,
                    visitedTargets,
                    visitedAgainst,
                    policy,
                    evaluationHorizon));
            }
        }

        for (var i = 0; i < againstSet.Episodes.Count; i++)
        {
            if (!visitedAgainst[i])
            {
                relations.Add(TraverseComponent(
                    new GraphNode(IsTarget: false, i),
                    targetSet,
                    againstSet,
                    targetEdges,
                    againstEdges,
                    visitedTargets,
                    visitedAgainst,
                    policy,
                    evaluationHorizon));
            }
        }

        return relations;
    }

    private static EpisodeRelation TraverseComponent(
        GraphNode first,
        EpisodeSet targetSet,
        EpisodeSet againstSet,
        List<int>[] targetEdges,
        List<int>[] againstEdges,
        bool[] visitedTargets,
        bool[] visitedAgainst,
        EpisodeRelationPolicy policy,
        TemporalPoint? evaluationHorizon)
    {
        var queue = new Queue<GraphNode>();
        var targetEpisodes = new List<Episode>();
        var againstEpisodes = new List<Episode>();
        MarkVisited(first, visitedTargets, visitedAgainst);
        queue.Enqueue(first);

        while (queue.Count > 0)
        {
            var node = queue.Dequeue();
            if (node.IsTarget)
            {
                targetEpisodes.Add(targetSet.Episodes[node.Index]);
                EnqueueNeighbors(
                    targetEdges[node.Index],
                    isTarget: false,
                    queue,
                    visitedTargets,
                    visitedAgainst);
            }
            else
            {
                againstEpisodes.Add(againstSet.Episodes[node.Index]);
                EnqueueNeighbors(
                    againstEdges[node.Index],
                    isTarget: true,
                    queue,
                    visitedTargets,
                    visitedAgainst);
            }
        }

        targetEpisodes.Sort(CompareEpisodes);
        againstEpisodes.Sort(CompareEpisodes);
        var metrics = CalculateMetrics(targetEpisodes, againstEpisodes, policy.TimeAxis);
        var finality = CalculateFinality(
            targetEpisodes,
            againstEpisodes,
            policy.ToleranceMagnitude,
            evaluationHorizon);

        return new EpisodeRelation(
            Classify(targetEpisodes.Count, againstEpisodes.Count),
            targetEpisodes,
            againstEpisodes,
            metrics,
            finality);
    }

    private static void EnqueueNeighbors(
        List<int> neighbors,
        bool isTarget,
        Queue<GraphNode> queue,
        bool[] visitedTargets,
        bool[] visitedAgainst)
    {
        for (var i = 0; i < neighbors.Count; i++)
        {
            var node = new GraphNode(isTarget, neighbors[i]);
            if (IsVisited(node, visitedTargets, visitedAgainst))
            {
                continue;
            }

            MarkVisited(node, visitedTargets, visitedAgainst);
            queue.Enqueue(node);
        }
    }

    private static bool IsVisited(
        GraphNode node,
        bool[] visitedTargets,
        bool[] visitedAgainst)
    {
        return node.IsTarget
            ? visitedTargets[node.Index]
            : visitedAgainst[node.Index];
    }

    private static void MarkVisited(
        GraphNode node,
        bool[] visitedTargets,
        bool[] visitedAgainst)
    {
        if (node.IsTarget)
        {
            visitedTargets[node.Index] = true;
        }
        else
        {
            visitedAgainst[node.Index] = true;
        }
    }

    private static EpisodeRelationKind Classify(int targetCount, int againstCount)
    {
        if (targetCount == 0)
        {
            return EpisodeRelationKind.UnmatchedAgainst;
        }

        if (againstCount == 0)
        {
            return EpisodeRelationKind.UnmatchedTarget;
        }

        if (targetCount == 1 && againstCount == 1)
        {
            return EpisodeRelationKind.OneToOne;
        }

        if (targetCount == 1)
        {
            return EpisodeRelationKind.Split;
        }

        return againstCount == 1
            ? EpisodeRelationKind.Merge
            : EpisodeRelationKind.Complex;
    }

    private static EpisodeRelationMetrics CalculateMetrics(
        IReadOnlyList<Episode> targetEpisodes,
        IReadOnlyList<Episode> againstEpisodes,
        TemporalAxis timeAxis)
    {
        var targetUnion = BuildUnion(targetEpisodes);
        var againstUnion = BuildUnion(againstEpisodes);
        var targetMagnitude = UnionMagnitude(targetUnion);
        var againstMagnitude = UnionMagnitude(againstUnion);
        var overlapMagnitude = IntersectionMagnitude(targetUnion, againstUnion);
        var activeUnionMagnitude = (double)targetMagnitude + againstMagnitude - overlapMagnitude;
        var hasBothSides = targetEpisodes.Count > 0 && againstEpisodes.Count > 0;

        return new EpisodeRelationMetrics(
            timeAxis,
            targetMagnitude,
            againstMagnitude,
            overlapMagnitude,
            targetMagnitude == 0 ? null : (double)overlapMagnitude / targetMagnitude,
            againstMagnitude == 0 ? null : (double)overlapMagnitude / againstMagnitude,
            activeUnionMagnitude == 0d ? null : overlapMagnitude / activeUnionMagnitude,
            hasBothSides ? MinimumGapMagnitude(targetUnion, againstUnion) : null,
            hasBothSides
                ? TemporalMagnitudeMath.SaturatingSubtract(againstUnion[0].Start, targetUnion[0].Start)
                : null,
            hasBothSides
                ? TemporalMagnitudeMath.SaturatingSubtract(againstUnion[^1].End, targetUnion[^1].End)
                : null,
            hasBothSides
                ? TemporalMagnitudeMath.SaturatingSubtract(againstMagnitude, targetMagnitude)
                : null,
            hasBothSides
                ? TemporalMagnitudeMath.SaturatingSubtract(
                    RangeMagnitude(againstUnion[0].Start, againstUnion[^1].End),
                    RangeMagnitude(targetUnion[0].Start, targetUnion[^1].End))
                : null);
    }

    private static List<MagnitudeInterval> BuildUnion(IReadOnlyList<Episode> episodes)
    {
        var ranges = new List<MagnitudeInterval>();
        for (var episodeIndex = 0; episodeIndex < episodes.Count; episodeIndex++)
        {
            var fragments = episodes[episodeIndex].Fragments;
            for (var fragmentIndex = 0; fragmentIndex < fragments.Count; fragmentIndex++)
            {
                var range = fragments[fragmentIndex].Range;
                ranges.Add(new MagnitudeInterval(
                    PointMagnitude(range.Start),
                    PointMagnitude(RequireEnd(range))));
            }
        }

        ranges.Sort(static (left, right) =>
        {
            var start = left.Start.CompareTo(right.Start);
            return start != 0 ? start : left.End.CompareTo(right.End);
        });

        if (ranges.Count == 0)
        {
            return ranges;
        }

        var union = new List<MagnitudeInterval>();
        var current = ranges[0];
        for (var i = 1; i < ranges.Count; i++)
        {
            var next = ranges[i];
            if (next.Start <= current.End)
            {
                current = new MagnitudeInterval(current.Start, Math.Max(current.End, next.End));
                continue;
            }

            union.Add(current);
            current = next;
        }

        union.Add(current);
        return union;
    }

    private static long UnionMagnitude(List<MagnitudeInterval> ranges)
    {
        var total = 0L;
        for (var i = 0; i < ranges.Count; i++)
        {
            total = checked(total + RangeMagnitude(ranges[i].Start, ranges[i].End));
        }

        return total;
    }

    private static long IntersectionMagnitude(
        List<MagnitudeInterval> target,
        List<MagnitudeInterval> against)
    {
        var total = 0L;
        var targetIndex = 0;
        var againstIndex = 0;
        while (targetIndex < target.Count && againstIndex < against.Count)
        {
            var start = Math.Max(target[targetIndex].Start, against[againstIndex].Start);
            var end = Math.Min(target[targetIndex].End, against[againstIndex].End);
            if (end > start)
            {
                total = checked(total + RangeMagnitude(start, end));
            }

            if (target[targetIndex].End <= against[againstIndex].End)
            {
                targetIndex++;
            }
            else
            {
                againstIndex++;
            }
        }

        return total;
    }

    private static long MinimumGapMagnitude(
        List<MagnitudeInterval> target,
        List<MagnitudeInterval> against)
    {
        var minimum = long.MaxValue;
        for (var targetIndex = 0; targetIndex < target.Count; targetIndex++)
        {
            for (var againstIndex = 0; againstIndex < against.Count; againstIndex++)
            {
                var gap = GapMagnitude(target[targetIndex], against[againstIndex]);
                if (gap == 0)
                {
                    return 0;
                }

                minimum = Math.Min(minimum, gap);
            }
        }

        return minimum;
    }

    private static long GapMagnitude(TemporalRange left, TemporalRange right)
    {
        return GapMagnitude(
            new MagnitudeInterval(PointMagnitude(left.Start), PointMagnitude(RequireEnd(left))),
            new MagnitudeInterval(PointMagnitude(right.Start), PointMagnitude(RequireEnd(right))));
    }

    private static long GapMagnitude(MagnitudeInterval left, MagnitudeInterval right)
    {
        if (left.End < right.Start)
        {
            return TemporalMagnitudeMath.SaturatingSubtract(right.Start, left.End);
        }

        return right.End < left.Start
            ? TemporalMagnitudeMath.SaturatingSubtract(left.Start, right.End)
            : 0;
    }

    private static ComparisonFinality CalculateFinality(
        IReadOnlyList<Episode> targetEpisodes,
        IReadOnlyList<Episode> againstEpisodes,
        long toleranceMagnitude,
        TemporalPoint? evaluationHorizon)
    {
        var latestEnd = long.MinValue;
        var allEpisodes = targetEpisodes.Concat(againstEpisodes);
        foreach (var episode in allEpisodes)
        {
            if (episode.Finality == ComparisonFinality.Provisional)
            {
                return ComparisonFinality.Provisional;
            }

            for (var i = 0; i < episode.Fragments.Count; i++)
            {
                latestEnd = Math.Max(
                    latestEnd,
                    PointMagnitude(RequireEnd(episode.Fragments[i].Range)));
            }
        }

        if (!evaluationHorizon.HasValue)
        {
            return ComparisonFinality.Final;
        }

        var settlingBoundary = TemporalMagnitudeMath.SaturatingAdd(latestEnd, toleranceMagnitude);
        return PointMagnitude(evaluationHorizon.Value) <= settlingBoundary
            ? ComparisonFinality.Provisional
            : ComparisonFinality.Final;
    }

    private static int CompareRelations(EpisodeRelation left, EpisodeRelation right)
    {
        var leftEpisode = FirstEpisode(left);
        var rightEpisode = FirstEpisode(right);
        var comparison = CompareEpisodes(leftEpisode, rightEpisode);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.Kind.CompareTo(right.Kind);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.TargetEpisodes.Count.CompareTo(right.TargetEpisodes.Count);
        return comparison != 0
            ? comparison
            : left.AgainstEpisodes.Count.CompareTo(right.AgainstEpisodes.Count);
    }

    private static Episode FirstEpisode(EpisodeRelation relation)
    {
        if (relation.TargetEpisodes.Count == 0)
        {
            return relation.AgainstEpisodes[0];
        }

        if (relation.AgainstEpisodes.Count == 0)
        {
            return relation.TargetEpisodes[0];
        }

        return CompareEpisodes(relation.TargetEpisodes[0], relation.AgainstEpisodes[0]) <= 0
            ? relation.TargetEpisodes[0]
            : relation.AgainstEpisodes[0];
    }

    private static int CompareEpisodes(Episode left, Episode right)
    {
        var comparison = string.CompareOrdinal(left.WindowName, right.WindowName);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.CompareOrdinal(
            CanonicalValueFormatter.Format(left.Key),
            CanonicalValueFormatter.Format(right.Key));
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.CompareOrdinal(
            CanonicalValueFormatter.Format(left.Partition),
            CanonicalValueFormatter.Format(right.Partition));
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.TimeAxis.CompareTo(right.TimeAxis);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.CompareOrdinal(
            left.Envelope.Start.Clock,
            right.Envelope.Start.Clock);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.Envelope.Start.CompareTo(right.Envelope.Start);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = RequireEnd(left.Envelope).CompareTo(RequireEnd(right.Envelope));
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.CompareOrdinal(
            CanonicalValueFormatter.Format(left.Source),
            CanonicalValueFormatter.Format(right.Source));
        return comparison != 0
            ? comparison
            : string.CompareOrdinal(left.Id.Value, right.Id.Value);
    }

    private static long RangeMagnitude(long start, long end)
    {
        return TemporalMagnitudeMath.SaturatingSubtract(end, start);
    }

    private static long PointMagnitude(TemporalPoint point)
    {
        return point.Axis switch
        {
            TemporalAxis.ProcessingPosition => point.Position,
            TemporalAxis.Timestamp => point.Timestamp.UtcDateTime.Ticks,
            _ => throw new InvalidOperationException("Episode relation requires a known temporal axis.")
        };
    }

    private static TemporalPoint RequireEnd(TemporalRange range)
    {
        return range.End
            ?? throw new InvalidOperationException("Episode relation fragments require an effective end.");
    }

    private readonly record struct GraphNode(bool IsTarget, int Index);

    private readonly record struct MagnitudeInterval(long Start, long End);
}
