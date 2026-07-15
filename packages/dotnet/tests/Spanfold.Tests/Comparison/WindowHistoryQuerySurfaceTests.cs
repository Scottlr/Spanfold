using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class WindowHistoryQuerySurfaceTests
{
    [Fact]
    public void CompareReturnsBuilderForHistory()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var builder = history.Compare("Provider QA");

        Assert.Equal("Provider QA", builder.Name);
    }

    [Fact]
    public void CompareRejectsEmptyName()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        Assert.Throws<ArgumentException>(() => history.Compare(""));
    }

    [Fact]
    public void CompareDoesNotMutateHistory()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false));

        _ = pipeline.History.Compare("Provider QA");

        Assert.Single(pipeline.History.OpenWindows);
        Assert.Empty(pipeline.History.ClosedWindows);
    }

    [Fact]
    public void ExistingDirectQueriesRemainAvailable()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        Assert.Empty(history.FindOverlaps());
        Assert.Empty(history.FindResiduals("provider-a"));
    }

    [Fact]
    public void QueryCanFilterClosedWindowsByWindowKeyLanePartitionSegmentAndTag()
    {
        var pipeline = CreateSegmentedPipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Normal", "standard"), "lane-b", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Normal", "standard"), "lane-b", "partition-1");

        var windows = pipeline.History.Query()
            .Window("DeviceOffline")
            .Key("device-1")
            .Lane("lane-a")
            .Partition("partition-1")
            .Segment("lifecycle", "Incident")
            .Tag("fleet", "critical")
            .ClosedWindows();

        var window = Assert.Single(windows);
        Assert.Equal("lane-a", window.Source);
        Assert.Equal("partition-1", window.Partition);
        Assert.Equal("Incident", Assert.Single(window.Segments).Value);
        Assert.Equal("critical", Assert.Single(window.Tags).Value);
    }

    [Fact]
    public void QueryCanReturnOpenWindowsAndLatestWindow()
    {
        var pipeline = CreateSegmentedPipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");

        var open = pipeline.History.Query()
            .Window("DeviceOffline")
            .Source("lane-a")
            .OpenWindows();
        var latest = pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .LatestWindow();

        var openWindow = Assert.Single(open);
        Assert.Equal(openWindow, latest);
        Assert.False(latest!.IsClosed);
        Assert.Equal(3, latest.StartPosition);
    }

    [Fact]
    public void QueryReturnsEmptyLatestWindowWhenNothingMatches()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var latest = history.Query()
            .Window("DeviceOffline")
            .Lane("missing")
            .LatestWindow();

        Assert.Null(latest);
    }

    private static EventPipeline<DeviceSignal> CreateSegmentedPipeline()
    {
        return EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", window => window
                .Key(signal => signal.DeviceId)
                .ActiveWhen(signal => !signal.IsOnline)
                .Segment("lifecycle", segment => segment.Value(signal => signal.Lifecycle))
                .Tag("fleet", signal => signal.Fleet));
    }

    private sealed record DeviceSignal(
        string DeviceId,
        bool IsOnline,
        string Lifecycle = "",
        string Fleet = "");
}
