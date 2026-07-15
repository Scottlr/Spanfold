using Spanfold;

namespace Spanfold.Tests.Episodes;

public sealed class EpisodeRelationTests
{
    [Fact]
    public void OneToOneComponentIsClassified()
    {
        var relation = Assert.Single(Compare(
            Target(0, 5),
            Against(1, 4)).Run().Relations);

        Assert.Equal(EpisodeRelationKind.OneToOne, relation.Kind);
    }

    [Fact]
    public void SplitComponentIsDirectional()
    {
        var relation = Assert.Single(Compare(
            Target(0, 10),
            Against(0, 4),
            Against(6, 10)).Run().Relations);

        Assert.Equal(EpisodeRelationKind.Split, relation.Kind);
        Assert.Single(relation.TargetEpisodes);
        Assert.Equal(2, relation.AgainstEpisodes.Count);
    }

    [Fact]
    public void MergeComponentIsDirectional()
    {
        var relation = Assert.Single(Compare(
            Target(0, 4),
            Target(6, 10),
            Against(0, 10)).Run().Relations);

        Assert.Equal(EpisodeRelationKind.Merge, relation.Kind);
        Assert.Equal(2, relation.TargetEpisodes.Count);
        Assert.Single(relation.AgainstEpisodes);
    }

    [Fact]
    public void ChainedManyToManyGraphRemainsOneComplexComponent()
    {
        var result = Compare(
            Target(0, 4),
            Target(6, 10),
            Against(0, 6),
            Against(7, 10))
            .RelateWithin(0)
            .Run();

        var relation = Assert.Single(result.Relations);
        Assert.Equal(EpisodeRelationKind.Complex, relation.Kind);
        Assert.Equal(2, relation.TargetEpisodes.Count);
        Assert.Equal(2, relation.AgainstEpisodes.Count);
        Assert.Equal(4, relation.TargetEpisodes.Count + relation.AgainstEpisodes.Count);
    }

    [Fact]
    public void IsolatedEpisodesUseBothUnmatchedKinds()
    {
        var relations = Compare(
            Target(0, 2),
            Against(5, 7)).Run().Relations;

        Assert.Equal(2, relations.Count);
        Assert.Contains(relations, relation => relation.Kind == EpisodeRelationKind.UnmatchedTarget);
        Assert.Contains(relations, relation => relation.Kind == EpisodeRelationKind.UnmatchedAgainst);
    }

    [Fact]
    public void ExactProximityToleranceRelatesAndOneUnitLessDoesNot()
    {
        var atTolerance = Compare(Target(0, 2), Against(4, 6))
            .RelateWithin(2)
            .Run();
        var beyondTolerance = Compare(Target(0, 2), Against(4, 6))
            .RelateWithin(1)
            .Run();

        Assert.Equal(EpisodeRelationKind.OneToOne, Assert.Single(atTolerance.Relations).Kind);
        Assert.Equal(2, atTolerance.Relations[0].Metrics.MinimumGapMagnitude);
        Assert.Equal(2, beyondTolerance.Relations.Count);
    }

    [Fact]
    public void EnvelopeOnlyOverlapDoesNotCreateAnEdge()
    {
        var result = Compare(
            Target(0, 2),
            Target(8, 10),
            Against(4, 6))
            .StitchGapsUpTo(6)
            .RelateWithin(1)
            .Run();

        Assert.Single(result.TargetEpisodes.Episodes);
        Assert.Equal(2, result.Relations.Count);
        Assert.DoesNotContain(result.Relations, relation => relation.Kind == EpisodeRelationKind.OneToOne);
    }

    [Fact]
    public void ComponentMetricsUseFragmentUnionsWithoutDoubleCounting()
    {
        var relation = Assert.Single(Compare(
            Target(0, 5),
            Target(3, 8),
            Against(2, 6)).Run().Relations);

        Assert.Equal(8, relation.Metrics.TargetActiveMagnitude);
        Assert.Equal(4, relation.Metrics.AgainstActiveMagnitude);
        Assert.Equal(4, relation.Metrics.OverlapMagnitude);
        Assert.Equal(0.5, relation.Metrics.TargetCoverageRatio);
        Assert.Equal(1, relation.Metrics.AgainstCoverageRatio);
        Assert.Equal(0.5, relation.Metrics.IntersectionOverUnion);
        Assert.Equal(0, relation.Metrics.MinimumGapMagnitude);
        Assert.Equal(2, relation.Metrics.OnsetDeltaMagnitude);
        Assert.Equal(-2, relation.Metrics.RecoveryDeltaMagnitude);
        Assert.Equal(-4, relation.Metrics.ActiveMagnitudeDelta);
        Assert.Equal(-4, relation.Metrics.ElapsedMagnitudeDelta);
    }

    [Fact]
    public void ZeroMagnitudeCoverageRatiosAreNull()
    {
        var relation = Assert.Single(Compare(Target(5, 5), Against(5, 5)).Run().Relations);

        Assert.Null(relation.Metrics.TargetCoverageRatio);
        Assert.Null(relation.Metrics.AgainstCoverageRatio);
        Assert.Null(relation.Metrics.IntersectionOverUnion);
        Assert.Equal(0, relation.Metrics.MinimumGapMagnitude);
    }

    [Fact]
    public void ConfiguredCustomKeyEqualityRelatesEquivalentKeys()
    {
        var history = CustomComparerHistory();

        var relation = Assert.Single(Builder(history).Run().Relations);

        Assert.Equal(EpisodeRelationKind.OneToOne, relation.Kind);
        Assert.Equal("Device-A", Assert.Single(relation.TargetEpisodes).Key);
        Assert.Equal("device-a", Assert.Single(relation.AgainstEpisodes).Key);
    }

    [Fact]
    public void IncompatibleTimestampClocksRemainUnmatched()
    {
        var epoch = DateTimeOffset.UnixEpoch;
        var history = WindowHistory.FromRecords(
            [
                new ClosedWindow("State", "device-1", 0, 1, Source: "target", StartTime: epoch, EndTime: epoch.AddMinutes(1), TimestampClock: "clock-a"),
                new ClosedWindow("State", "device-1", 0, 1, Source: "against", StartTime: epoch, EndTime: epoch.AddMinutes(1), TimestampClock: "clock-b")
            ],
            []);

        var relations = Builder(history)
            .Within(scope => scope.Window("State", TemporalAxis.Timestamp))
            .Normalize(normalization => normalization.OnEventTime())
            .RelateWithin(TimeSpan.Zero)
            .Run()
            .Relations;

        Assert.Equal(2, relations.Count);
        Assert.Contains(relations, relation => relation.Kind == EpisodeRelationKind.UnmatchedTarget);
        Assert.Contains(relations, relation => relation.Kind == EpisodeRelationKind.UnmatchedAgainst);
    }

    [Fact]
    public void RelationOrderingIsStableAcrossHistoryInsertionOrder()
    {
        var records = new[]
        {
            Target(20, 25, key: "device-2"),
            Against(21, 24, key: "device-2"),
            Target(0, 5, key: "device-1"),
            Against(1, 4, key: "device-1")
        };
        var forward = Builder(WindowHistory.FromRecords(records, [])).Run();
        var reversed = Builder(WindowHistory.FromRecords(records.Reverse(), [])).Run();

        Assert.Equal(
            forward.Relations.Select(RelationIdentity),
            reversed.Relations.Select(RelationIdentity));
    }

    [Fact]
    public void OverlappingSelectorLineageIsRejected()
    {
        var history = WindowHistory.FromRecords([Target(0, 5)], []);
        var builder = history.CompareEpisodes("Self match")
            .Target("target", selector => selector.Runtime("target", "all", static _ => true))
            .Against("against", selector => selector.Runtime("against", "all", static _ => true))
            .Within(scope => scope.Window("State"));

        var exception = Assert.Throws<InvalidOperationException>(() => builder.Run());
        Assert.Contains(history.Windows[0].Id.Value, exception.Message);
        Assert.Contains("target", exception.Message);
        Assert.Contains("against", exception.Message);
    }

    [Fact]
    public void ExtremePositionDeltasDoNotWrap()
    {
        var relation = Assert.Single(Compare(
            Target(long.MaxValue - 10, long.MaxValue - 5),
            Against(0, 5))
            .RelateWithin(long.MaxValue)
            .Run()
            .Relations);

        Assert.Equal(-(long.MaxValue - 10), relation.Metrics.OnsetDeltaMagnitude);
        Assert.Equal(-(long.MaxValue - 10), relation.Metrics.RecoveryDeltaMagnitude);
    }

    [Fact]
    public void RelationSettlesStrictlyAfterItsToleranceBoundary()
    {
        var history = WindowHistory.FromRecords([Target(0, 5), Against(0, 5)], []);
        var before = Builder(history).RelateWithin(2).RunLive(TemporalPoint.ForPosition(6));
        var atBoundary = Builder(history).RelateWithin(2).RunLive(TemporalPoint.ForPosition(7));
        var afterBoundary = Builder(history).RelateWithin(2).RunLive(TemporalPoint.ForPosition(8));

        Assert.Equal(ComparisonFinality.Provisional, Assert.Single(before.Relations).Finality);
        Assert.Equal(ComparisonFinality.Provisional, Assert.Single(atBoundary.Relations).Finality);
        Assert.Equal(ComparisonFinality.Final, Assert.Single(afterBoundary.Relations).Finality);
    }

    private static EpisodeComparisonBuilder Compare(params ClosedWindow[] records)
    {
        return Builder(WindowHistory.FromRecords(records, []));
    }

    private static EpisodeComparisonBuilder Builder(WindowHistory history)
    {
        return history.CompareEpisodes("State comparison")
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

    private static string RelationIdentity(EpisodeRelation relation)
    {
        return relation.Kind + ":"
            + string.Join(",", relation.TargetEpisodes.Select(episode => episode.Id.Value))
            + ":"
            + string.Join(",", relation.AgainstEpisodes.Select(episode => episode.Id.Value));
    }

    private static WindowHistory CustomComparerHistory()
    {
        var comparers = new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal)
        {
            ["State"] = new ObjectStringComparer(StringComparer.OrdinalIgnoreCase)
        };
        var history = new WindowHistory(enabled: true, comparers);
        history.Record(
            [new WindowEmission<int>("State", "Device-A", 0, WindowTransitionKind.Opened, "target")],
            0,
            eventTime: null);
        history.Record(
            [new WindowEmission<int>("State", "device-a", 0, WindowTransitionKind.Opened, "against")],
            1,
            eventTime: null);
        history.Record(
            [new WindowEmission<int>("State", "device-a", 0, WindowTransitionKind.Closed, "against")],
            4,
            eventTime: null);
        history.Record(
            [new WindowEmission<int>("State", "Device-A", 0, WindowTransitionKind.Closed, "target")],
            5,
            eventTime: null);
        return history;
    }

    private sealed class ObjectStringComparer(StringComparer comparer) : IEqualityComparer<object>
    {
        public new bool Equals(object? x, object? y)
        {
            return x is string left && y is string right && comparer.Equals(left, right);
        }

        public int GetHashCode(object obj)
        {
            return comparer.GetHashCode((string)obj);
        }
    }
}
