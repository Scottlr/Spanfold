using System.Text.Json;

using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class LiveFinalitySnapshotTests
{
    [Fact]
    public void CurrentOpenWindowEmitsProvisionalRow()
    {
        var pipeline = CreatePipeline();
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");

        var result = pipeline.History.Compare("Live QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(10));

        var row = Assert.Single(result.ResidualRows);
        Assert.Equal(TemporalRangeEndStatus.Closed, row.Range.EndStatus);
        var finality = Assert.Single(result.RowFinalities);
        Assert.StartsWith("residual:", finality.RowId);
        Assert.Equal(ComparisonFinality.Provisional, finality.Finality);
    }

    [Fact]
    public void ClosedWindowsEmitFinalRowsInLiveSnapshot()
    {
        var result = BuildClosedHistory().Compare("Live QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(10));

        Assert.NotEmpty(result.ResidualRows);
        Assert.All(result.RowFinalities, finality =>
            Assert.Equal(ComparisonFinality.Final, finality.Finality));
    }

    [Fact]
    public void LiveSnapshotConvergesWithBatchAfterWindowsClose()
    {
        var history = BuildClosedHistory();

        var batch = history.Compare("Provider QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Residual())
            .Run();

        var live = history.Compare("Provider QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(10));

        Assert.Equal(batch.ResidualRows.Count, live.ResidualRows.Count);
        Assert.Equal(batch.ResidualRows[0].Range, live.ResidualRows[0].Range);
        Assert.All(live.RowFinalities, finality =>
            Assert.Equal(ComparisonFinality.Final, finality.Finality));
    }

    [Fact]
    public void HorizonMetadataAppearsInResultAndExport()
    {
        var pipeline = CreatePipeline();
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");

        var result = pipeline.History.Compare("Live QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(10));

        Assert.Equal(TemporalPoint.ForPosition(10), result.EvaluationHorizon);

        using var document = JsonDocument.Parse(result.ExportJson());
        var horizon = document.RootElement.GetProperty("evaluationHorizon");
        Assert.Equal("ProcessingPosition", horizon.GetProperty("axis").GetString());
        Assert.Equal(10, horizon.GetProperty("position").GetInt64());
        Assert.Contains("evaluation horizon: pos:10", result.ExportMarkdown());
    }

    private static EventPipeline<DeviceSignal> CreatePipeline()
    {
        return EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);
    }

    private static WindowHistory BuildClosedHistory()
    {
        var pipeline = CreatePipeline();
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b");
        return pipeline.History;
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
