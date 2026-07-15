using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ResidualMissingComparatorTests
{
    [Fact]
    public void ResidualComparatorEmitsTargetOnlySegments()
    {
        var result = InvokeRuntime(Prepared(
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a"),
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b"),
            "residual"));

        var row = Assert.Single(result.ResidualRows!);
        Assert.Equal(1, row.Range.Start.Position);
        Assert.Equal(3, row.Range.End!.Value.Position);
        Assert.Single(row.TargetRecordIds);
        Assert.Equal(1, Assert.Single(result.ComparatorSummaries!).RowCount);
    }

    [Fact]
    public void MissingComparatorEmitsAgainstOnlySegments()
    {
        var result = InvokeRuntime(Prepared(
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a"),
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b"),
            "missing"));

        var row = Assert.Single(result.MissingRows!);
        Assert.Equal(5, row.Range.Start.Position);
        Assert.Equal(7, row.Range.End!.Value.Position);
        Assert.Single(row.AgainstRecordIds);
        Assert.Equal(1, Assert.Single(result.ComparatorSummaries!).RowCount);
    }

    private static PreparedComparison Prepared(ClosedWindow target, ClosedWindow against, string comparator)
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            [comparator]
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
