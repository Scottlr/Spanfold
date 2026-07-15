using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class PartitionAwareRuntimeTests
{
    [Fact]
    public void SameLogicalKeyOpensIndependentlyAcrossPartitions()
    {
        var pipeline = CreatePipeline();

        var first = pipeline.Ingest(
            new PriceTick("selection-1", 0m),
            source: null,
            partition: "partition-a");
        var second = pipeline.Ingest(
            new PriceTick("selection-1", 0m),
            source: null,
            partition: "partition-b");

        Assert.Equal(WindowTransitionKind.Opened, Assert.Single(first.Emissions).Kind);

        var emission = Assert.Single(second.Emissions);
        Assert.Equal(WindowTransitionKind.Opened, emission.Kind);
        Assert.Equal("partition-b", emission.Partition);
    }

    [Fact]
    public void ClosingOnePartitionDoesNotCloseAnotherPartition()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", 0m), source: null, partition: "partition-a");
        pipeline.Ingest(new PriceTick("selection-1", 0m), source: null, partition: "partition-b");
        var close = pipeline.Ingest(
            new PriceTick("selection-1", 1.01m),
            source: null,
            partition: "partition-a");
        var stillActive = pipeline.Ingest(
            new PriceTick("selection-1", 0m),
            source: null,
            partition: "partition-b");

        var closeEmission = Assert.Single(close.Emissions);
        Assert.Equal(WindowTransitionKind.Closed, closeEmission.Kind);
        Assert.Equal("partition-a", closeEmission.Partition);
        Assert.Empty(stillActive.Emissions);
    }

    private static EventPipeline<PriceTick> CreatePipeline()
    {
        return EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();
    }

    private sealed record PriceTick(string SelectionId, decimal Price);
}
