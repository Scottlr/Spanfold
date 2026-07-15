using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class WindowHistorySnapshotTests
{
    [Fact]
    public void SnapshotClipsCurrentlyOpenWindowsToHorizon()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");

        var snapshot = pipeline.History.SnapshotAt(TemporalPoint.ForPosition(10));

        var record = Assert.Single(snapshot.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, record.Range.EndStatus);
        Assert.Equal(1, record.Range.Start.Position);
        Assert.Equal(10, record.Range.End!.Value.Position);
        Assert.Equal(ComparisonFinality.Provisional, record.Finality);
    }

    [Fact]
    public void SnapshotKeepsClosedWindowsFinalWhenTheyEndBeforeHorizon()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a", "partition-1");

        var snapshot = pipeline.History.SnapshotAt(TemporalPoint.ForPosition(10));

        var record = Assert.Single(snapshot.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .ClosedWindows());

        Assert.Equal(TemporalRangeEndStatus.Closed, record.Range.EndStatus);
        Assert.Equal(1, record.Range.Start.Position);
        Assert.Equal(2, record.Range.End!.Value.Position);
        Assert.Equal(ComparisonFinality.Final, record.Finality);
    }

    [Fact]
    public void SnapshotShowsClosedFutureEndWindowAsOpenAtEarlierHorizon()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a", "partition-1");

        var snapshot = pipeline.History.SnapshotAt(TemporalPoint.ForPosition(1));

        var record = Assert.Single(snapshot.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .OpenWindows());

        Assert.True(record.Window.IsClosed);
        Assert.Equal(1, record.Range.Start.Position);
        Assert.Equal(1, record.Range.End!.Value.Position);
        Assert.Equal(ComparisonFinality.Provisional, record.Finality);
    }

    [Fact]
    public void SnapshotQueryCanFilterByKeyPartitionSegmentAndTag()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-2", IsOnline: false, "Normal", "standard"), "lane-b", "partition-2");

        var record = Assert.Single(pipeline.History
            .SnapshotAt(TemporalPoint.ForPosition(3))
            .Query()
            .Window("DeviceOffline")
            .Key("device-1")
            .Lane("lane-a")
            .Partition("partition-1")
            .Segment("lifecycle", "Incident")
            .Tag("fleet", "critical")
            .OpenWindows());

        Assert.Equal("device-1", record.Window.Key);
        Assert.Equal("lane-a", record.Window.Source);
    }

    [Fact]
    public void HistoryQueryCanMaterializeOpenWindowsAtHorizon()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-2", IsOnline: false, "Normal", "standard"), "lane-b", "partition-2");

        var record = Assert.Single(pipeline.History.Query()
            .Window("DeviceOffline")
            .Key("device-1")
            .Lane("lane-a")
            .Partition("partition-1")
            .Segment("lifecycle", "Incident")
            .Tag("fleet", "critical")
            .OpenWindowsAt(TemporalPoint.ForPosition(3)));

        Assert.Equal("device-1", record.Window.Key);
        Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, record.Range.EndStatus);
        Assert.Equal(3, record.Range.End!.Value.Position);
    }

    [Fact]
    public void HistoryQueryCanMaterializeLatestWindowAtHorizon()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true, "Incident", "critical"), "lane-a", "partition-1");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false, "Incident", "critical"), "lane-a", "partition-1");

        var record = pipeline.History.Query()
            .Window("DeviceOffline")
            .Lane("lane-a")
            .LatestWindowAt(TemporalPoint.ForPosition(3));

        Assert.NotNull(record);
        Assert.Equal(3, record.Range.Start.Position);
        Assert.Equal(ComparisonFinality.Provisional, record.Finality);
    }

    [Fact]
    public void SnapshotRejectsUnknownHorizonAxis()
    {
        var pipeline = CreatePipeline();

        Assert.Throws<ArgumentException>(() => pipeline.History.SnapshotAt(default));
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
