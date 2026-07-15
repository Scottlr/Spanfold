using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class GapSymmetricDifferenceComparatorTests
{
    [Fact]
    public void GapComparatorDetectsInternalUncoveredSpaces()
    {
        var result = InvokeRuntime(Prepared(
            "gap",
            new NormalizedInput("DeviceOffline", "device-1", 1, 3, ComparisonSide.Target, "provider-a"),
            new NormalizedInput("DeviceOffline", "device-1", 5, 7, ComparisonSide.Against, "provider-b")));

        var row = Assert.Single(result.GapRows);
        Assert.Equal(3, row.Range.Start.Position);
        Assert.Equal(5, row.Range.End!.Value.Position);
        Assert.Equal(1, Assert.Single(result.ComparatorSummaries).RowCount);
    }

    [Fact]
    public void GapComparatorDoesNotInventBoundaryGaps()
    {
        var result = InvokeRuntime(Prepared(
            "gap",
            new NormalizedInput("DeviceOffline", "device-1", 1, 3, ComparisonSide.Target, "provider-a")));

        Assert.Empty(result.GapRows);
        Assert.Equal(0, Assert.Single(result.ComparatorSummaries).RowCount);
    }

    [Fact]
    public void SymmetricDifferenceIncludesBothDisagreementSides()
    {
        var result = InvokeRuntime(Prepared(
            "symmetric-difference",
            new NormalizedInput("DeviceOffline", "device-1", 1, 5, ComparisonSide.Target, "provider-a"),
            new NormalizedInput("DeviceOffline", "device-1", 3, 7, ComparisonSide.Against, "provider-b")));

        Assert.Collection(
            result.SymmetricDifferenceRows,
            first =>
            {
                Assert.Equal(ComparisonSide.Target, first.Side);
                Assert.Equal(1, first.Range.Start.Position);
                Assert.Equal(3, first.Range.End!.Value.Position);
                Assert.Single(first.TargetRecordIds);
                Assert.Empty(first.AgainstRecordIds);
            },
            second =>
            {
                Assert.Equal(ComparisonSide.Against, second.Side);
                Assert.Equal(5, second.Range.Start.Position);
                Assert.Equal(7, second.Range.End!.Value.Position);
                Assert.Empty(second.TargetRecordIds);
                Assert.Single(second.AgainstRecordIds);
            });
    }

    [Fact]
    public void GapAndSymmetricDifferenceRowsSortDeterministically()
    {
        var result = InvokeRuntime(Prepared(
            "symmetric-difference",
            new NormalizedInput("DeviceOffline", "device-2", 1, 3, ComparisonSide.Target, "provider-a"),
            new NormalizedInput("DeviceOffline", "device-1", 1, 3, ComparisonSide.Target, "provider-a")));

        Assert.Collection(
            result.SymmetricDifferenceRows,
            first => Assert.Equal("device-1", first.Key),
            second => Assert.Equal("device-2", second.Key));
    }

    [Fact]
    public void BuilderAddsGapAndSymmetricDifferenceComparators()
    {
        var pipeline = EventPipeline
            .For<NormalizedInput>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", input => input.Key, static _ => true);
        var plan = pipeline.History
            .Compare("Provider QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Gap().SymmetricDifference())
            .Build();

        Assert.Equal(["gap", "symmetric-difference"], plan.Comparators);
    }

    private static PreparedComparison Prepared(string comparator, params NormalizedInput[] inputs)
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            [comparator]
            );
        var selected = new List<WindowRecord>(inputs.Length);
        var normalized = new List<NormalizedWindowRecord>(inputs.Length);

        for (var i = 0; i < inputs.Length; i++)
        {
            var input = inputs[i];
            var source = input.Side == ComparisonSide.Target ? "provider-a" : "provider-b";
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
