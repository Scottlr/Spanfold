using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class RollUpDefinitionApiTests
{
    [Fact]
    public void RollUpCanBeDefinedAfterWindow()
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
            .Build();

        var window = Assert.Single(pipeline.Windows);
        var rollUp = Assert.Single(window.RollUps);
        Assert.Equal("MarketSuspension", rollUp.Name);
    }

    [Fact]
    public void RollUpRequiresName()
    {
        var builder = CreateWindowBuilder();

        Assert.Throws<ArgumentException>(() => builder.RollUp(
            "",
            key: tick => tick.MarketId,
            isActive: children => children.AllActive()));
    }

    [Fact]
    public void RollUpRequiresKeySelector()
    {
        var builder = CreateWindowBuilder();

        Assert.Throws<ArgumentNullException>(() => builder.RollUp<string>(
            "MarketSuspension",
            key: null!,
            isActive: children => children.AllActive()));
    }

    [Fact]
    public void RollUpRequiresActiveSelector()
    {
        var builder = CreateWindowBuilder();

        Assert.Throws<ArgumentNullException>(() => builder.RollUp(
            "MarketSuspension",
            key: tick => tick.MarketId,
            isActive: null!));
    }

    private static WindowPipelineBuilder<PriceTick> CreateWindowBuilder()
    {
        return EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m);
    }

    private sealed record PriceTick(string SelectionId, string MarketId, decimal Price);
}
