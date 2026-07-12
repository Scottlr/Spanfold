using Spanfold;

namespace Spanfold.Tests.Recording;

public sealed class WindowSegmentRecordTests
{
    [Fact]
    public void WindowRecordsExposeEmptySegmentsAndTagsByDefault()
    {
        var window = new ClosedWindow("SelectionPriced", "selection-1", 1, 5);

        Assert.Empty(window.Segments);
        Assert.Empty(window.Tags);
    }

    [Fact]
    public void WindowRecordsPreserveSegmentsAndTags()
    {
        var window = new ClosedWindow(
            "SelectionPriced",
            "selection-1",
            1,
            5,
            Segments:
            [
                new WindowSegment("phase", "InPlay"),
                new WindowSegment("period", "FinalQuarter", ParentName: "phase")
            ],
            Tags:
            [
                new WindowTag("competition", "cup")
            ]);

        Assert.Equal("phase", window.Segments[0].Name);
        Assert.Equal("InPlay", window.Segments[0].Value);
        Assert.Equal("phase", window.Segments[1].ParentName);
        Assert.Equal("competition", Assert.Single(window.Tags).Name);
    }

    [Fact]
    public void WindowRecordsCloneCallerOwnedCollections()
    {
        var segments = new[] { new WindowSegment("phase", "InPlay") };
        var window = new ClosedWindow("SelectionPriced", "selection-1", 1, 5, Segments: segments);

        segments[0] = new WindowSegment("phase", "Final");

        Assert.Equal("InPlay", Assert.Single(window.Segments).Value);
    }

    [Fact]
    public void WindowRecordIdIncludesSegmentContext()
    {
        var pregame = new ClosedWindow(
            "SelectionPriced",
            "selection-1",
            1,
            5,
            Segments: [new WindowSegment("phase", "Pregame")]);
        var inPlay = new ClosedWindow(
            "SelectionPriced",
            "selection-1",
            1,
            5,
            Segments: [new WindowSegment("phase", "InPlay")]);

        Assert.NotEqual(pregame.Id, inPlay.Id);
    }

    [Fact]
    public void RecordedWindowsCarryEmissionSegments()
    {
        var pipeline = Spanfold
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", update => update.SelectionId, update => update.HasPrice);

        var opened = new WindowEmission<PriceUpdate>(
            "SelectionPriced",
            "selection-1",
            new PriceUpdate("selection-1", HasPrice: true),
            WindowTransitionKind.Opened,
            Segments: [new WindowSegment("phase", "InPlay")]);
        var closed = opened with
        {
            Event = new PriceUpdate("selection-1", HasPrice: false),
            Kind = WindowTransitionKind.Closed
        };

        pipeline.History.Record([opened], processingPosition: 1, eventTime: null);
        pipeline.History.Record([closed], processingPosition: 2, eventTime: null);

        var window = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Equal("phase", Assert.Single(window.Segments).Name);
    }

    private sealed record PriceUpdate(string SelectionId, bool HasPrice);
}
