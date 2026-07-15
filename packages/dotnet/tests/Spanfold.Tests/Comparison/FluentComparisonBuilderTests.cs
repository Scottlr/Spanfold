using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class FluentComparisonBuilderTests
{
    [Fact]
    public void ProviderQaExampleBuildsPlan()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var plan = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Normalize(n => n.RequireClosedWindows())
            .Using(c => c.Overlap().Residual().Coverage())
            .Build();

        Assert.Empty(plan.Validate());
        Assert.Equal("provider-a", plan.Target?.Name);
        Assert.Equal("provider-b", Assert.Single(plan.Against).Name);
        Assert.Equal(["overlap", "residual", "coverage"], plan.Comparators);
    }

    [Fact]
    public void MissingStagesProduceDiagnostics()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var diagnostics = history.Compare("Provider QA").Validate();

        Assert.Contains(diagnostics, d => d.Code == ComparisonPlanValidationCode.MissingTarget);
        Assert.Contains(diagnostics, d => d.Code == ComparisonPlanValidationCode.MissingAgainst);
        Assert.Contains(diagnostics, d => d.Code == ComparisonPlanValidationCode.MissingScope);
        Assert.Contains(diagnostics, d => d.Code == ComparisonPlanValidationCode.MissingComparator);
    }

    [Fact]
    public void MultipleAgainstSelectorsAreSupported()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var plan = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Against("provider-c", s => s.Source("provider-c"))
            .Within(s => s.Window("DeviceOffline"))
            .Using(c => c.Overlap())
            .Build();

        Assert.Equal(["provider-b", "provider-c"], plan.Against.Select(a => a.Name).ToArray());
    }

    [Fact]
    public void ComparatorDeclarationsCanBePassedThrough()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var plan = history.Compare("Extension QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Using(c => c.Overlap().Declaration("quality:drift"))
            .Build();

        Assert.Equal(["overlap", "quality:drift"], plan.Comparators);
    }

    [Fact]
    public void BlankComparatorDeclarationsAreRejected()
    {
        var builder = new ComparisonComparatorBuilder();

        Assert.Throws<ArgumentException>(() => builder.Declaration(" "));
    }

    [Fact]
    public void BuildDoesNotExecuteOrMutateHistory()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false));

        _ = pipeline.History.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Using(c => c.Overlap())
            .Build();

        Assert.Single(pipeline.History.OpenWindows);
        Assert.Empty(pipeline.History.ClosedWindows);
    }

    [Fact]
    public void PrepareAndRunReturnValidationArtifacts()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var builder = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Using(c => c.Overlap());

        var prepared = builder.Prepare();
        var result = builder.Run();

        Assert.Empty(prepared.Diagnostics);
        Assert.True(result.IsValid);
        Assert.Equal("Provider QA", result.Plan.Name);
    }

    [Fact]
    public void RunCanWriteDebugHtmlWhenConfigured()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);
        var directory = Path.Combine(Path.GetTempPath(), "spanfold-run-debug-" + Guid.NewGuid().ToString("N"));
        var path = Path.Combine(directory, "provider-qa.html");

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");

        try
        {
            var result = pipeline.History
                .Compare("Provider QA")
                .Target("provider-a", selector => selector.Source("provider-a"))
                .Against("provider-b", selector => selector.Source("provider-b"))
                .Within(scope => scope.Window("DeviceOffline"))
                .Using(comparators => comparators.Overlap().Residual())
                .Run();
            result.ExportDebugHtml(path);

            Assert.True(result.IsValid);
            Assert.True(File.Exists(path));
            Assert.Contains("Provider QA", File.ReadAllText(path));
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }
    }

    [Fact]
    public void RunCanWriteLlmContextWhenConfigured()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);
        var directory = Path.Combine(Path.GetTempPath(), "spanfold-run-llm-" + Guid.NewGuid().ToString("N"));
        var path = Path.Combine(directory, "provider-qa.llm.json");

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");

        try
        {
            var result = pipeline.History
                .Compare("Provider QA")
                .Target("provider-a", selector => selector.Source("provider-a"))
                .Against("provider-b", selector => selector.Source("provider-b"))
                .Within(scope => scope.Window("DeviceOffline"))
                .Using(comparators => comparators.Overlap().Residual())
                .Run();
            result.ExportLlmContext(path);

            Assert.True(result.IsValid);
            Assert.True(File.Exists(path));
            Assert.Contains("spanfold.comparison.llm-context", File.ReadAllText(path));
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
