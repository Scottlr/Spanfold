using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class ValidationTests
{
    [Fact]
    public void WindowNamesMustBeUnique()
    {
        var builder = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m);

        var exception = Assert.Throws<InvalidOperationException>(() => builder.Window(
            "SelectionSuspension",
            key: tick => tick.MarketId,
            isActive: tick => tick.Price == 0m));

        Assert.Contains("SelectionSuspension", exception.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void RollUpNameCannotDuplicateWindowName()
    {
        var builder = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m);

        Assert.Throws<InvalidOperationException>(() => builder.RollUp(
            "SelectionSuspension",
            key: tick => tick.MarketId,
            isActive: children => children.AllActive()));
    }

    [Fact]
    public void WindowKeyCannotBeNull()
    {
        var pipeline = EventPipeline
            .For<NullableKeyTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId!,
                isActive: tick => tick.Price == 0m)
            .Build();

        var exception = Assert.Throws<InvalidOperationException>(() =>
            pipeline.Ingest(new NullableKeyTick(null, "market-1", 0m)));

        Assert.Contains("SelectionSuspension", exception.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void RollUpKeyCannotBeNull()
    {
        var pipeline = EventPipeline
            .For<NullableKeyTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId!,
                isActive: tick => tick.Price == 0m)
            .RollUp(
                "MarketSuspension",
                key: tick => tick.MarketId!,
                isActive: children => children.AllActive())
            .Build();

        var exception = Assert.Throws<InvalidOperationException>(() =>
            pipeline.Ingest(new NullableKeyTick("selection-1", null, 0m)));

        Assert.Contains("MarketSuspension", exception.Message, StringComparison.Ordinal);
    }

    private sealed record PriceTick(string SelectionId, string MarketId, decimal Price);

    private sealed record NullableKeyTick(string? SelectionId, string? MarketId, decimal Price);
}
