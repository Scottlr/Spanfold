using System.Text.Json;

using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class KnownAtComparisonTests
{
    [Fact]
    public void ClosedWindowIsMaterializedProvisionalBeforeKnownAtPosition()
    {
        var history = BuildHistory();

        var prepared = history.Compare("Known-at QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Normalize(normalization => normalization.KnownAtPosition(2))
            .Using(comparators => comparators.Overlap())
            .Prepare();

        Assert.Equal(2, prepared.NormalizedWindows.Count);
        Assert.All(prepared.NormalizedWindows, static window =>
            Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, window.Range.EndStatus));
        Assert.DoesNotContain(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.FutureWindowExcluded);
    }

    [Fact]
    public void ClosedWindowIsIncludedAfterKnownAtPosition()
    {
        var history = BuildHistory();

        var prepared = history.Compare("Known-at QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Normalize(normalization => normalization.KnownAtPosition(4))
            .Using(comparators => comparators.Overlap())
            .Prepare();

        Assert.NotEmpty(prepared.NormalizedWindows);
        Assert.DoesNotContain(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.FutureWindowExcluded);
    }

    [Fact]
    public void TimestampKnownAtWithoutAvailabilityInformationIsDiagnosed()
    {
        var history = BuildHistory();

        var prepared = history.Compare("Known-at QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Normalize(normalization => normalization.KnownAt(TemporalPoint.ForTimestamp(DateTimeOffset.UnixEpoch)))
            .Using(comparators => comparators.Overlap())
            .Prepare();

        Assert.Contains(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.KnownAtRequiresProcessingPosition);
        Assert.DoesNotContain(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.FutureWindowExcluded);
    }

    [Fact]
    public void ExportCarriesKnownAtMetadata()
    {
        var result = BuildHistory().Compare("Known-at QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Normalize(normalization => normalization.KnownAtPosition(4))
            .Using(comparators => comparators.Overlap())
            .Run();

        using var document = JsonDocument.Parse(result.ExportJson());
        var knownAt = document.RootElement
            .GetProperty("plan")
            .GetProperty("normalization")
            .GetProperty("knownAt");

        Assert.Equal("ProcessingPosition", knownAt.GetProperty("axis").GetString());
        Assert.Equal(4, knownAt.GetProperty("position").GetInt64());
        Assert.Equal(TemporalPoint.ForPosition(4), result.KnownAt);
        Assert.Contains("knownAt=pos:4", result.ExportMarkdown());
    }

    private static WindowHistory BuildHistory()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b");

        return pipeline.History;
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
