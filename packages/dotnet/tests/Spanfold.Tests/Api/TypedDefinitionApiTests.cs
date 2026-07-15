using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class TypedDefinitionApiTests
{
    [Fact]
    public void WindowCanBeDefinedByType()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .Window<DeviceOffline>()
            .Build();

        var emission = Assert.Single(
            pipeline.Ingest(new DeviceSignal("device-1", "zone-a", IsOnline: false)).Emissions);

        Assert.Equal("DeviceOffline", emission.WindowName);
        Assert.Equal("device-1", emission.Key);
        Assert.Equal(WindowTransitionKind.Opened, emission.Kind);
    }

    [Fact]
    public void TrackWindowCanBeDefinedByType()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .TrackWindow<DeviceOffline>();

        var emission = Assert.Single(
            pipeline.Ingest(new DeviceSignal("device-1", "zone-a", IsOnline: false)).Emissions);

        Assert.Equal("DeviceOffline", emission.WindowName);
    }

    [Fact]
    public void RollUpCanBeDefinedByType()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .Window<DeviceOffline>()
            .RollUp<ZoneOutage>()
            .Build();

        var result = pipeline.Ingest(new DeviceSignal("device-1", "zone-a", IsOnline: false));

        Assert.Collection(
            result.Emissions,
            device => Assert.Equal("DeviceOffline", device.WindowName),
            zone =>
            {
                Assert.Equal("ZoneOutage", zone.WindowName);
                Assert.Equal("zone-a", zone.Key);
            });
    }

    [Fact]
    public void TypedDefinitionCanOverrideNameAndRegisterCallbacks()
    {
        CallbackDeviceOffline.Opened.Clear();
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .TrackWindow<CallbackDeviceOffline>();

        pipeline.Ingest(new DeviceSignal("device-1", "zone-a", IsOnline: false));

        var emission = Assert.Single(CallbackDeviceOffline.Opened);
        Assert.Equal("OfflineDevice", emission.WindowName);
    }

    [Fact]
    public void TypedWindowDefinitionMustConfigureKey()
    {
        var exception = Assert.Throws<InvalidOperationException>(() => EventPipeline
            .For<DeviceSignal>()
            .Window<MissingKeyWindow>());

        Assert.Contains("must configure a key", exception.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void TypedRollUpDefinitionMustConfigureActiveState()
    {
        var exception = Assert.Throws<InvalidOperationException>(() => EventPipeline
            .For<DeviceSignal>()
            .Window<DeviceOffline>()
            .RollUp<MissingActiveRollUp>());

        Assert.Contains("must configure active state", exception.Message, StringComparison.Ordinal);
    }

    private sealed record DeviceSignal(string DeviceId, string ZoneId, bool IsOnline);

    private sealed class DeviceOffline : IWindowDefinition<DeviceSignal>
    {
        public void Define(WindowDefinitionBuilder<DeviceSignal> window)
        {
            window
                .Key(signal => signal.DeviceId)
                .ActiveWhen(signal => !signal.IsOnline);
        }
    }

    private sealed class ZoneOutage : IRollUpDefinition<DeviceSignal>
    {
        public void Define(RollUpDefinitionBuilder<DeviceSignal> rollUp)
        {
            rollUp
                .Key(signal => signal.ZoneId)
                .ActiveWhen(children => children.AllActive());
        }
    }

    private sealed class CallbackDeviceOffline : IWindowDefinition<DeviceSignal>
    {
        public static List<WindowEmission<DeviceSignal>> Opened { get; } = [];

        public void Define(WindowDefinitionBuilder<DeviceSignal> window)
        {
            window
                .Named("OfflineDevice")
                .Key(signal => signal.DeviceId)
                .ActiveWhen(signal => !signal.IsOnline)
                .OnOpened(Opened.Add);
        }
    }

    private sealed class MissingKeyWindow : IWindowDefinition<DeviceSignal>
    {
        public void Define(WindowDefinitionBuilder<DeviceSignal> window)
        {
            window.ActiveWhen(signal => !signal.IsOnline);
        }
    }

    private sealed class MissingActiveRollUp : IRollUpDefinition<DeviceSignal>
    {
        public void Define(RollUpDefinitionBuilder<DeviceSignal> rollUp)
        {
            rollUp.Key(signal => signal.ZoneId);
        }
    }
}
