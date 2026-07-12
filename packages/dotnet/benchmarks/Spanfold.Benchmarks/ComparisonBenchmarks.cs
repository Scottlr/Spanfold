using BenchmarkDotNet.Attributes;
using Spanfold;

namespace Spanfold.Benchmarks;

[MemoryDiagnoser]
public class ComparisonBenchmarks
{
    private ComparisonBenchmarkData data = null!;
    private ComparisonResult resultForExport = null!;
    private ComparisonResult liveResultForExport = null!;

    [Params(
        ComparisonScenario.Small,
        ComparisonScenario.Medium,
        ComparisonScenario.Large,
        ComparisonScenario.HighOverlap,
        ComparisonScenario.HighCardinality,
        ComparisonScenario.ManySource,
        ComparisonScenario.DenseSingleScope)]
    public ComparisonScenario Scenario { get; set; }

    [GlobalSetup]
    public void GlobalSetup()
    {
        this.data = ComparisonBenchmarkData.Create(Scenario);
        this.resultForExport = CreateBaseBuilder()
            .Using(comparators => comparators.Overlap().Residual().Missing().Coverage().Gap().SymmetricDifference())
            .Run();
        this.liveResultForExport = CreateBaseBuilder()
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(this.data.EventCount + 1));
    }

    [Benchmark]
    public PreparedComparison Prepare()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Overlap())
            .Prepare();
    }

    [Benchmark]
    public AlignedComparison Align()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Overlap())
            .Prepare()
            .Align();
    }

    [Benchmark]
    public ComparisonResult RunOverlap()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Overlap())
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunResidual()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Residual())
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunCoverage()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Coverage())
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunContainment()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Containment())
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunLeadLag()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.LeadLag(
                LeadLagTransition.Start,
                TemporalAxis.ProcessingPosition,
                toleranceMagnitude: 10))
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunLiveResidual()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(this.data.EventCount + 1));
    }

    [Benchmark]
    public ComparisonResult RunMultiComparator()
    {
        return CreateBaseBuilder()
            .Using(comparators => comparators.Overlap().Residual().Missing().Coverage().Gap().SymmetricDifference())
            .Run();
    }

    [Benchmark]
    public string ExportJson()
    {
        return this.resultForExport.ExportJson();
    }

    [Benchmark]
    public string ExportMarkdown()
    {
        return this.resultForExport.ExportMarkdown();
    }

    [Benchmark]
    public string ExportLiveJson()
    {
        return this.liveResultForExport.ExportJson();
    }

    private WindowComparisonBuilder CreateBaseBuilder()
    {
        return this.data.History.Compare("Benchmark Provider QA")
            .Target("provider-0", selector => selector.Source("provider-0"))
            .Against("provider-1", selector => selector.Source("provider-1"))
            .Within(scope => scope.Window("DeviceOffline"));
    }
}
