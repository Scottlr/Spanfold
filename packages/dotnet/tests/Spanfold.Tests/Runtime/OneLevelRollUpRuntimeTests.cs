using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class OneLevelRollUpRuntimeTests
{
    [Fact]
    public void RollUpOpensWhenAllKnownChildrenAreActive()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-2", "market-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-1", "market-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-2", "market-1", 0m));

        Assert.Collection(
            result.Emissions,
            child =>
            {
                Assert.Equal("SelectionSuspension", child.WindowName);
                Assert.Equal("selection-2", child.Key);
                Assert.Equal(WindowTransitionKind.Opened, child.Kind);
            },
            parent =>
            {
                Assert.Equal("MarketSuspension", parent.WindowName);
                Assert.Equal("market-1", parent.Key);
                Assert.Equal(WindowTransitionKind.Opened, parent.Kind);
            });
    }

    [Fact]
    public void RollUpClosesWhenAnyKnownChildBecomesInactive()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-2", "market-1", 1.01m));
        pipeline.Ingest(new PriceTick("selection-1", "market-1", 0m));
        pipeline.Ingest(new PriceTick("selection-2", "market-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", "market-1", 1.01m));

        Assert.Collection(
            result.Emissions,
            child =>
            {
                Assert.Equal("SelectionSuspension", child.WindowName);
                Assert.Equal(WindowTransitionKind.Closed, child.Kind);
            },
            parent =>
            {
                Assert.Equal("MarketSuspension", parent.WindowName);
                Assert.Equal("market-1", parent.Key);
                Assert.Equal(WindowTransitionKind.Closed, parent.Kind);
            });
    }

    [Fact]
    public void RollUpReevaluatesWhenAnInactiveChildBecomesKnown()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-2", "market-1", 1.01m));

        var parent = Assert.Single(result.Emissions);
        Assert.Equal("MarketSuspension", parent.WindowName);
        Assert.Equal(WindowTransitionKind.Closed, parent.Kind);
    }

    [Fact]
    public void ActiveChildMovesBetweenParentKeysAtomically()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", "market-2", 0m));

        Assert.Collection(
            result.Emissions,
            oldParent =>
            {
                Assert.Equal("market-1", oldParent.Key);
                Assert.Equal(WindowTransitionKind.Closed, oldParent.Kind);
            },
            newParent =>
            {
                Assert.Equal("market-2", newParent.Key);
                Assert.Equal(WindowTransitionKind.Opened, newParent.Kind);
            });
    }

    [Fact]
    public void RollUpStateIsPartitionedByParentKey()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-2", "market-2", 0m));

        Assert.Collection(
            result.Emissions,
            child =>
            {
                Assert.Equal("SelectionSuspension", child.WindowName);
                Assert.Equal("selection-2", child.Key);
            },
            parent =>
            {
                Assert.Equal("MarketSuspension", parent.WindowName);
                Assert.Equal("market-2", parent.Key);
            });
    }

    [Fact]
    public void RollUpUsesChildWindowKeyComparerForMembership()
    {
        var pipeline = Spanfold
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m,
                comparer: StringComparer.OrdinalIgnoreCase)
            .RollUp(
                "MarketSuspension",
                key: tick => tick.MarketId,
                isActive: children => children.AllActive())
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", "market-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", "market-1", 1.01m));

        Assert.Collection(
            result.Emissions,
            child => Assert.Equal(WindowTransitionKind.Closed, child.Kind),
            parent =>
            {
                Assert.Equal("MarketSuspension", parent.WindowName);
                Assert.Equal(WindowTransitionKind.Closed, parent.Kind);
            });
    }

    private static EventPipeline<PriceTick> CreatePipeline()
    {
        return Spanfold
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .RollUp(
                "MarketSuspension",
                key: tick => tick.MarketId,
                isActive: children => children.AllActive())
            .Build();
    }

    private sealed record PriceTick(string SelectionId, string MarketId, decimal Price);
}
