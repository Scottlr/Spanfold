using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class WindowOptionsCallbackTests
{
    [Fact]
    public void TrackWindowOptionsReceiveOpenedAndClosedCallbacks()
    {
        var opened = new List<WindowEmission<DeviceSignal>>();
        var closed = new List<WindowEmission<DeviceSignal>>();
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline,
                window => window
                    .OnOpened(opened.Add)
                    .OnClosed(closed.Add));

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false));
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true));

        Assert.Equal(WindowTransitionKind.Opened, Assert.Single(opened).Kind);
        Assert.Equal(WindowTransitionKind.Closed, Assert.Single(closed).Kind);
    }

    [Fact]
    public void WindowOptionsReceiveCallbacksForConfiguredWindowOnly()
    {
        var offlineOpened = new List<WindowEmission<DeviceSignal>>();
        var maintenanceOpened = new List<WindowEmission<DeviceSignal>>();
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .Window(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline,
                window => window.OnOpened(offlineOpened.Add))
            .Window(
                "ZoneMaintenance",
                signal => signal.ZoneId,
                signal => signal.ZoneInMaintenance,
                window => window.OnOpened(maintenanceOpened.Add))
            .Build();

        pipeline.Ingest(new DeviceSignal(
            DeviceId: "device-1",
            ZoneId: "zone-a",
            IsOnline: false,
            ZoneInMaintenance: true));

        Assert.Equal("DeviceOffline", Assert.Single(offlineOpened).WindowName);
        Assert.Equal("ZoneMaintenance", Assert.Single(maintenanceOpened).WindowName);
    }

    [Fact]
    public void WindowOptionCallbacksRunBeforeGlobalCallbacks()
    {
        var calls = new List<string>();
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .OnEmission(_ => calls.Add("global"))
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline,
                window => window.OnOpened(_ => calls.Add("window")));

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false));

        Assert.Equal(["window", "global"], calls);
    }

    [Fact]
    public void WindowOptionCallbackIsRequired()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline,
                window => Assert.Throws<ArgumentNullException>(() => window.OnOpened(null!)));

        Assert.Empty(pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true)).Emissions);
    }

    [Fact]
    public void BuiltPipelineUsesCallbackSnapshot()
    {
        WindowOptions<DeviceSignal, string>? captured = null;
        var opened = new List<WindowEmission<DeviceSignal>>();
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline,
                window => captured = window);

        captured!.OnOpened(opened.Add);
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false));

        Assert.Empty(opened);
    }

    private sealed record DeviceSignal(
        string DeviceId,
        bool IsOnline,
        string ZoneId = "zone-a",
        bool ZoneInMaintenance = false);
}
