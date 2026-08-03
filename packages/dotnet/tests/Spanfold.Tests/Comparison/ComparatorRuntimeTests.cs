using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparatorRuntimeTests
{
    [Fact]
    public void RunProducesSummariesForKnownComparators()
    {
        var history = BuildHistory();

        var result = history.Compare("Provider QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Using(c => c.Overlap().Residual().Coverage())
            .Run();

        Assert.True(result.IsValid);
        Assert.NotNull(result.Prepared);
        Assert.NotNull(result.Aligned);
        Assert.Equal(["overlap", "residual", "coverage"], result.ComparatorSummaries!.Select(s => s.ComparatorName).ToArray());
    }

    [Fact]
    public void RunDiagnosesUnknownComparators()
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["unknown"]
            );
        var prepared = new PreparedComparison(plan, [], [], [], []);

        var result = InvokeRuntime(prepared);

        Assert.False(result.IsValid);
        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.UnknownComparator);
    }

    [Theory]
    [InlineData("overlap")]
    [InlineData("lead-lag:End:Timestamp:9223372036854775807")]
    [InlineData("asof:Nearest:ProcessingPosition:+5")]
    [InlineData("lead-lag:Start:ProcessingPosition:1,000")]
    [InlineData("asof:Previous:ProcessingPosition: 10")]
    [InlineData("asof:previous:ProcessingPosition:10")]
    public void CatalogAndRuntimeAgreeOnComparatorDeclarations(string declaration)
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            [declaration]);
        var prepared = new PreparedComparison(plan, [], [], [], []);

        var result = InvokeRuntime(prepared);

        var runtimeRecognized = !result.Diagnostics.Any(diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.UnknownComparator);
        Assert.Equal(ComparisonComparatorCatalog.IsKnownDeclaration(declaration), runtimeRecognized);

        if (runtimeRecognized)
        {
            var summary = Assert.Single(result.ComparatorSummaries);
            Assert.Equal(declaration, summary.ComparatorName);
            Assert.Equal(0, summary.RowCount);
        }
    }

    private static ComparisonResult InvokeRuntime(PreparedComparison prepared)
    {
        var method = typeof(WindowComparisonBuilder)
            .Assembly
            .GetType("Spanfold.Internal.Comparison.ComparisonRuntime")!
            .GetMethod("Run", System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.NonPublic)!;

        return (ComparisonResult)method.Invoke(null, [prepared])!;
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
