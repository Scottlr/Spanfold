using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class OverlapComparatorTests
{
    [Fact]
    public void RunEmitsOverlapRowsForSharedAlignedSegments()
    {
        var target = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a");
        var against = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b");
        var result = InvokeRuntime(Prepared(target, against));

        var row = Assert.Single(result.OverlapRows!);
        Assert.Equal("DeviceOffline", row.WindowName);
        Assert.Equal("device-1", row.Key);
        Assert.Equal(3, row.Range.Start.Position);
        Assert.Equal(5, row.Range.End!.Value.Position);
        Assert.Single(row.TargetRecordIds);
        Assert.Single(row.AgainstRecordIds);
        Assert.Equal(1, Assert.Single(result.ComparatorSummaries!).RowCount);
    }

    [Fact]
    public void TouchingWindowsDoNotEmitOverlapRows()
    {
        var target = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 3, Source: "provider-a");
        var against = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 5, Source: "provider-b");
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
            );
        var prepared = new PreparedComparison(
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
                    TemporalRange.Closed(TemporalPoint.ForPosition(1), TemporalPoint.ForPosition(3))),
                new NormalizedWindowRecord(
                    against,
                    against.Id,
                    "provider-b",
                    ComparisonSide.Against,
                    TemporalRange.Closed(TemporalPoint.ForPosition(3), TemporalPoint.ForPosition(5)))
            ]);

        var result = InvokeRuntime(prepared);

        Assert.Empty(result.OverlapRows!);
        Assert.Equal(0, Assert.Single(result.ComparatorSummaries!).RowCount);
    }

    private static PreparedComparison Prepared(ClosedWindow target, ClosedWindow against)
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
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
