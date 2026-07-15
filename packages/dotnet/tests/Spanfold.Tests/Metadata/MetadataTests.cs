using Spanfold;

namespace Spanfold.Tests.Metadata;

public sealed class MetadataTests
{
    [Fact]
    public void MetadataExposesEventTypeAndWindowNames()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        Assert.Equal(typeof(PriceTick), pipeline.Metadata.EventType);
        Assert.Equal("SelectionSuspension", Assert.Single(pipeline.Metadata.Windows).Name);
    }

    [Fact]
    public void MetadataExposesRollUpHierarchy()
    {
        var pipeline = EventPipeline
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

        var selection = Assert.Single(pipeline.Metadata.Windows);
        var market = Assert.Single(selection.RollUps);
        var fixture = Assert.Single(market.RollUps);

        Assert.Equal("MarketSuspension", market.Name);
        Assert.Equal("FixtureSuspension", fixture.Name);
    }

    private sealed record PriceTick(
        string SelectionId,
        string MarketId,
        string FixtureId,
        decimal Price);
}
