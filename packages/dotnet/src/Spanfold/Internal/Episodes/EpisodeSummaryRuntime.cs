using Spanfold.Episodes;
using Spanfold.Internal.Analysis;

namespace Spanfold.Internal.Episodes;

internal static class EpisodeSummaryRuntime
{
    internal static EpisodeSetSummary Summarize(
        EpisodeFormationPlan plan,
        IReadOnlyList<Episode> episodes)
    {
        var finalCount = 0;
        var provisionalCount = 0;
        var fragmentCount = 0;
        var multiFragmentCount = 0;
        var maximumFragments = 0;
        var totalActive = 0L;
        var totalElapsed = 0L;
        var totalInternalGap = 0L;
        var activeValues = new long[episodes.Count];
        var elapsedValues = new long[episodes.Count];
        var gapValues = new long[episodes.Count];

        for (var i = 0; i < episodes.Count; i++)
        {
            var episode = episodes[i];
            if (episode.TimeAxis != plan.Formation.TimeAxis)
            {
                throw new InvalidOperationException("Every episode must use the formation plan temporal axis.");
            }

            if (episode.Finality == ComparisonFinality.Final)
            {
                finalCount++;
            }
            else
            {
                provisionalCount++;
            }

            fragmentCount = checked(fragmentCount + episode.Fragments.Count);
            if (episode.Fragments.Count > 1)
            {
                multiFragmentCount++;
            }

            maximumFragments = Math.Max(maximumFragments, episode.Fragments.Count);
            totalActive = checked(totalActive + episode.ActiveMagnitude);
            totalElapsed = checked(totalElapsed + episode.ElapsedMagnitude);
            totalInternalGap = checked(totalInternalGap + episode.InternalGapMagnitude);
            activeValues[i] = episode.ActiveMagnitude;
            elapsedValues[i] = episode.ElapsedMagnitude;
            gapValues[i] = episode.InternalGapMagnitude;
        }

        return new EpisodeSetSummary(
            plan.Formation.TimeAxis,
            episodes.Count,
            finalCount,
            provisionalCount,
            fragmentCount,
            multiFragmentCount,
            Rate(multiFragmentCount, episodes.Count),
            episodes.Count == 0 ? null : (double)fragmentCount / episodes.Count,
            maximumFragments,
            totalActive,
            totalElapsed,
            totalInternalGap,
            Describe(activeValues),
            Describe(elapsedValues),
            Describe(gapValues));
    }

    internal static EpisodeComparisonSummary Summarize(
        EpisodeSet target,
        EpisodeSet against,
        IReadOnlyList<EpisodeRelation> relations)
    {
        var matchedTarget = 0;
        var matchedAgainst = 0;
        var unmatchedTarget = 0;
        var unmatchedAgainst = 0;
        var oneToOneCount = 0;
        var splitCount = 0;
        var mergeCount = 0;
        var complexCount = 0;
        var splitTargetCount = 0;
        var mergedAgainstCount = 0;
        var totalOverlap = 0L;
        var targetCoverageMagnitude = 0L;
        var againstCoverageMagnitude = 0L;
        var onsetValues = new List<long>();
        var recoveryValues = new List<long>();
        var activeDeltaValues = new List<long>();
        var elapsedDeltaValues = new List<long>();

        for (var i = 0; i < relations.Count; i++)
        {
            var relation = relations[i];
            var hasBothSides = relation.TargetEpisodes.Count > 0
                && relation.AgainstEpisodes.Count > 0;
            if (hasBothSides)
            {
                matchedTarget = checked(matchedTarget + relation.TargetEpisodes.Count);
                matchedAgainst = checked(matchedAgainst + relation.AgainstEpisodes.Count);
            }

            switch (relation.Kind)
            {
                case EpisodeRelationKind.OneToOne:
                    oneToOneCount++;
                    AddIfPresent(onsetValues, relation.Metrics.OnsetDeltaMagnitude);
                    AddIfPresent(recoveryValues, relation.Metrics.RecoveryDeltaMagnitude);
                    AddIfPresent(activeDeltaValues, relation.Metrics.ActiveMagnitudeDelta);
                    AddIfPresent(elapsedDeltaValues, relation.Metrics.ElapsedMagnitudeDelta);
                    break;
                case EpisodeRelationKind.Split:
                    splitCount++;
                    splitTargetCount = checked(splitTargetCount + relation.TargetEpisodes.Count);
                    break;
                case EpisodeRelationKind.Merge:
                    mergeCount++;
                    mergedAgainstCount = checked(mergedAgainstCount + relation.AgainstEpisodes.Count);
                    break;
                case EpisodeRelationKind.Complex:
                    complexCount++;
                    break;
                case EpisodeRelationKind.UnmatchedTarget:
                    unmatchedTarget = checked(unmatchedTarget + relation.TargetEpisodes.Count);
                    break;
                case EpisodeRelationKind.UnmatchedAgainst:
                    unmatchedAgainst = checked(unmatchedAgainst + relation.AgainstEpisodes.Count);
                    break;
                default:
                    throw new ArgumentOutOfRangeException(nameof(relations), relation.Kind, "Unknown episode relation kind.");
            }

            totalOverlap = checked(totalOverlap + relation.Metrics.OverlapMagnitude);
            targetCoverageMagnitude = checked(
                targetCoverageMagnitude + relation.Metrics.TargetActiveMagnitude);
            againstCoverageMagnitude = checked(
                againstCoverageMagnitude + relation.Metrics.AgainstActiveMagnitude);
        }

        var activeUnionMagnitude = (double)targetCoverageMagnitude
            + againstCoverageMagnitude
            - totalOverlap;

        return new EpisodeComparisonSummary(
            target.Summary.TimeAxis,
            target.Episodes.Count,
            against.Episodes.Count,
            matchedTarget,
            matchedAgainst,
            unmatchedTarget,
            unmatchedAgainst,
            oneToOneCount,
            splitCount,
            mergeCount,
            complexCount,
            splitTargetCount,
            mergedAgainstCount,
            against.Episodes.Count - target.Episodes.Count,
            TemporalMagnitudeMath.SaturatingSubtract(
                against.Summary.TotalActiveMagnitude,
                target.Summary.TotalActiveMagnitude),
            Rate(matchedTarget, target.Episodes.Count),
            Rate(matchedAgainst, against.Episodes.Count),
            Rate(splitTargetCount, target.Episodes.Count),
            Rate(mergedAgainstCount, against.Episodes.Count),
            totalOverlap,
            Ratio(totalOverlap, targetCoverageMagnitude),
            Ratio(totalOverlap, againstCoverageMagnitude),
            activeUnionMagnitude == 0d ? null : totalOverlap / activeUnionMagnitude,
            Describe(onsetValues),
            Describe(recoveryValues),
            Describe(activeDeltaValues),
            Describe(elapsedDeltaValues));
    }

    internal static EpisodeReferenceScorecard AsReference(EpisodeComparisonResult result)
    {
        var referenceCount = result.TargetEpisodes.Episodes.Count;
        var detectedReferenceCount = result.Summary.MatchedTargetEpisodeCount;
        var missedReferenceCount = result.Summary.UnmatchedTargetEpisodeCount;
        var detectionCount = result.AgainstEpisodes.Episodes.Count;
        var matchedDetectionCount = result.Summary.MatchedAgainstEpisodeCount;
        var unexpectedDetectionCount = result.Summary.UnmatchedAgainstEpisodeCount;
        var recall = Rate(detectedReferenceCount, referenceCount);
        var precision = Rate(matchedDetectionCount, detectionCount);
        double? f1 = null;

        if (recall.HasValue && precision.HasValue)
        {
            var total = recall.Value + precision.Value;
            f1 = total == 0d
                ? 0d
                : 2d * recall.Value * precision.Value / total;
        }

        return new EpisodeReferenceScorecard(
            referenceCount,
            detectedReferenceCount,
            missedReferenceCount,
            detectionCount,
            matchedDetectionCount,
            unexpectedDetectionCount,
            recall,
            precision,
            f1);
    }

    internal static EpisodeDistributionSummary Describe(IReadOnlyList<long> values)
    {
        if (values.Count == 0)
        {
            return new EpisodeDistributionSummary(0, null, null, null, null, null);
        }

        var ordered = values.ToArray();
        Array.Sort(ordered);
        var mean = 0d;
        for (var i = 0; i < ordered.Length; i++)
        {
            mean += ((double)ordered[i] - mean) / (i + 1);
        }

        var middle = ordered.Length / 2;
        var median = ordered.Length % 2 == 1
            ? ordered[middle]
            : (ordered[middle - 1] / 2d) + (ordered[middle] / 2d);
        var percentile95Index = (int)Math.Ceiling(0.95d * ordered.Length) - 1;

        return new EpisodeDistributionSummary(
            ordered.Length,
            ordered[0],
            mean,
            median,
            ordered[percentile95Index],
            ordered[^1]);
    }

    private static void AddIfPresent(List<long> values, long? value)
    {
        if (value.HasValue)
        {
            values.Add(value.Value);
        }
    }

    private static double? Rate(int numerator, int denominator)
    {
        return denominator == 0 ? null : (double)numerator / denominator;
    }

    private static double? Ratio(long numerator, long denominator)
    {
        return denominator == 0 ? null : (double)numerator / denominator;
    }
}
