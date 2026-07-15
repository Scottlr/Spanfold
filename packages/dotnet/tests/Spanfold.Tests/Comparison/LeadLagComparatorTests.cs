using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class LeadLagComparatorTests
{
    [Fact]
    public void LeadLagComparatorReportsTargetLeadsAgainst()
    {
        var result = InvokeRuntime(Prepared(
            "lead-lag:Start:ProcessingPosition:5",
            new NormalizedInput("DeviceOffline", "device-1", 1, 4, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 3, 6, ComparisonSide.Against, "comparison")));

        var row = Assert.Single(result.LeadLagRows);
        Assert.Equal(LeadLagDirection.TargetLeads, row.Direction);
        Assert.Equal(-2, row.DeltaMagnitude);
        Assert.True(row.IsWithinTolerance);
        Assert.Equal(1, Assert.Single(result.LeadLagSummaries).TargetLeadCount);
    }

    [Fact]
    public void LeadLagComparatorReportsTargetLagsAgainst()
    {
        var result = InvokeRuntime(Prepared(
            "lead-lag:Start:ProcessingPosition:5",
            new NormalizedInput("DeviceOffline", "device-1", 5, 8, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 2, 7, ComparisonSide.Against, "comparison")));

        var row = Assert.Single(result.LeadLagRows);
        Assert.Equal(LeadLagDirection.TargetLags, row.Direction);
        Assert.Equal(3, row.DeltaMagnitude);
        Assert.True(row.IsWithinTolerance);
    }

    [Fact]
    public void LeadLagComparatorReportsEqualTransitions()
    {
        var result = InvokeRuntime(Prepared(
            "lead-lag:Start:ProcessingPosition:0",
            new NormalizedInput("DeviceOffline", "device-1", 5, 8, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 5, 9, ComparisonSide.Against, "comparison")));

        var row = Assert.Single(result.LeadLagRows);
        Assert.Equal(LeadLagDirection.Equal, row.Direction);
        Assert.Equal(0, row.DeltaMagnitude);
        Assert.True(row.IsWithinTolerance);
        Assert.Equal(1, Assert.Single(result.LeadLagSummaries).EqualCount);
    }

    [Fact]
    public void LeadLagComparatorMarksRowsOutsideTolerance()
    {
        var result = InvokeRuntime(Prepared(
            "lead-lag:Start:ProcessingPosition:1",
            new NormalizedInput("DeviceOffline", "device-1", 5, 8, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 1, 7, ComparisonSide.Against, "comparison")));

        var row = Assert.Single(result.LeadLagRows);
        Assert.Equal(4, row.DeltaMagnitude);
        Assert.False(row.IsWithinTolerance);
        Assert.Equal(1, Assert.Single(result.LeadLagSummaries).OutsideToleranceCount);
    }

    [Fact]
    public void LeadLagComparatorReportsMissingComparisonTransition()
    {
        var result = InvokeRuntime(Prepared(
            "lead-lag:Start:ProcessingPosition:5",
            new NormalizedInput("DeviceOffline", "device-1", 5, 8, ComparisonSide.Target, "target")));

        var row = Assert.Single(result.LeadLagRows);
        Assert.Equal(LeadLagDirection.MissingComparison, row.Direction);
        Assert.Null(row.ComparisonPoint);
        Assert.Null(row.DeltaMagnitude);
        Assert.False(row.IsWithinTolerance);
        Assert.Equal(1, Assert.Single(result.LeadLagSummaries).MissingComparisonCount);
    }

    [Fact]
    public void LeadLagComparatorCanCompareEndTransitions()
    {
        var result = InvokeRuntime(Prepared(
            "lead-lag:End:ProcessingPosition:5",
            new NormalizedInput("DeviceOffline", "device-1", 2, 9, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 1, 7, ComparisonSide.Against, "comparison")));

        var row = Assert.Single(result.LeadLagRows);
        Assert.Equal(LeadLagTransition.End, row.Transition);
        Assert.Equal(2, row.DeltaMagnitude);
        Assert.Equal(LeadLagDirection.TargetLags, row.Direction);
    }

    [Fact]
    public void BuilderRequiresExplicitLeadLagOptions()
    {
        var pipeline = EventPipeline
            .For<NormalizedInput>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", input => input.Key, static _ => true);

        var plan = pipeline.History
            .Compare("Latency QA")
            .Target("target", selector => selector.Source("target"))
            .Against("comparison", selector => selector.Source("comparison"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.LeadLag(
                LeadLagTransition.Start,
                TemporalAxis.ProcessingPosition,
                toleranceMagnitude: 5))
            .Build();

        Assert.Equal(["lead-lag:Start:ProcessingPosition:5"], plan.Comparators);
    }

    private static PreparedComparison Prepared(string comparator, params NormalizedInput[] inputs)
    {
        var plan = new ComparisonPlan(
            "Latency QA",
            ComparisonSelector.ForSource("target"),
            [ComparisonSelector.ForSource("comparison")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            [comparator]
            );
        var selected = new List<WindowRecord>(inputs.Length);
        var normalized = new List<NormalizedWindowRecord>(inputs.Length);

        for (var i = 0; i < inputs.Length; i++)
        {
            var input = inputs[i];
            var source = input.Side == ComparisonSide.Target ? "target" : "comparison";
            var window = new ClosedWindow(
                input.WindowName,
                input.Key,
                input.StartPosition,
                input.EndPosition,
                Source: source);

            selected.Add(window);
            normalized.Add(new NormalizedWindowRecord(
                window,
                window.Id,
                input.SelectorName,
                input.Side,
                TemporalRange.Closed(
                    TemporalPoint.ForPosition(input.StartPosition),
                    TemporalPoint.ForPosition(input.EndPosition))));
        }

        return new PreparedComparison(plan, [], selected.ToArray(), [], normalized.ToArray());
    }

    private static ComparisonResult InvokeRuntime(PreparedComparison prepared)
    {
        var method = typeof(WindowComparisonBuilder)
            .Assembly
            .GetType("Spanfold.Internal.Comparison.ComparisonRuntime")!
            .GetMethod("Run", System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.NonPublic)!;

        return (ComparisonResult)method.Invoke(null, [prepared])!;
    }

    private sealed record NormalizedInput(
        string WindowName,
        string Key,
        long StartPosition,
        long EndPosition,
        ComparisonSide Side,
        string SelectorName);
}
