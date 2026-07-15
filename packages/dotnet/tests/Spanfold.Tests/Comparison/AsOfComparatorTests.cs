using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class AsOfComparatorTests
{
    [Fact]
    public void AsOfComparatorEmitsExactMatch()
    {
        var result = InvokeRuntime(Prepared(
            "asof:Previous:ProcessingPosition:5",
            new NormalizedInput("Quote", "selection-1", 10, 11, ComparisonSide.Target, "trade"),
            new NormalizedInput("Quote", "selection-1", 10, 20, ComparisonSide.Against, "quote")));

        var row = Assert.Single(result.AsOfRows);
        Assert.Equal(AsOfMatchStatus.Exact, row.Status);
        Assert.Equal(0, row.DistanceMagnitude);
        Assert.NotNull(row.MatchedRecordId);
    }

    [Fact]
    public void AsOfComparatorEmitsPreviousMatchWithinTolerance()
    {
        var result = InvokeRuntime(Prepared(
            "asof:Previous:ProcessingPosition:5",
            new NormalizedInput("Quote", "selection-1", 10, 11, ComparisonSide.Target, "trade"),
            new NormalizedInput("Quote", "selection-1", 7, 20, ComparisonSide.Against, "quote")));

        var row = Assert.Single(result.AsOfRows);
        Assert.Equal(AsOfMatchStatus.Matched, row.Status);
        Assert.Equal(3, row.DistanceMagnitude);
        Assert.Equal(7, row.MatchedPoint!.Value.Position);
    }

    [Fact]
    public void AsOfComparatorEmitsNoMatchOutsideTolerance()
    {
        var result = InvokeRuntime(Prepared(
            "asof:Previous:ProcessingPosition:2",
            new NormalizedInput("Quote", "selection-1", 10, 11, ComparisonSide.Target, "trade"),
            new NormalizedInput("Quote", "selection-1", 5, 20, ComparisonSide.Against, "quote")));

        var row = Assert.Single(result.AsOfRows);
        Assert.Equal(AsOfMatchStatus.NoMatch, row.Status);
        Assert.Equal(5, row.DistanceMagnitude);
        Assert.Null(row.MatchedRecordId);
    }

    [Fact]
    public void AsOfComparatorRejectsFutureMatchForPreviousDirection()
    {
        var result = InvokeRuntime(Prepared(
            "asof:Previous:ProcessingPosition:5",
            new NormalizedInput("Quote", "selection-1", 10, 11, ComparisonSide.Target, "trade"),
            new NormalizedInput("Quote", "selection-1", 12, 20, ComparisonSide.Against, "quote")));

        var row = Assert.Single(result.AsOfRows);
        Assert.Equal(AsOfMatchStatus.FutureRejected, row.Status);
        Assert.Equal(2, row.DistanceMagnitude);
        Assert.Null(row.MatchedRecordId);
    }

    [Fact]
    public void AsOfComparatorCanExplicitlyAllowFutureMatches()
    {
        var result = InvokeRuntime(Prepared(
            "asof:Next:ProcessingPosition:5",
            new NormalizedInput("Quote", "selection-1", 10, 11, ComparisonSide.Target, "trade"),
            new NormalizedInput("Quote", "selection-1", 12, 20, ComparisonSide.Against, "quote")));

        var row = Assert.Single(result.AsOfRows);
        Assert.Equal(AsOfMatchStatus.Matched, row.Status);
        Assert.Equal(2, row.DistanceMagnitude);
        Assert.NotNull(row.MatchedRecordId);
    }

    [Fact]
    public void AmbiguousSameDistanceMatchIsDeterministicAndDiagnosed()
    {
        var result = InvokeRuntime(Prepared(
            "asof:Nearest:ProcessingPosition:5",
            new NormalizedInput("Quote", "selection-1", 10, 11, ComparisonSide.Target, "trade"),
            new NormalizedInput("Quote", "selection-1", 8, 20, ComparisonSide.Against, "quote-a"),
            new NormalizedInput("Quote", "selection-1", 12, 20, ComparisonSide.Against, "quote-b")));

        var row = Assert.Single(result.AsOfRows);
        Assert.Equal(AsOfMatchStatus.Ambiguous, row.Status);
        Assert.Equal(2, row.DistanceMagnitude);
        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.AmbiguousAsOfMatch);
    }

    [Fact]
    public void BuilderRequiresExplicitAsOfOptions()
    {
        var pipeline = EventPipeline
            .For<NormalizedInput>()
            .RecordWindows()
            .TrackWindow("Quote", input => input.Key, static _ => true);

        var plan = pipeline.History
            .Compare("Quote at trade")
            .Target("trade", selector => selector.Source("trade"))
            .Against("quote", selector => selector.Source("quote"))
            .Within(scope => scope.Window("Quote"))
            .Using(comparators => comparators.AsOf(
                AsOfDirection.Previous,
                TemporalAxis.ProcessingPosition,
                toleranceMagnitude: 5))
            .Build();

        Assert.Equal(["asof:Previous:ProcessingPosition:5"], plan.Comparators);
    }

    private static PreparedComparison Prepared(string comparator, params NormalizedInput[] inputs)
    {
        var plan = new ComparisonPlan(
            "Quote at trade",
            ComparisonSelector.ForSource("trade"),
            [ComparisonSelector.ForSource("quote")],
            ComparisonScope.Window("Quote"),
            ComparisonNormalizationPolicy.Default,
            [comparator]
            );
        var selected = new List<WindowRecord>(inputs.Length);
        var normalized = new List<NormalizedWindowRecord>(inputs.Length);

        for (var i = 0; i < inputs.Length; i++)
        {
            var input = inputs[i];
            var source = input.Side == ComparisonSide.Target ? "trade" : "quote";
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
