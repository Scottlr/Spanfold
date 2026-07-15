using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class TrackWindowShortcutTests
{
    [Fact]
    public void TrackWindowBuildsSingleWindowPipeline()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline);

        var emission = Assert.Single(
            pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false)).Emissions);

        Assert.Equal("DeviceOffline", emission.WindowName);
        Assert.Equal("device-1", emission.Key);
        Assert.Equal(WindowTransitionKind.Opened, emission.Kind);
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
