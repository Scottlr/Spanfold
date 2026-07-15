using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ContainmentComparatorTests
{
    [Fact]
    public void ContainmentComparatorEmitsContainedRows()
    {
        var result = InvokeRuntime(Prepared(
            new NormalizedInput("DeviceOffline", "device-1", 2, 4, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 1, 5, ComparisonSide.Against, "container")));

        var row = Assert.Single(result.ContainmentRows);
        Assert.Equal(ContainmentStatus.Contained, row.Status);
        Assert.Equal(2, row.Range.Start.Position);
        Assert.Equal(4, row.Range.End!.Value.Position);
        Assert.Single(row.TargetRecordIds);
        Assert.Single(row.ContainerRecordIds);
    }

    [Fact]
    public void ContainmentComparatorClassifiesLeftAndRightOverhangs()
    {
        var result = InvokeRuntime(Prepared(
            new NormalizedInput("DeviceOffline", "device-1", 1, 7, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 3, 5, ComparisonSide.Against, "container")));

        Assert.Collection(
            result.ContainmentRows,
            left =>
            {
                Assert.Equal(ContainmentStatus.LeftOverhang, left.Status);
                Assert.Equal(1, left.Range.Start.Position);
                Assert.Equal(3, left.Range.End!.Value.Position);
            },
            contained =>
            {
                Assert.Equal(ContainmentStatus.Contained, contained.Status);
                Assert.Equal(3, contained.Range.Start.Position);
                Assert.Equal(5, contained.Range.End!.Value.Position);
            },
            right =>
            {
                Assert.Equal(ContainmentStatus.RightOverhang, right.Status);
                Assert.Equal(5, right.Range.Start.Position);
                Assert.Equal(7, right.Range.End!.Value.Position);
            });
    }

    [Fact]
    public void TargetLargerThanContainerProducesNotFullyContainedRows()
    {
        var result = InvokeRuntime(Prepared(
            new NormalizedInput("DeviceOffline", "device-1", 1, 5, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 2, 4, ComparisonSide.Against, "container")));

        Assert.Contains(result.ContainmentRows, row => row.Status == ContainmentStatus.LeftOverhang);
        Assert.Contains(result.ContainmentRows, row => row.Status == ContainmentStatus.RightOverhang);
        Assert.Contains(result.ContainmentRows, row => row.Status == ContainmentStatus.Contained);
    }

    [Fact]
    public void MultipleContainersCanFullyContainOneTarget()
    {
        var result = InvokeRuntime(Prepared(
            new NormalizedInput("DeviceOffline", "device-1", 1, 7, ComparisonSide.Target, "target"),
            new NormalizedInput("DeviceOffline", "device-1", 1, 3, ComparisonSide.Against, "container-a"),
            new NormalizedInput("DeviceOffline", "device-1", 3, 7, ComparisonSide.Against, "container-b")));

        Assert.Equal(2, result.ContainmentRows.Count);
        Assert.All(result.ContainmentRows, row => Assert.Equal(ContainmentStatus.Contained, row.Status));
        Assert.Equal(2, Assert.Single(result.ComparatorSummaries).RowCount);
    }

    [Fact]
    public void BuilderAddsContainmentComparator()
    {
        var pipeline = EventPipeline
            .For<NormalizedInput>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", input => input.Key, static _ => true);

        var plan = pipeline.History
            .Compare("Containment QA")
            .Target("target", selector => selector.Source("target"))
            .Against("container", selector => selector.Source("container"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Containment())
            .Build();

        Assert.Equal(["containment"], plan.Comparators);
    }

    private static PreparedComparison Prepared(params NormalizedInput[] inputs)
    {
        var plan = new ComparisonPlan(
            "Containment QA",
            ComparisonSelector.ForSource("target"),
            [ComparisonSelector.ForSource("container")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["containment"]
            );
        var selected = new List<WindowRecord>(inputs.Length);
        var normalized = new List<NormalizedWindowRecord>(inputs.Length);

        for (var i = 0; i < inputs.Length; i++)
        {
            var input = inputs[i];
            var source = input.Side == ComparisonSide.Target ? "target" : "container";
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
