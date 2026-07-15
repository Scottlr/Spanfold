using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class RuntimePlanCriticTests
{
    [Fact]
    public void NonSerializablePlansAreCriticizedAtRuntime()
    {
        var result = BuildHistory().Compare("Runtime QA")
            .Target("dynamic", selector => selector.Runtime("dynamic", "runtime predicate", static _ => true))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Overlap())
            .Run();

        Assert.True(result.IsValid);
        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.RuntimeNonSerializablePlan
            && diagnostic.Severity == ComparisonPlanDiagnosticSeverity.Warning);
    }

    [Fact]
    public void BroadScopesAreCriticizedWithStableCode()
    {
        var result = BuildHistory().Compare("Broad QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.All())
            .Using(comparators => comparators.Overlap())
            .Run();

        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.BroadSelector);
    }

    [Fact]
    public void AsOfWithoutKnownAtIsCriticizedAsFutureLeakageRisk()
    {
        var result = BuildHistory().Compare("As-of QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.AsOf(AsOfDirection.Next, TemporalAxis.ProcessingPosition, 10))
            .Run();

        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.FutureLeakageRisk);
    }

    [Fact]
    public void LiveFinalityWithoutHorizonIsCriticized()
    {
        var plan = new ComparisonPlan(
            "Live QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default with
            {
                OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon,
                OpenWindowHorizon = null
            },
            ["overlap"]
            );
        var result = InvokeRuntime(new PreparedComparison(plan, [], [], [], []));

        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.LiveFinalityWithoutHorizon);
    }

    [Fact]
    public void UnboundedOpenDurationsAreCriticized()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");

        var result = pipeline.History.Compare("Open QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Overlap())
            .Run();

        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.UnboundedOpenDuration);
    }

    [Fact]
    public void MixedTimestampClocksAreCriticized()
    {
        var plan = new ComparisonPlan(
            "Clock QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline", TemporalAxis.Timestamp),
            ComparisonNormalizationPolicy.Default with
            {
                TimeAxis = TemporalAxis.Timestamp,
                OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon,
                OpenWindowHorizon = TemporalPoint.ForTimestamp(DateTimeOffset.UnixEpoch, "provider"),
                KnownAt = TemporalPoint.ForTimestamp(DateTimeOffset.UnixEpoch, "received")
            },
            ["overlap"]
            );
        var result = InvokeRuntime(new PreparedComparison(plan, [], [], [], []));

        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.MixedClockRisk);
    }

    [Fact]
    public void StrictModeBlocksRuntimeCriticWarnings()
    {
        var result = BuildHistory().Compare("Strict broad QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.All())
            .Using(comparators => comparators.Overlap())
            .Strict()
            .Run();

        Assert.False(result.IsValid);
        Assert.Null(result.Aligned);
        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.BroadSelector
            && diagnostic.Severity == ComparisonPlanDiagnosticSeverity.Error);
    }

    [Fact]
    public void NonStrictCriticWarningsAreIncludedInExplainOutput()
    {
        var result = BuildHistory().Compare("Broad QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.All())
            .Using(comparators => comparators.Overlap())
            .Run();

        Assert.Contains("Warning BroadSelector", result.Explain());
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
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b");

        return pipeline.History;
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
