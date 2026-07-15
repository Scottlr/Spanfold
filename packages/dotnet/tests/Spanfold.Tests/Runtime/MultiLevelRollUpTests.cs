using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class MultiLevelRollUpTests
{
    [Fact]
    public void RollUpCanFeedAnotherRollUp()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", "fixture-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-2", "market-2", "fixture-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-1", "market-1", "fixture-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-2", "market-2", "fixture-1", 0m));

        Assert.Collection(
            result.Emissions,
            selection => Assert.Equal("SelectionSuspension", selection.WindowName),
            market => Assert.Equal("MarketSuspension", market.WindowName),
            fixture =>
            {
                Assert.Equal("FixtureSuspension", fixture.WindowName);
                Assert.Equal("fixture-1", fixture.Key);
                Assert.Equal(WindowTransitionKind.Opened, fixture.Kind);
            });
    }

    [Fact]
    public void RollUpClosingPropagatesUpward()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", "fixture-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-2", "market-2", "fixture-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-1", "market-1", "fixture-1", 0m));
        pipeline.Ingest(new PriceTick("selection-2", "market-2", "fixture-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", "market-1", "fixture-1", 1.01m));

        Assert.Collection(
            result.Emissions,
            selection => Assert.Equal("SelectionSuspension", selection.WindowName),
            market => Assert.Equal("MarketSuspension", market.WindowName),
            fixture =>
            {
                Assert.Equal("FixtureSuspension", fixture.WindowName);
                Assert.Equal(WindowTransitionKind.Closed, fixture.Kind);
            });
    }

    private static EventPipeline<PriceTick> CreatePipeline()
    {
        return EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .RollUp(
                "MarketSuspension",
                key: tick => tick.MarketId,
                isActive: children => children.AllActive())
            .RollUp(
                "FixtureSuspension",
                key: tick => tick.FixtureId,
                isActive: children => children.AllActive())
            .Build();
    }

    private sealed record PriceTick(
        string SelectionId,
        string MarketId,
        string FixtureId,
        decimal Price);
}
