using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class EmissionCallbackTests
{
    [Fact]
    public void CallbackReceivesOpenAndCloseEmissions()
    {
        var callbackEmissions = new List<WindowEmission<PriceTick>>();
        var pipeline = Spanfold
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .OnEmission(callbackEmissions.Add)
            .Build();

        pipeline.Ingest(new PriceTick("selection-1", 0m));
        pipeline.Ingest(new PriceTick("selection-1", 1.01m));

        Assert.Collection(
            callbackEmissions,
            opened => Assert.Equal(WindowTransitionKind.Opened, opened.Kind),
            closed => Assert.Equal(WindowTransitionKind.Closed, closed.Kind));
    }

    [Fact]
    public void CallbackEmissionsMatchReturnedEmissions()
    {
        var callbackEmissions = new List<WindowEmission<PriceTick>>();
        var pipeline = Spanfold
            .For<PriceTick>()
            .OnEmission(callbackEmissions.Add)
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        var returned = Assert.Single(result.Emissions);
        var pushed = Assert.Single(callbackEmissions);
        Assert.Equal(returned, pushed);
    }

    [Fact]
    public void CallbackIsRequired()
    {
        var builder = Spanfold.For<PriceTick>();

        Assert.Throws<ArgumentNullException>(() => builder.OnEmission(null!));
    }

    [Fact]
    public void CallbackFailureReportsCommittedResultAndRunsRemainingCallbacks()
    {
        var callbacks = 0;
        var pipeline = Spanfold
            .For<PriceTick>()
            .OnEmission(_ => throw new InvalidOperationException("first"))
            .OnEmission(_ => callbacks++)
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        var exception = Assert.Throws<IngestionCallbackException<PriceTick>>(() =>
            pipeline.Ingest(new PriceTick("selection-1", 0m)));

        Assert.Single(exception.Result.Emissions);
        Assert.Equal(1, callbacks);
    }

    private sealed record PriceTick(string SelectionId, decimal Price);
}
