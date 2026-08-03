using BenchmarkDotNet.Attributes;
using Spanfold;

namespace Spanfold.Benchmarks;

[MemoryDiagnoser]
public class TransitionComparatorBenchmarks
{
    private WindowComparisonBuilder leadLagComparison = null!;
    private WindowComparisonBuilder asOfComparison = null!;

    [Params(64, 256, 1_024)]
    public int TransitionCountPerSide { get; set; }

    [GlobalSetup]
    public void GlobalSetup()
    {
        var history = TransitionComparatorBenchmarkData.CreateHistory(TransitionCountPerSide);
        this.leadLagComparison = TransitionComparatorBenchmarkData.CreateComparison(history)
            .Using(comparators => comparators.LeadLag(
                LeadLagTransition.Start,
                TemporalAxis.ProcessingPosition,
                TransitionComparatorBenchmarkData.ToleranceMagnitude));
        this.asOfComparison = TransitionComparatorBenchmarkData.CreateComparison(history)
            .Using(comparators => comparators.AsOf(
                AsOfDirection.Previous,
                TemporalAxis.ProcessingPosition,
                TransitionComparatorBenchmarkData.ToleranceMagnitude));

        VerifyLeadLagSemantics(this.leadLagComparison.Run());
        VerifyAsOfSemantics(this.asOfComparison.Run());
    }

    [Benchmark]
    public ComparisonResult LeadLagStart()
    {
        return this.leadLagComparison.Run();
    }

    [Benchmark]
    public ComparisonResult AsOfPrevious()
    {
        return this.asOfComparison.Run();
    }

    private void VerifyLeadLagSemantics(ComparisonResult result)
    {
        var hasExpectedMatches = result.LeadLagRows.Count == TransitionCountPerSide
            && result.LeadLagRows.All(row =>
                row.ComparisonRecordId is not null
                && row.DeltaMagnitude == TransitionComparatorBenchmarkData.ExpectedDeltaMagnitude
                && row.IsWithinTolerance);
        if (!hasExpectedMatches)
        {
            throw new InvalidOperationException("Lead/lag benchmark history did not produce the expected matched rows.");
        }
    }

    private void VerifyAsOfSemantics(ComparisonResult result)
    {
        var hasExpectedMatches = result.AsOfRows.Count == TransitionCountPerSide
            && result.AsOfRows.All(row =>
                row.MatchedRecordId is not null
                && row.DistanceMagnitude == TransitionComparatorBenchmarkData.ExpectedDeltaMagnitude
                && row.Status == AsOfMatchStatus.Matched);
        if (!hasExpectedMatches)
        {
            throw new InvalidOperationException("As-of benchmark history did not produce the expected previous matches.");
        }
    }
}

internal static class TransitionComparatorBenchmarkData
{
    internal const long ExpectedDeltaMagnitude = 1;
    internal const long ToleranceMagnitude = 2;

    private const string AgainstSource = "against";
    private const string Key = "dense-scope";
    private const string Partition = "partition-0";
    private const long TransitionStride = 10;
    private const string TargetSource = "target";
    private const string WindowName = "State";

    internal static WindowHistory CreateHistory(int transitionCountPerSide)
    {
        var windows = new ClosedWindow[transitionCountPerSide * 2];
        for (var index = 0; index < transitionCountPerSide; index++)
        {
            var againstStart = index * TransitionStride;
            windows[index * 2] = new ClosedWindow(
                WindowName,
                Key,
                againstStart,
                againstStart + 4,
                Source: AgainstSource,
                Partition: Partition);

            var targetStart = againstStart + ExpectedDeltaMagnitude;
            windows[(index * 2) + 1] = new ClosedWindow(
                WindowName,
                Key,
                targetStart,
                targetStart + 4,
                Source: TargetSource,
                Partition: Partition);
        }

        return WindowHistory.FromRecords(windows, []);
    }

    internal static WindowComparisonBuilder CreateComparison(WindowHistory history)
    {
        return history.Compare("Dense transition matching")
            .Target(TargetSource, selector => selector.Source(TargetSource))
            .Against(AgainstSource, selector => selector.Source(AgainstSource))
            .Within(scope => scope.Window(WindowName));
    }
}
