using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class WindowAnnotationTests
{
    [Fact]
    public void AnnotationAttachedToOpenWindowRemainsAfterWindowCloses()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), "lane-a");

        var open = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        var annotation = pipeline.History.Annotate(
            open,
            "reason",
            "maintenance",
            TemporalPoint.ForPosition(2));

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), "lane-a");

        var closed = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .ClosedWindows());

        var annotations = pipeline.History.AnnotationsFor(closed);
        var attached = Assert.Single(annotations);

        Assert.Equal(annotation, attached);
        Assert.Equal("reason", attached.Name);
        Assert.Equal("maintenance", attached.Value);
        Assert.Equal(1, attached.Revision);
    }

    [Fact]
    public void RepeatedAnnotationNamesAppendRevisions()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), "lane-a");

        var open = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        var first = pipeline.History.Annotate(open, "classification", "initial");
        var second = pipeline.History.Annotate(open, "classification", "revised");

        Assert.Equal(1, first.Revision);
        Assert.Equal(2, second.Revision);
        Assert.Equal([first, second], pipeline.History.AnnotationsFor(open));
    }

    [Fact]
    public void AnnotationsKnownAtExcludesFutureAndUnknownAvailability()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), "lane-a");

        var open = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        var known = pipeline.History.Annotate(open, "reason", "maintenance", TemporalPoint.ForPosition(5));
        pipeline.History.Annotate(open, "owner", "team-a");
        pipeline.History.Annotate(open, "classification", "future", TemporalPoint.ForPosition(8));
        pipeline.History.Annotate(open, "timestamp-note", "different-axis", TemporalPoint.ForTimestamp(DateTimeOffset.Parse("2026-04-21T10:00:00Z")));

        var annotations = pipeline.History.AnnotationsKnownAt(open, TemporalPoint.ForPosition(6));

        var annotation = Assert.Single(annotations);
        Assert.Equal(known, annotation);
    }

    [Fact]
    public void AnnotationsKnownAtExcludesAnnotationsFromAnotherClock()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), "lane-a");

        var open = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        var differentClock = pipeline.History.Annotate(
            open,
            "reason",
            "other-clock",
            TemporalPoint.ForTimestamp(DateTimeOffset.Parse("2026-04-21T10:00:00Z"), "clock-b"));

        var annotations = pipeline.History.AnnotationsKnownAt(
            open,
            TemporalPoint.ForTimestamp(DateTimeOffset.Parse("2026-04-21T11:00:00Z"), "clock-a"));

        Assert.DoesNotContain(differentClock, annotations);
    }

    [Fact]
    public void AnnotationRejectsUnknownKnownAtAxis()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), "lane-a");

        var open = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        Assert.Throws<ArgumentException>(() => pipeline.History.Annotate(open, "reason", "unknown", default(TemporalPoint)));
    }

    [Fact]
    public void AnnotationsKnownAtRejectsUnknownHorizonAxis()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), "lane-a");

        var open = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        Assert.Throws<ArgumentException>(() => pipeline.History.AnnotationsKnownAt(open, default));
    }

    private static EventPipeline<DeviceSignal> CreatePipeline()
    {
        return EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", window => window
                .Key(signal => signal.DeviceId)
                .ActiveWhen(signal => !signal.IsOnline));
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
