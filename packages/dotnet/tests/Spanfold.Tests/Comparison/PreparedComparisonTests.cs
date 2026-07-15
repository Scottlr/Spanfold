using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class PreparedComparisonTests
{
    [Fact]
    public void PrepareSelectsAndNormalizesMatchingWindows()
    {
        var history = BuildHistory();

        var prepared = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Using(c => c.Overlap())
            .Prepare();

        Assert.Empty(prepared.Diagnostics);
        Assert.Equal(2, prepared.SelectedWindows.Count);
        Assert.Equal(2, prepared.NormalizedWindows.Count);
        Assert.Contains(prepared.NormalizedWindows, window => window.Side == ComparisonSide.Target);
        Assert.Contains(prepared.NormalizedWindows, window => window.Side == ComparisonSide.Against);
    }

    [Fact]
    public void PrepareExcludesWindowsOutsideScope()
    {
        var history = BuildHistory();

        var prepared = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceDegraded"))
            .Using(c => c.Overlap())
            .Prepare();

        Assert.Empty(prepared.SelectedWindows);
        Assert.All(prepared.ExcludedWindows, excluded =>
            Assert.Equal("Window is outside the comparison scope.", excluded.Reason));
    }

    [Fact]
    public void OpenWindowsAreExcludedByDefault()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");

        var prepared = pipeline.History.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Using(c => c.Overlap())
            .Prepare();

        Assert.Empty(prepared.NormalizedWindows);
        Assert.Contains(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.OpenWindowsWithoutPolicy);
    }

    [Fact]
    public void OpenWindowsCanBeClippedToHorizon()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");

        var prepared = pipeline.History.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Normalize(n => n.ClipOpenWindowsTo(TemporalPoint.ForPosition(10)))
            .Using(c => c.Overlap())
            .Prepare();

        var normalized = Assert.Single(prepared.NormalizedWindows);
        Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, normalized.Range.EndStatus);
        Assert.Equal(9, normalized.Range.GetPositionLength());
    }

    [Fact]
    public void ClosedWindowsAreUnaffectedByOpenWindowHorizon()
    {
        var history = BuildHistory();

        var prepared = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Normalize(n => n.ClipOpenWindowsTo(TemporalPoint.ForPosition(100)))
            .Using(c => c.Overlap())
            .Prepare();

        Assert.Equal(2, prepared.NormalizedWindows.Count);
        Assert.All(prepared.NormalizedWindows, normalized =>
        {
            Assert.Equal(TemporalRangeEndStatus.Closed, normalized.Range.EndStatus);
            Assert.Equal(normalized.Window.EndPosition, normalized.Range.End!.Value.Position);
        });
    }

    [Fact]
    public void HorizonBeforeOpenWindowStartIsDiagnosed()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");

        var prepared = pipeline.History.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Normalize(n => n.ClipOpenWindowsTo(TemporalPoint.ForPosition(0)))
            .Using(c => c.Overlap())
            .Prepare();

        Assert.Empty(prepared.NormalizedWindows);
        Assert.Contains(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.InvalidRangeDuration);
    }

    [Fact]
    public void MissingEventTimeProducesDiagnosticInEventTimeMode()
    {
        var history = BuildHistory();

        var prepared = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Normalize(n => n.OnEventTime())
            .Using(c => c.Overlap())
            .Prepare();

        Assert.Empty(prepared.NormalizedWindows);
        Assert.Contains(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.MissingEventTime);
    }

    private static WindowHistory BuildHistory()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b");

        return pipeline.History;
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
