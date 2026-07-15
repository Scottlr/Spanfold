using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class SingleWindowRuntimeTests
{
    [Fact]
    public void IngestOpensWindowWhenKeyBecomesActive()
    {
        var pipeline = CreatePipeline();

        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        var emission = Assert.Single(result.Emissions);
        Assert.Equal("SelectionSuspension", emission.WindowName);
        Assert.Equal("selection-1", emission.Key);
        Assert.Equal(WindowTransitionKind.Opened, emission.Kind);
    }

    [Fact]
    public void IngestDoesNotEmitWhenActiveKeyStaysActive()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        Assert.Empty(result.Emissions);
    }

    [Fact]
    public void IngestClosesWindowWhenKeyBecomesInactive()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", 1.01m));

        var emission = Assert.Single(result.Emissions);
        Assert.Equal("selection-1", emission.Key);
        Assert.Equal(WindowTransitionKind.Closed, emission.Kind);
    }

    [Fact]
    public void IngestDoesNotEmitWhenInactiveKeyStaysInactive()
    {
        var pipeline = CreatePipeline();

        var result = pipeline.Ingest(new PriceTick("selection-1", 1.01m));

        Assert.Empty(result.Emissions);
    }

    [Fact]
    public void IngestTracksStatePerKey()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceTick("selection-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-2", 0m));

        var emission = Assert.Single(result.Emissions);
        Assert.Equal("selection-2", emission.Key);
        Assert.Equal(WindowTransitionKind.Opened, emission.Kind);
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
