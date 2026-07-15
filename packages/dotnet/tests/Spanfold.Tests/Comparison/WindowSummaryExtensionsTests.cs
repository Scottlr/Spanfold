using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class WindowSummaryExtensionsTests
{
    [Fact]
    public void RecordedWindowsCanBeSummarizedBySegment()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a");
        pipeline.Ingest(new DeviceSignal("device-2", IsOnline: false, "Incident", "critical"), "lane-a");
        pipeline.Ingest(new DeviceSignal("device-3", IsOnline: false, "Normal", "standard"), "lane-b");
        pipeline.Ingest(new DeviceSignal("device-3", IsOnline: true, "Normal", "standard"), "lane-b");

        var summaries = pipeline.History.Query()
            .Window("DeviceOffline")
            .Windows()
            .SummarizeBySegment("lifecycle");

        var incident = Assert.Single(summaries, summary => Equals(summary.Value, "Incident"));
        Assert.Equal(WindowGroupKind.Segment, incident.GroupKind);
        Assert.Equal("lifecycle", incident.Name);
        Assert.Equal(2, incident.RecordCount);
        Assert.Equal(1, incident.FinalCount);
        Assert.Equal(1, incident.ProvisionalCount);
        Assert.Equal(1, incident.MeasuredPositionCount);
        Assert.Equal(1, incident.TotalPositionLength);

        var normal = Assert.Single(summaries, summary => Equals(summary.Value, "Normal"));
        Assert.Equal(1, normal.RecordCount);
        Assert.Equal(1, normal.FinalCount);
        Assert.Equal(0, normal.ProvisionalCount);
        Assert.Equal(1, normal.TotalPositionLength);
    }

    [Fact]
    public void RecordedWindowsCanBeSummarizedByTag()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a");
        pipeline.Ingest(new DeviceSignal("device-2", IsOnline: false, "Normal", "standard"), "lane-b");

        var summaries = pipeline.History.Query()
            .Window("DeviceOffline")
            .Windows()
            .SummarizeByTag("fleet");

        var critical = Assert.Single(summaries, summary => Equals(summary.Value, "critical"));
        Assert.Equal(WindowGroupKind.Tag, critical.GroupKind);
        Assert.Equal("fleet", critical.Name);
        Assert.Equal(1, critical.RecordCount);
        Assert.Equal(1, critical.FinalCount);
        Assert.Equal(0, critical.ProvisionalCount);
        Assert.Equal(1, critical.TotalPositionLength);

        var standard = Assert.Single(summaries, summary => Equals(summary.Value, "standard"));
        Assert.Equal(1, standard.RecordCount);
        Assert.Equal(0, standard.FinalCount);
        Assert.Equal(1, standard.ProvisionalCount);
        Assert.Equal(0, standard.TotalPositionLength);
    }

    [Fact]
    public void SnapshotRecordsCanBeSummarizedBySegmentWithHorizonLength()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a");
        pipeline.Ingest(new DeviceSignal("device-2", IsOnline: false, "Incident", "critical"), "lane-b");

        var summaries = pipeline.History
            .SnapshotAt(TemporalPoint.ForPosition(6))
            .Query()
            .Window("DeviceOffline")
            .Windows()
            .SummarizeBySegment("lifecycle");

        var incident = Assert.Single(summaries);
        Assert.Equal(2, incident.RecordCount);
        Assert.Equal(1, incident.FinalCount);
        Assert.Equal(1, incident.ProvisionalCount);
        Assert.Equal(2, incident.MeasuredPositionCount);
        Assert.Equal(4, incident.TotalPositionLength);
    }

    [Fact]
    public void SummaryRejectsMissingDimensionName()
    {
        var windows = Array.Empty<WindowRecord>();

        Assert.Throws<ArgumentException>(() => windows.SummarizeBySegment(""));
    }

    private static EventPipeline<DeviceSignal> CreatePipeline()
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
        string Lifecycle,
        string Fleet);
}
