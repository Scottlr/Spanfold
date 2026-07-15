using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class CoverageComparatorTests
{
    [Fact]
    public void CoverageSummarizesCoveredTargetMagnitude()
    {
        var result = InvokeRuntime(Prepared(
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a"),
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b")));

        Assert.Equal(2, result.CoverageRows!.Count);
        var summary = Assert.Single(result.CoverageSummaries!);

        Assert.Equal(4, summary.TargetMagnitude);
        Assert.Equal(2, summary.CoveredMagnitude);
        Assert.Equal(0.5d, summary.CoverageRatio);
        Assert.Equal(2, Assert.Single(result.ComparatorSummaries!).RowCount);
    }

    [Fact]
    public void CoverageWithNoComparisonCoverageHasZeroRatio()
    {
        var result = InvokeRuntime(Prepared(
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a"),
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 5, EndPosition: 7, Source: "provider-b")));

        var summary = Assert.Single(result.CoverageSummaries!);

        Assert.Equal(4, summary.TargetMagnitude);
        Assert.Equal(0, summary.CoveredMagnitude);
        Assert.Equal(0d, summary.CoverageRatio);
    }

    private static PreparedComparison Prepared(ClosedWindow target, ClosedWindow against)
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["coverage"]
            );

        return new PreparedComparison(
            plan,
            [],
            [target, against],
            [],
            [
                new NormalizedWindowRecord(
                    target,
                    target.Id,
                    "provider-a",
                    ComparisonSide.Target,
                    TemporalRange.Closed(TemporalPoint.ForPosition(target.StartPosition), TemporalPoint.ForPosition(target.EndPosition!.Value))),
                new NormalizedWindowRecord(
                    against,
                    against.Id,
                    "provider-b",
                    ComparisonSide.Against,
                    TemporalRange.Closed(TemporalPoint.ForPosition(against.StartPosition), TemporalPoint.ForPosition(against.EndPosition!.Value)))
            ]);
    }

    private static ComparisonResult InvokeRuntime(PreparedComparison prepared)
    {
        var method = typeof(WindowComparisonBuilder)
            .Assembly
            .GetType("Spanfold.Internal.Comparison.ComparisonRuntime")!
            .GetMethod("Run", System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.NonPublic)!;

        return (ComparisonResult)method.Invoke(null, [prepared])!;
    }
}
