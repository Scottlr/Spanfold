using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class CustomKeyComparerTests
{
    [Fact]
    public void WindowCanUseCustomKeyComparer()
    {
        var pipeline = Spanfold
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m,
                comparer: StringComparer.OrdinalIgnoreCase)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        Assert.Empty(result.Emissions);
    }

    [Fact]
    public void RecordedWindowClosesWithCustomKeyComparer()
    {
        var pipeline = Spanfold
            .For<PriceTick>()
            .RecordWindows()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m,
                comparer: StringComparer.OrdinalIgnoreCase)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m));
        pipeline.Ingest(new PriceTick("selection-1", 1m));

        Assert.Empty(pipeline.History.OpenWindows);
        Assert.Single(pipeline.History.ClosedWindows);
    }

    [Fact]
    public void WindowUsesDefaultComparerWhenCustomComparerIsOmitted()
    {
        var pipeline = Spanfold
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        var emission = Assert.Single(result.Emissions);
        Assert.Equal("selection-1", emission.Key);
    }

    private sealed record PriceTick(string SelectionId, decimal Price);
}
