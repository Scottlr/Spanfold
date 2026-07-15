using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class BatchIngestionTests
{
    [Fact]
    public void IngestManyProcessesEventsInSourceOrder()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        var result = pipeline.IngestMany(
        [
            new PriceTick("selection-1", 0m),
            new PriceTick("selection-2", 0m),
            new PriceTick("selection-1", 1.01m)
        ]);

        Assert.Collection(
            result.Emissions,
            first =>
            {
                Assert.Equal("selection-1", first.Key);
                Assert.Equal(WindowTransitionKind.Opened, first.Kind);
            },
            second =>
            {
                Assert.Equal("selection-2", second.Key);
                Assert.Equal(WindowTransitionKind.Opened, second.Kind);
            },
            third =>
            {
                Assert.Equal("selection-1", third.Key);
                Assert.Equal(WindowTransitionKind.Closed, third.Kind);
            });
    }

    [Fact]
    public void IngestManyRequiresEvents()
    {
        var pipeline = EventPipeline.For<PriceTick>().Build();

        Assert.Throws<ArgumentNullException>(() => pipeline.IngestMany(null!));
    }

    private sealed record PriceTick(string SelectionId, decimal Price);
}
