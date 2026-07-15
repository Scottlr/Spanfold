using System.Globalization;
using Spanfold;

namespace Spanfold.Benchmarks;

public sealed class SegmentCohortBenchmarkData
{
    private SegmentCohortBenchmarkData(
        WindowHistory history,
        IReadOnlyList<SegmentCohortBenchmarkEvent> events,
        int deviceCount,
        int sourceCount)
    {
        History = history;
        Events = events;
        DeviceCount = deviceCount;
        SourceCount = sourceCount;
    }

    public WindowHistory History { get; }

    public IReadOnlyList<SegmentCohortBenchmarkEvent> Events { get; }

    public int EventCount => Events.Count;

    public int DeviceCount { get; }

    public int SourceCount { get; }

    public static SegmentCohortBenchmarkData Create(int eventCount)
    {
        var events = CreateEvents(eventCount, deviceCount: 128, sourceCount: 4);
        var pipeline = CreatePipeline();

        for (var i = 0; i < events.Length; i++)
        {
            pipeline.Ingest(events[i].Signal, events[i].Source);
        }

        return new SegmentCohortBenchmarkData(
            pipeline.History,
            events,
            deviceCount: 128,
            sourceCount: 4);
    }

    public static EventPipeline<BenchmarkSegmentSignal> CreatePipeline()
    {
        return global::Spanfold.EventPipeline
            .For<BenchmarkSegmentSignal>()
            .RecordWindows()
            .Window("DeviceOffline", window => window
                .Key(signal => signal.DeviceId)
                .ActiveWhen(signal => !signal.IsOnline)
                .Segment("phase", phase => phase
                    .Value(signal => signal.Phase)
                    .Child("period", period => period
                        .Value(signal => signal.Period)
                        .Child("state", state => state.Value(signal => signal.State))))
                .Tag("fixture", signal => signal.FixtureId))
            .RollUp(
                "MarketOffline",
                signal => signal.MarketId,
                children => children.ActiveCount > 0,
                segments => segments
                    .Preserve("phase")
                    .Preserve("period")
                    .Drop("state"))
            .Build();
    }

    private static SegmentCohortBenchmarkEvent[] CreateEvents(
        int eventCount,
        int deviceCount,
        int sourceCount)
    {
        var events = new SegmentCohortBenchmarkEvent[eventCount];
        var occurrences = new int[deviceCount * sourceCount];

        for (var eventIndex = 0; eventIndex < events.Length; eventIndex++)
        {
            var deviceIndex = eventIndex % deviceCount;
            var sourceIndex = (eventIndex / deviceCount) % sourceCount;
            var occurrenceIndex = (deviceIndex * sourceCount) + sourceIndex;
            var occurrence = occurrences[occurrenceIndex];
            occurrences[occurrenceIndex] = occurrence + 1;

            var source = "provider-" + sourceIndex.ToString(CultureInfo.InvariantCulture);
            var signal = new BenchmarkSegmentSignal(
                DeviceId: "device-" + deviceIndex.ToString(CultureInfo.InvariantCulture),
                MarketId: "market-" + (deviceIndex / 8).ToString(CultureInfo.InvariantCulture),
                FixtureId: "fixture-" + (deviceIndex / 32).ToString(CultureInfo.InvariantCulture),
                IsOnline: IsOnline(occurrence, sourceIndex),
                Phase: Phase(occurrence),
                Period: Period(occurrence),
                State: State(occurrence, sourceIndex));
            events[eventIndex] = new SegmentCohortBenchmarkEvent(signal, source);
        }

        return events;
    }

    private static bool IsOnline(int occurrence, int sourceIndex)
    {
        return ((occurrence + sourceIndex) % 3) == 0;
    }

    private static string Phase(int occurrence)
    {
        return (occurrence & 1) == 0 ? "Pregame" : "InPlay";
    }

    private static string Period(int occurrence)
    {
        return "P" + ((occurrence % 4) + 1).ToString(CultureInfo.InvariantCulture);
    }

    private static string State(int occurrence, int sourceIndex)
    {
        return ((occurrence + sourceIndex) & 1) == 0 ? "Open" : "Suspended";
    }
}
