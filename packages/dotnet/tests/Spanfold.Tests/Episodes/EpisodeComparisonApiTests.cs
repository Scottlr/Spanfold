using Spanfold;

namespace Spanfold.Tests.Episodes;

public sealed class EpisodeComparisonApiTests
{
    [Fact]
    public void StagedConsumerJourneyReturnsStructuredRelationGraph()
    {
        var history = WindowHistory.FromRecords(
            [
                new ClosedWindow("Outage", "device-1", 0, 5, Source: "provider-a"),
                new ClosedWindow("Outage", "device-1", 1, 6, Source: "provider-b")
            ],
            []);

        var result = history.CompareEpisodes("Provider outage QA")
            .Target("reference", selector => selector.Source("provider-a"))
            .Against("detector", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("Outage"))
            .StitchGapsUpTo(1L)
            .RelateWithin(1L)
            .Run();

        Assert.Equal("Provider outage QA", result.Name);
        Assert.Equal("reference", result.TargetEpisodes.Name);
        Assert.Equal("detector", result.AgainstEpisodes.Name);
        Assert.Single(result.TargetEpisodes.Episodes);
        Assert.Single(result.AgainstEpisodes.Episodes);
        Assert.Equal(EpisodeRelationKind.OneToOne, Assert.Single(result.Relations).Kind);
        Assert.Null(result.EvaluationHorizon);
    }

    [Fact]
    public void BuilderRejectsASecondAgainstSide()
    {
        var history = WindowHistory.FromRecords([], []);
        var builder = history.CompareEpisodes("Provider QA")
            .Target("reference", selector => selector.Source("provider-a"))
            .Against("detector", selector => selector.Source("provider-b"));

        Assert.Throws<InvalidOperationException>(() =>
            builder.Against("third", selector => selector.Source("provider-c")));
    }
}
