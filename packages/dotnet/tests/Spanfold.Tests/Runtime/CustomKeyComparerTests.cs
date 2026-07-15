using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class CustomKeyComparerTests
{
    [Fact]
    public void WindowCanUseCustomKeyComparer()
    {
        var pipeline = EventPipeline
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
        var pipeline = EventPipeline
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
        Assert.Single(pipeline.History.Query().Key("SELECTION-1").ClosedWindows());
    }

    [Fact]
    public void ComparisonAlignsComparerEquivalentKeysAcrossSources()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m,
                comparer: StringComparer.OrdinalIgnoreCase)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m), source: "provider-a");
        pipeline.Ingest(new PriceTick("selection-1", 0m), source: "provider-b");
        pipeline.Ingest(new PriceTick("Selection-1", 1m), source: "provider-a");
        pipeline.Ingest(new PriceTick("selection-1", 1m), source: "provider-b");

        var result = pipeline.History.Compare("Comparer alignment")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("SelectionSuspension"))
            .Using(comparators => comparators.Overlap())
            .Run();

        Assert.Single(result.OverlapRows);
    }

    [Fact]
    public void WindowUsesDefaultComparerWhenCustomComparerIsOmitted()
    {
        var pipeline = EventPipeline
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
