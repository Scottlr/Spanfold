using Spanfold;

namespace Spanfold.Benchmarks;

public sealed class ComparisonBenchmarkData
{
    private ComparisonBenchmarkData(WindowHistory history, int eventCount, int deviceCount, int sourceCount)
    {
        History = history;
        EventCount = eventCount;
        DeviceCount = deviceCount;
        SourceCount = sourceCount;
    }

    public WindowHistory History { get; }

    public int EventCount { get; }

    public int DeviceCount { get; }

    public int SourceCount { get; }

    public static ComparisonBenchmarkData Create(ComparisonScenario scenario)
    {
        var shape = GetShape(scenario);
        var pipeline = CreatePipeline();
        var occurrences = new int[shape.DeviceCount * shape.SourceCount];

        for (var eventIndex = 0; eventIndex < shape.EventCount; eventIndex++)
        {
            var deviceIndex = eventIndex % shape.DeviceCount;
            var sourceIndex = (eventIndex / shape.DeviceCount) % shape.SourceCount;
            var occurrenceIndex = (deviceIndex * shape.SourceCount) + sourceIndex;
            var occurrence = occurrences[occurrenceIndex];
            occurrences[occurrenceIndex] = occurrence + 1;

            var isOnline = GetOnlineState(scenario, eventIndex, deviceIndex, sourceIndex, occurrence);
            pipeline.Ingest(
                new BenchmarkDeviceSignal("device-" + deviceIndex.ToString(System.Globalization.CultureInfo.InvariantCulture), isOnline),
                source: "provider-" + sourceIndex.ToString(System.Globalization.CultureInfo.InvariantCulture));
        }

        return new ComparisonBenchmarkData(pipeline.History, shape.EventCount, shape.DeviceCount, shape.SourceCount);
    }

    public static EventPipeline<BenchmarkDeviceSignal> CreatePipeline()
    {
        return global::Spanfold.EventPipeline
            .For<BenchmarkDeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);
    }

    private static BenchmarkShape GetShape(ComparisonScenario scenario)
    {
        return scenario switch
        {
            ComparisonScenario.Small => new BenchmarkShape(EventCount: 128, DeviceCount: 16, SourceCount: 2),
            ComparisonScenario.Medium => new BenchmarkShape(EventCount: 1_024, DeviceCount: 128, SourceCount: 2),
            ComparisonScenario.Large => new BenchmarkShape(EventCount: 8_192, DeviceCount: 512, SourceCount: 2),
            ComparisonScenario.HighOverlap => new BenchmarkShape(EventCount: 2_048, DeviceCount: 32, SourceCount: 2),
            ComparisonScenario.HighCardinality => new BenchmarkShape(EventCount: 4_096, DeviceCount: 2_048, SourceCount: 2),
            ComparisonScenario.ManySource => new BenchmarkShape(EventCount: 4_096, DeviceCount: 256, SourceCount: 8),
            ComparisonScenario.DenseSingleScope => new BenchmarkShape(EventCount: 8_192, DeviceCount: 1, SourceCount: 2),
            _ => new BenchmarkShape(EventCount: 1_024, DeviceCount: 128, SourceCount: 2)
        };
    }

    private static bool GetOnlineState(
        ComparisonScenario scenario,
        int eventIndex,
        int deviceIndex,
        int sourceIndex,
        int occurrence)
    {
        return scenario switch
        {
            ComparisonScenario.HighOverlap => (occurrence % 8) >= 4,
            ComparisonScenario.HighCardinality => (occurrence & 1) == 1,
            ComparisonScenario.ManySource => ((occurrence + sourceIndex) & 1) == 1,
            ComparisonScenario.DenseSingleScope => (occurrence & 1) == 1,
            _ => (occurrence & 1) == 1
        };
    }

    private readonly record struct BenchmarkShape(int EventCount, int DeviceCount, int SourceCount);
}
