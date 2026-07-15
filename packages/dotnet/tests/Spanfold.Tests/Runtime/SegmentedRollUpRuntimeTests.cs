using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class SegmentedRollUpRuntimeTests
{
    [Fact]
    public void RollUpsPreserveChildSegmentContext()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)))
            .RollUp("MarketPriced", update => update.MarketId, children => children.ActiveCount > 0)
            .Build();

        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "Pregame"));
        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay"));

        var closedMarket = Assert.Single(
            pipeline.History.ClosedWindows,
            window => window.WindowName == "MarketPriced");
        Assert.Equal("Pregame", Assert.Single(closedMarket.Segments).Value);

        var openMarket = Assert.Single(
            pipeline.History.OpenWindows,
            window => window.WindowName == "MarketPriced");
        Assert.Equal("InPlay", Assert.Single(openMarket.Segments).Value);
    }

    [Fact]
    public void MultiLevelRollUpsPreserveChildSegmentContext()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)))
            .RollUp("MarketPriced", update => update.MarketId, children => children.ActiveCount > 0)
            .RollUp("FixturePriced", update => update.FixtureId, children => children.ActiveCount > 0)
            .Build();

        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "Pregame"));
        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay"));

        var closedFixture = Assert.Single(
            pipeline.History.ClosedWindows,
            window => window.WindowName == "FixturePriced");
        Assert.Equal("Pregame", Assert.Single(closedFixture.Segments).Value);

        var openFixture = Assert.Single(
            pipeline.History.OpenWindows,
            window => window.WindowName == "FixturePriced");
        Assert.Equal("InPlay", Assert.Single(openFixture.Segments).Value);
    }

    [Fact]
    public void RollUpCanDropChildSegmentContext()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase
                    .Value(update => update.Phase)
                    .Child("state", state => state.Value(update => update.State))))
            .RollUp(
                "MarketPriced",
                update => update.MarketId,
                children => children.ActiveCount > 0,
                segments => segments.Drop("state"))
            .Build();

        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay", "Suspended"));
        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay", "Open"));

        Assert.DoesNotContain(
            pipeline.History.ClosedWindows,
            window => window.WindowName == "MarketPriced");

        var openMarket = Assert.Single(
            pipeline.History.OpenWindows,
            window => window.WindowName == "MarketPriced");
        var segment = Assert.Single(openMarket.Segments);
        Assert.Equal("phase", segment.Name);
        Assert.Equal("InPlay", segment.Value);
    }

    [Fact]
    public void DroppedSegmentChangeStillMigratesActiveChildToNewParent()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase
                    .Value(update => update.Phase)
                    .Child("state", state => state.Value(update => update.State))))
            .RollUp(
                "MarketPriced",
                update => update.MarketId,
                children => children.AnyActive(),
                segments => segments.Drop("state"))
            .Build();

        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", true, "InPlay", State: "Suspended"));
        pipeline.Ingest(new PriceUpdate("selection-1", "market-2", "fixture-1", true, "InPlay", State: "Open"));

        var closed = Assert.Single(pipeline.History.ClosedWindows, window => window.WindowName == "MarketPriced");
        Assert.Equal("market-1", closed.Key);
        var open = Assert.Single(pipeline.History.OpenWindows, window => window.WindowName == "MarketPriced");
        Assert.Equal("market-2", open.Key);
    }

    [Fact]
    public void RollUpCanPreserveSelectedChildSegments()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase
                    .Value(update => update.Phase)
                    .Child("period", period => period
                        .Value(update => update.Period)
                        .Child("state", state => state.Value(update => update.State)))))
            .RollUp(
                "MarketPriced",
                update => update.MarketId,
                children => children.ActiveCount > 0,
                segments => segments
                    .Preserve("phase")
                    .Preserve("period"))
            .Build();

        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay", "Q4", "Suspended"));
        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay", "Q4", "Open"));

        Assert.DoesNotContain(
            pipeline.History.ClosedWindows,
            window => window.WindowName == "MarketPriced");

        var openMarket = Assert.Single(
            pipeline.History.OpenWindows,
            window => window.WindowName == "MarketPriced");
        Assert.Collection(
            openMarket.Segments,
            segment =>
            {
                Assert.Equal("phase", segment.Name);
                Assert.Equal("InPlay", segment.Value);
                Assert.Null(segment.ParentName);
            },
            segment =>
            {
                Assert.Equal("period", segment.Name);
                Assert.Equal("Q4", segment.Value);
                Assert.Equal("phase", segment.ParentName);
            });
    }

    [Fact]
    public void RollUpCanRenameChildSegment()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)))
            .RollUp(
                "MarketPriced",
                update => update.MarketId,
                children => children.ActiveCount > 0,
                segments => segments.Rename("phase", "lifecycle"))
            .Build();

        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay"));

        var openMarket = Assert.Single(
            pipeline.History.OpenWindows,
            window => window.WindowName == "MarketPriced");
        var segment = Assert.Single(openMarket.Segments);
        Assert.Equal("lifecycle", segment.Name);
        Assert.Equal("InPlay", segment.Value);
    }

    [Fact]
    public void RollUpCanTransformChildSegmentValue()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)))
            .RollUp(
                "MarketPriced",
                update => update.MarketId,
                children => children.ActiveCount > 0,
                segments => segments.Transform("phase", value => value?.ToString()?.ToUpperInvariant()))
            .Build();

        pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "in-play"));

        var openMarket = Assert.Single(
            pipeline.History.OpenWindows,
            window => window.WindowName == "MarketPriced");
        Assert.Equal("IN-PLAY", Assert.Single(openMarket.Segments).Value);
    }

    [Fact]
    public void RollUpRejectsDuplicateProjectedSegmentNames()
    {
        var pipeline = EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .Window("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase
                    .Value(update => update.Phase)
                    .Child("period", period => period.Value(update => update.Period))))
            .RollUp(
                "MarketPriced",
                update => update.MarketId,
                children => children.ActiveCount > 0,
                segments => segments.Rename("period", "phase"))
            .Build();

        var exception = Assert.Throws<InvalidOperationException>(() =>
            pipeline.Ingest(new PriceUpdate("selection-1", "market-1", "fixture-1", HasPrice: true, "InPlay", "Q4")));
        Assert.Contains("duplicate segment 'phase'", exception.Message);
    }

    private sealed record PriceUpdate(
        string SelectionId,
        string MarketId,
        string FixtureId,
        bool HasPrice,
        string Phase,
        string Period = "",
        string State = "");
}
