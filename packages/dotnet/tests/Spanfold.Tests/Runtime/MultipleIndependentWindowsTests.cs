using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class MultipleIndependentWindowsTests
{
    [Fact]
    public void IngestProcessesIndependentWindowsInDefinitionOrder()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Window(
                "MarketHalt",
                key: tick => tick.MarketId,
                isActive: tick => tick.MarketHalted)
            .Build();

        var result = pipeline.Ingest(new PriceTick(
            SelectionId: "selection-1",
            MarketId: "market-1",
            Price: 0m,
            MarketHalted: true));

        Assert.Collection(
            result.Emissions,
            first =>
            {
                Assert.Equal("SelectionSuspension", first.WindowName);
                Assert.Equal("selection-1", first.Key);
            },
            second =>
            {
                Assert.Equal("MarketHalt", second.WindowName);
                Assert.Equal("market-1", second.Key);
            });
    }

    [Fact]
    public void IndependentWindowsKeepSeparateState()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Window(
                "MarketHalt",
                key: tick => tick.MarketId,
                isActive: tick => tick.MarketHalted)
            .Build();

        pipeline.Ingest(new PriceTick(
            SelectionId: "selection-1",
            MarketId: "market-1",
            Price: 0m,
            MarketHalted: false));

        var result = pipeline.Ingest(new PriceTick(
            SelectionId: "selection-1",
            MarketId: "market-1",
            Price: 0m,
            MarketHalted: true));

        var emission = Assert.Single(result.Emissions);
        Assert.Equal("MarketHalt", emission.WindowName);
        Assert.Equal(WindowTransitionKind.Opened, emission.Kind);
    }

    private sealed record PriceTick(
        string SelectionId,
        string MarketId,
        decimal Price,
        bool MarketHalted);
}
