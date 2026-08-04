using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class StabilizedWindowRuntimeTests
{
    [Fact]
    public void FlappingPredicatesRequireConsecutiveAsymmetricConfirmation()
    {
        var pipeline = CreatePipeline(enterAfter: 2, exitAfter: 3);

        Assert.False(pipeline.Ingest(new Signal(Enter: true, Exit: false)).HasEmissions);
        Assert.False(pipeline.Ingest(new Signal(Enter: false, Exit: false)).HasEmissions);
        Assert.False(pipeline.Ingest(new Signal(Enter: true, Exit: false)).HasEmissions);
        var opened = pipeline.Ingest(new Signal(Enter: true, Exit: true));
        Assert.False(pipeline.Ingest(new Signal(Enter: true, Exit: true)).HasEmissions);
        Assert.False(pipeline.Ingest(new Signal(Enter: false, Exit: true)).HasEmissions);
        Assert.False(pipeline.Ingest(new Signal(Enter: false, Exit: false)).HasEmissions);
        Assert.False(pipeline.Ingest(new Signal(Enter: false, Exit: true)).HasEmissions);
        Assert.False(pipeline.Ingest(new Signal(Enter: false, Exit: true)).HasEmissions);
        var closed = pipeline.Ingest(new Signal(Enter: false, Exit: true));

        Assert.Equal(WindowTransitionKind.Opened, Assert.Single(opened.Emissions).Kind);
        Assert.Equal(WindowTransitionKind.Closed, Assert.Single(closed.Emissions).Kind);
        var window = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Equal(4, window.StartPosition);
        Assert.Equal(10, window.EndPosition);
    }

    [Fact]
    public void ConfirmationBoundariesUseCommittedMetadataRules()
    {
        var pipeline = CreatePipeline(enterAfter: 2, exitAfter: 2);

        pipeline.Ingest(new Signal(Enter: true, Exit: false, Segment: "candidate", Tag: "candidate"));
        pipeline.Ingest(new Signal(Enter: true, Exit: false, Segment: "confirmed", Tag: "confirmed"));
        pipeline.Ingest(new Signal(Enter: false, Exit: true, Segment: "ignored", Tag: "ignored"));

        var pendingExit = Assert.Single(pipeline.History.OpenWindows);
        Assert.Equal("confirmed", Assert.Single(pendingExit.Segments).Value);
        Assert.Equal("confirmed", Assert.Single(pendingExit.Tags).Value);

        var resumed = pipeline.Ingest(
            new Signal(Enter: false, Exit: false, Segment: "resumed", Tag: "resumed"));

        Assert.Equal(
            [WindowTransitionKind.Closed, WindowTransitionKind.Opened],
            resumed.Emissions.Select(emission => emission.Kind).ToArray());
        Assert.Equal(4, Assert.Single(pipeline.History.OpenWindows).StartPosition);
    }

    [Fact]
    public void PendingConfirmationRollsBackWhenLaterWindowFails()
    {
        var pipeline = EventPipeline
            .For<Signal>()
            .RecordWindows()
            .Window(
                "Stable",
                signal => signal.Key,
                signal => signal.Enter,
                options => options.Stabilize(signal => signal.Exit, enterAfter: 2))
            .Window(
                "Failure",
                signal => signal.Key,
                signal => signal.ShouldThrow
                    ? throw new InvalidOperationException("selector failed")
                    : false)
            .Build();

        Assert.Throws<InvalidOperationException>(() =>
            pipeline.Ingest(new Signal(Enter: true, Exit: false, ShouldThrow: true)));
        Assert.False(pipeline.Ingest(new Signal(Enter: true, Exit: false)).HasEmissions);
        var opened = pipeline.Ingest(new Signal(Enter: true, Exit: false));

        Assert.Equal("Stable", Assert.Single(opened.Emissions).WindowName);
        Assert.Equal(2, Assert.Single(pipeline.History.OpenWindows).StartPosition);
    }

    [Fact]
    public void RollUpsAndCallbacksObserveOnlyConfirmedTransitions()
    {
        var windowCallbacks = new List<WindowTransitionKind>();
        var globalCallbacks = new List<string>();
        var pipeline = EventPipeline
            .For<Signal>()
            .RecordWindows()
            .OnEmission(emission => globalCallbacks.Add(emission.WindowName))
            .Window(
                "Stable",
                signal => signal.Key,
                signal => signal.Enter,
                options => options
                    .Stabilize(signal => signal.Exit, enterAfter: 2, exitAfter: 2)
                    .OnOpened(emission => windowCallbacks.Add(emission.Kind))
                    .OnClosed(emission => windowCallbacks.Add(emission.Kind)))
            .RollUp("AnyStable", signal => signal.Parent, children => children.AnyActive())
            .Build();

        Assert.False(pipeline.Ingest(new Signal(Enter: true, Exit: false)).HasEmissions);
        Assert.Empty(windowCallbacks);
        Assert.Empty(globalCallbacks);

        var opened = pipeline.Ingest(new Signal(Enter: true, Exit: false));
        Assert.Equal(["Stable", "AnyStable"], opened.Emissions.Select(emission => emission.WindowName));

        Assert.False(pipeline.Ingest(new Signal(Enter: false, Exit: true)).HasEmissions);
        var closed = pipeline.Ingest(new Signal(Enter: false, Exit: true));

        Assert.Equal(["Stable", "AnyStable"], closed.Emissions.Select(emission => emission.WindowName));
        Assert.Equal([WindowTransitionKind.Opened, WindowTransitionKind.Closed], windowCallbacks);
        Assert.Equal(["Stable", "AnyStable", "Stable", "AnyStable"], globalCallbacks);
    }

    [Theory]
    [InlineData(0, 1)]
    [InlineData(1, 0)]
    public void ConfirmationCountsMustBePositive(int enterAfter, int exitAfter)
    {
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            EventPipeline
                .For<Signal>()
                .Window(
                    "Stable",
                    signal => signal.Key,
                    signal => signal.Enter,
                    options => options.Stabilize(
                        signal => signal.Exit,
                        enterAfter,
                        exitAfter)));
    }

    private static EventPipeline<Signal> CreatePipeline(int enterAfter, int exitAfter)
    {
        return EventPipeline
            .For<Signal>()
            .RecordWindows()
            .TrackWindow("Stable", window => window
                .Key(signal => signal.Key)
                .ActiveWhen(signal => signal.Enter)
                .Stabilize(signal => signal.Exit, enterAfter, exitAfter)
                .Segment("state", segment => segment.Value(signal => signal.Segment))
                .Tag("label", signal => signal.Tag));
    }

    private sealed record Signal(
        bool Enter,
        bool Exit,
        string Key = "item-1",
        string Parent = "parent-1",
        string Segment = "steady",
        string Tag = "current",
        bool ShouldThrow = false);
}
