using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class SourceAwareIngestionTests
{
    [Fact]
    public void IngestCanAttachSourceToEmissions()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        var result = pipeline.Ingest(new PriceTick("selection-1", 0m), source: "provider-a");

        var emission = Assert.Single(result.Emissions);
        Assert.Equal("provider-a", emission.Source);
    }

    [Fact]
    public void IngestWithoutSourceLeavesEmissionSourceEmpty()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        var emission = Assert.Single(result.Emissions);
        Assert.Null(emission.Source);
    }

    [Fact]
    public void SourceContextOwnsIndependentRuntimeState()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .TrackWindow(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m);

        pipeline.Ingest(new PriceTick("selection-1", 0m), source: "provider-a");
        pipeline.Ingest(new PriceTick("selection-1", 0m), source: "provider-b");
        pipeline.Ingest(new PriceTick("selection-1", 1.01m), source: "provider-b");
        pipeline.Ingest(new PriceTick("selection-1", 1.01m), source: "provider-a");

        var result = pipeline.History.Compare("Provider QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("SelectionSuspension"))
            .Using(comparators => comparators.Overlap().Residual())
            .Run();

        Assert.Single(result.OverlapRows);
        Assert.Equal(2, result.ResidualRows.Count);
    }

    private sealed record PriceTick(string SelectionId, decimal Price);
}
