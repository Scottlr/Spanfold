using Spanfold;
using Spanfold.Internal.Episodes;

namespace Spanfold.Tests.Episodes;

public sealed class EpisodeSummaryTests
{
    [Fact]
    public void EmptySetUsesPlanAxisAndNullRates()
    {
        var result = WindowHistory.FromRecords([], [])
            .FormEpisodes("Empty")
            .From(selector => selector.Source("target"))
            .Within(scope => scope.Window("State"))
            .Run();

        Assert.Equal(TemporalAxis.ProcessingPosition, result.Summary.TimeAxis);
        Assert.Equal(0, result.Summary.EpisodeCount);
        Assert.Equal(0, result.Summary.FragmentCount);
        Assert.Null(result.Summary.MultiFragmentEpisodeRate);
        Assert.Null(result.Summary.MeanFragmentsPerEpisode);
        Assert.Equal(0, result.Summary.MaximumFragmentsPerEpisode);
        Assert.Equal(0, result.Summary.ActiveMagnitudeDistribution.Count);
        Assert.Null(result.Summary.ActiveMagnitudeDistribution.Median);
    }

    [Fact]
    public void SetSummaryReportsFragmentsTotalsAndFinality()
    {
        var history = WindowHistory.FromRecords(
            [Target(0, 2), Target(4, 6)],
            []);

        var result = history.FormEpisodes("Target episodes")
            .From(selector => selector.Source("target"))
            .Within(scope => scope.Window("State"))
            .StitchGapsUpTo(2)
            .Run();

        Assert.Equal(1, result.Summary.EpisodeCount);
        Assert.Equal(1, result.Summary.FinalEpisodeCount);
        Assert.Equal(0, result.Summary.ProvisionalEpisodeCount);
        Assert.Equal(2, result.Summary.FragmentCount);
        Assert.Equal(1, result.Summary.MultiFragmentEpisodeCount);
        Assert.Equal(1, result.Summary.MultiFragmentEpisodeRate);
        Assert.Equal(2, result.Summary.MeanFragmentsPerEpisode);
        Assert.Equal(2, result.Summary.MaximumFragmentsPerEpisode);
        Assert.Equal(4, result.Summary.TotalActiveMagnitude);
        Assert.Equal(6, result.Summary.TotalElapsedMagnitude);
        Assert.Equal(2, result.Summary.TotalInternalGapMagnitude);
    }

    [Fact]
    public void LiveSetSummaryCountsProvisionalEpisodes()
    {
        var history = WindowHistory.FromRecords(
            [],
            [new OpenWindow("State", "device-1", 0, Source: "target")]);

        var result = history.FormEpisodes("Live target episodes")
            .From(selector => selector.Source("target"))
            .Within(scope => scope.Window("State"))
            .RunLive(TemporalPoint.ForPosition(5));

        Assert.Equal(1, result.Summary.EpisodeCount);
        Assert.Equal(0, result.Summary.FinalEpisodeCount);
        Assert.Equal(1, result.Summary.ProvisionalEpisodeCount);
    }

    [Fact]
    public void DistributionUsesIncrementalMeanMedianAndNearestRankP95()
    {
        var odd = EpisodeSummaryRuntime.Describe([-2, 0, 4, 8, 100]);
        var even = EpisodeSummaryRuntime.Describe([long.MinValue, long.MaxValue]);
        var twenty = EpisodeSummaryRuntime.Describe(
            Enumerable.Range(1, 20).Select(value => (long)value).ToArray());

        Assert.Equal(5, odd.Count);
        Assert.Equal(22, odd.Mean);
        Assert.Equal(4, odd.Median);
        Assert.Equal(100, odd.Percentile95);
        Assert.Equal(0, even.Mean);
        Assert.Equal(0, even.Median);
        Assert.Equal(long.MaxValue, even.Percentile95);
        Assert.Equal(19, twenty.Percentile95);
    }

    [Fact]
    public void ComparisonSummaryCountsEveryComponentWithoutDoubleCountingEpisodes()
    {
        var result = Compare(
            Target(0, 5, "one"), Against(1, 4, "one"),
            Target(0, 10, "split"), Against(0, 4, "split"), Against(6, 10, "split"),
            Target(0, 4, "merge"), Target(6, 10, "merge"), Against(0, 10, "merge"),
            Target(0, 4, "complex"), Target(6, 10, "complex"),
            Against(0, 6, "complex"), Against(7, 10, "complex"),
            Target(0, 2, "unmatched-target"),
            Against(0, 2, "unmatched-against"))
            .Run();

        var summary = result.Summary;
        Assert.Equal(7, summary.TargetEpisodeCount);
        Assert.Equal(7, summary.AgainstEpisodeCount);
        Assert.Equal(6, summary.MatchedTargetEpisodeCount);
        Assert.Equal(6, summary.MatchedAgainstEpisodeCount);
        Assert.Equal(1, summary.UnmatchedTargetEpisodeCount);
        Assert.Equal(1, summary.UnmatchedAgainstEpisodeCount);
        Assert.Equal(1, summary.OneToOneRelationCount);
        Assert.Equal(1, summary.SplitRelationCount);
        Assert.Equal(1, summary.MergeRelationCount);
        Assert.Equal(1, summary.ComplexRelationCount);
        Assert.Equal(1, summary.SplitTargetEpisodeCount);
        Assert.Equal(1, summary.MergedAgainstEpisodeCount);
        Assert.Equal(1, summary.OnsetDeltaDistribution.Count);
    }

    [Fact]
    public void CoverageAndBiasUseComponentUnionsAndSetTotals()
    {
        var result = Compare(Target(0, 8), Against(2, 6)).Run();

        Assert.Equal(0, result.Summary.EpisodeCountBias);
        Assert.Equal(-4, result.Summary.ActiveMagnitudeBias);
        Assert.Equal(4, result.Summary.TotalOverlapMagnitude);
        Assert.Equal(0.5, result.Summary.TargetCoverageRatio);
        Assert.Equal(1, result.Summary.AgainstCoverageRatio);
        Assert.Equal(0.5, result.Summary.IntersectionOverUnion);
        Assert.Equal(1, result.Summary.TargetMatchRate);
        Assert.Equal(1, result.Summary.AgainstMatchRate);
    }

    [Fact]
    public void SignedDistributionsIncludeOnlyOneToOneComponents()
    {
        var result = Compare(
            Target(2, 6, "early-against"), Against(0, 4, "early-against"),
            Target(10, 15, "late-against"), Against(13, 18, "late-against"),
            Target(20, 30, "split"), Against(20, 24, "split"), Against(26, 30, "split"))
            .Run();

        Assert.Equal(2, result.Summary.OnsetDeltaDistribution.Count);
        Assert.Equal(-2, result.Summary.OnsetDeltaDistribution.Minimum);
        Assert.Equal(0.5, result.Summary.OnsetDeltaDistribution.Median);
        Assert.Equal(3, result.Summary.OnsetDeltaDistribution.Maximum);
        Assert.Equal(2, result.Summary.RecoveryDeltaDistribution.Count);
    }

    [Fact]
    public void MultiFragmentEpisodeIsDistinctFromRelationshipSplit()
    {
        var result = Compare(
            Target(0, 10),
            Against(0, 4),
            Against(6, 10))
            .StitchGapsUpTo(2)
            .Run();

        Assert.Equal(1, result.AgainstEpisodes.Summary.MultiFragmentEpisodeCount);
        Assert.Equal(1, result.AgainstEpisodes.Summary.MultiFragmentEpisodeRate);
        Assert.Equal(0, result.Summary.SplitRelationCount);
        Assert.Equal(0, result.Summary.SplitTargetRate);
    }

    [Fact]
    public void SmallerStitchToleranceProducesRelationshipSplit()
    {
        var result = Compare(
            Target(0, 10),
            Against(0, 4),
            Against(6, 10))
            .StitchGapsUpTo(1)
            .Run();

        Assert.Equal(0, result.AgainstEpisodes.Summary.MultiFragmentEpisodeCount);
        Assert.Equal(1, result.Summary.SplitRelationCount);
        Assert.Equal(1, result.Summary.SplitTargetRate);
        Assert.Equal(1, result.Summary.EpisodeCountBias);
        Assert.Equal(-2, result.Summary.ActiveMagnitudeBias);
    }

    [Fact]
    public void EmptyAndDisconnectedComparisonsDistinguishNullFromZero()
    {
        var empty = Compare().Run();
        var disconnected = Compare(Target(0, 2), Against(5, 7)).Run();

        Assert.Null(empty.Summary.TargetMatchRate);
        Assert.Null(empty.Summary.AgainstMatchRate);
        Assert.Null(empty.Summary.TargetCoverageRatio);
        Assert.Null(empty.Summary.IntersectionOverUnion);
        Assert.Equal(0, disconnected.Summary.TargetMatchRate);
        Assert.Equal(0, disconnected.Summary.AgainstMatchRate);
        Assert.Equal(0, disconnected.Summary.TargetCoverageRatio);
        Assert.Equal(0, disconnected.Summary.IntersectionOverUnion);
        Assert.Single(disconnected.UnmatchedTargetEpisodes());
        Assert.Single(disconnected.UnmatchedAgainstEpisodes());
        Assert.Single(disconnected.RelationsOfKind(EpisodeRelationKind.UnmatchedTarget));
    }

    private static EpisodeComparisonBuilder Compare(params ClosedWindow[] records)
    {
        return WindowHistory.FromRecords(records, [])
            .CompareEpisodes("State comparison")
            .Target("target", selector => selector.Source("target"))
            .Against("against", selector => selector.Source("against"))
            .Within(scope => scope.Window("State"));
    }

    private static ClosedWindow Target(long start, long end, string key = "device-1")
    {
        return new ClosedWindow("State", key, start, end, Source: "target");
    }

    private static ClosedWindow Against(long start, long end, string key = "device-1")
    {
        return new ClosedWindow("State", key, start, end, Source: "against");
    }
}
