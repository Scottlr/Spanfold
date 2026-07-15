using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class BuildPipelineTests
{
    [Fact]
    public void BuildCreatesEmptyPipeline()
    {
        var pipeline = EventPipeline.For<PriceTick>().Build();

        Assert.Empty(pipeline.Windows);
    }

    [Fact]
    public void BuildPreservesConfiguredWindowDefinitions()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        var definition = Assert.Single(pipeline.Windows);
        Assert.Equal("SelectionSuspension", definition.Name);
    }

    private sealed record PriceTick(string SelectionId, decimal Price);
}
