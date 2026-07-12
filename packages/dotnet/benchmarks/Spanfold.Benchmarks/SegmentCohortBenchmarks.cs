using BenchmarkDotNet.Attributes;
using Spanfold;

namespace Spanfold.Benchmarks;

[MemoryDiagnoser]
public class SegmentCohortBenchmarks
{
    private SegmentCohortBenchmarkData data = null!;

    [Params(1_024, 8_192)]
    public int EventCount { get; set; }

    [GlobalSetup]
    public void GlobalSetup()
    {
        this.data = SegmentCohortBenchmarkData.Create(EventCount);
    }

    [Benchmark]
    public WindowHistory IngestSegmentedRollUps()
    {
        var pipeline = SegmentCohortBenchmarkData.CreatePipeline();

        for (var i = 0; i < this.data.Events.Count; i++)
        {
            var item = this.data.Events[i];
            pipeline.Ingest(item.Signal, item.Source);
        }

        return pipeline.History;
    }

    [Benchmark]
    public ComparisonResult RunSegmentFilteredResidual()
    {
        return CreateSegmentBuilder()
            .Against("provider-1", selector => selector.Source("provider-1"))
            .Using(comparators => comparators.Residual())
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunAnyCohortResidual()
    {
        return CreateSegmentBuilder()
            .AgainstCohort("cohort", cohort => cohort
                .Sources("provider-1", "provider-2", "provider-3")
                .Activity(CohortActivity.Any()))
            .Using(comparators => comparators.Residual())
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunAtLeastCohortResidual()
    {
        return CreateSegmentBuilder()
            .AgainstCohort("cohort", cohort => cohort
                .Sources("provider-1", "provider-2", "provider-3")
                .Activity(CohortActivity.AtLeast(2)))
            .Using(comparators => comparators.Residual())
            .Run();
    }

    [Benchmark]
    public ComparisonResult RunLiveSegmentCohortResidual()
    {
        return CreateSegmentBuilder()
            .AgainstCohort("cohort", cohort => cohort
                .Sources("provider-1", "provider-2", "provider-3")
                .Activity(CohortActivity.AtLeast(2)))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(this.data.EventCount + 1));
    }

    private WindowComparisonBuilder CreateSegmentBuilder()
    {
        return this.data.History
            .Compare("Segment cohort benchmark")
            .Target("provider-0", selector => selector.Source("provider-0"))
            .Within(scope => scope
                .Window("DeviceOffline")
                .Segment("phase", "InPlay"));
    }
}
