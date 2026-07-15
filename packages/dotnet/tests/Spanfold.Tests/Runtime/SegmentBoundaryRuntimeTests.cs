using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class SegmentBoundaryRuntimeTests
{
    [Fact]
    public void SegmentValueChangeClosesAndReopensActiveWindow()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)));

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "Pregame", Period: null));
        var transition = pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay", "FirstHalf"));

        Assert.Equal(
            [WindowTransitionKind.Closed, WindowTransitionKind.Opened],
            transition.Emissions.Select(emission => emission.Kind).ToArray());
        Assert.Equal(2, pipeline.History.OpenWindows.Single().StartPosition);

        var closed = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Equal(1, closed.StartPosition);
        Assert.Equal(2, closed.EndPosition);
        Assert.Equal(WindowBoundaryReason.SegmentChanged, closed.BoundaryReason);
        Assert.Equal("phase", Assert.Single(closed.BoundaryChanges).SegmentName);
        Assert.Equal("Pregame", Assert.Single(closed.Segments).Value);

        var open = Assert.Single(pipeline.History.OpenWindows);
        Assert.Equal("InPlay", Assert.Single(open.Segments).Value);
    }

    [Fact]
    public void ActivePredicateCloseRecordsPredicateEndedBoundaryReason()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)));

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay", "FirstHalf"));
        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: false, "InPlay", "FirstHalf"));

        var closed = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Equal(WindowBoundaryReason.ActivePredicateEnded, closed.BoundaryReason);
        Assert.Empty(closed.BoundaryChanges);
    }

    [Fact]
    public void NestedSegmentChangeSplitsCurrentWindow()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", PhaseWithPeriod));

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay", "FirstHalf"));
        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay", "SecondHalf"));

        var closed = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Equal("period", closed.Segments[1].Name);
        Assert.Equal("FirstHalf", closed.Segments[1].Value);
        Assert.Equal("phase", closed.Segments[1].ParentName);

        var open = Assert.Single(pipeline.History.OpenWindows);
        Assert.Equal("SecondHalf", open.Segments[1].Value);
    }

    [Fact]
    public void SameSegmentWhileActiveDoesNotEmitTransition()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)));

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay", "FirstHalf"));
        var second = pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay", "FirstHalf"));

        Assert.False(second.HasEmissions);
        Assert.Empty(pipeline.History.ClosedWindows);
        Assert.Single(pipeline.History.OpenWindows);
    }

    [Fact]
    public void TagChangesDoNotSplitActiveWindow()
    {
        var pipeline = EventPipeline
            .For<TaggedPriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase))
                .Tag("riskTier", update => update.RiskTier));

        pipeline.Ingest(new TaggedPriceUpdate("selection-1", HasPrice: true, "InPlay", "low"));
        var second = pipeline.Ingest(new TaggedPriceUpdate("selection-1", HasPrice: true, "InPlay", "high"));

        Assert.False(second.HasEmissions);
        var open = Assert.Single(pipeline.History.OpenWindows);
        Assert.Equal("low", Assert.Single(open.Tags).Value);
    }

    [Fact]
    public void MissingSegmentValueSelectorIsRejected()
    {
        var exception = Assert.Throws<InvalidOperationException>(() =>
            EventPipeline
                .For<PriceUpdate>()
                .TrackWindow("SelectionPriced", window => window
                    .Key(update => update.SelectionId)
                    .ActiveWhen(update => update.HasPrice)
                    .Segment("phase", _ => { })));

        Assert.Contains("must configure a value selector", exception.Message);
    }

    [Fact]
    public void DuplicateSegmentNamesAreRejected()
    {
        var exception = Assert.Throws<InvalidOperationException>(() =>
            EventPipeline
                .For<PriceUpdate>()
                .TrackWindow("SelectionPriced", window => window
                    .Key(update => update.SelectionId)
                    .ActiveWhen(update => update.HasPrice)
                    .Segment("phase", phase => phase.Value(update => update.Phase))
                    .Segment("phase", phase => phase.Value(update => update.Period))));

        Assert.Contains("has already been configured", exception.Message);
    }

    private static void PhaseWithPeriod(SegmentBuilder<PriceUpdate> phase)
    {
        phase
            .Value(update => update.Phase)
            .Child("period", period => period.Value(update => update.Period));
    }

    private sealed record PriceUpdate(
        string SelectionId,
        bool HasPrice,
        string Phase,
        string? Period);

    private sealed record TaggedPriceUpdate(
        string SelectionId,
        bool HasPrice,
        string Phase,
        string RiskTier);
}
