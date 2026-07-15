using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class ReadmeExampleTests
{
    [Fact]
    public void ReadmeDeviceToZoneExampleCompilesAndRuns()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .Window(
                "DeviceOffline",
                key: signal => signal.DeviceId,
                isActive: signal => !signal.IsOnline)
            .RollUp(
                "ZoneOutage",
                key: signal => signal.ZoneId,
                isActive: children => children.AllActive())
            .Build();

        var opened = pipeline.Ingest(new DeviceSignal(
            DeviceId: "device-1",
            ZoneId: "zone-a",
            IsOnline: false));

        Assert.NotEmpty(opened.Emissions);
    }

    [Fact]
    public void ReadmeComparisonQuickstartCompilesAndRuns()
    {
        var pipeline = EventPipeline
            .For<ComparisonDeviceSignal>()
            .RecordWindows()
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline);

        pipeline.Ingest(new ComparisonDeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new ComparisonDeviceSignal("device-1", IsOnline: true), source: "provider-a");

        var result = pipeline.History
            .Compare("Source coverage")
            .Target("all-offline", selector => selector.WindowName("DeviceOffline"))
            .Against("provider-a", selector => selector.Source("provider-a"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators
                .Overlap()
                .Coverage())
            .Run();

        Assert.Single(result.OverlapRows);
        Assert.Empty(result.ResidualRows);
        Assert.Single(result.CoverageRows);
        Assert.Contains("overlap rows: 1", result.ExportMarkdown());
    }

    private sealed record DeviceSignal(
        string DeviceId,
        string ZoneId,
        bool IsOnline);

    private sealed record ComparisonDeviceSignal(string DeviceId, bool IsOnline);
}
