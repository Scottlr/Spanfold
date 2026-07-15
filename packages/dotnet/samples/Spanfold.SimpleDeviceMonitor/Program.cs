using Spanfold;
using Spanfold.Comparison;

var start = DateTimeOffset.Parse("2026-04-21T09:00:00Z");

var pipeline = EventPipeline
    .For<DeviceSignal>()
    .RecordWindows()
    .WithEventTime(signal => signal.Timestamp)
    .TrackWindow(
        "DeviceOffline",
        key: signal => signal.DeviceId,
        isActive: signal => !signal.IsOnline);

PrintScenario();

// Two independent lanes observe the same device. Spanfold records each lane's
// offline windows, then compares where the lanes agree or diverge.
Ingest("agent-a", 0, true);
Ingest("agent-b", 0, true);
Ingest("agent-a", 5, false);
Ingest("agent-b", 7, false);
Ingest("agent-b", 12, true);
Ingest("agent-a", 15, true);
Ingest("agent-a", 20, false);

var closed = pipeline.History.Query()
    .Window("DeviceOffline")
    .Lane("agent-a")
    .ClosedWindows();

var openAtEight = pipeline.History.Query()
    .Window("DeviceOffline")
    .Lane("agent-a")
    .OpenWindowsAt(TemporalPoint.ForPosition(8));

var comparison = pipeline.History
    .Compare("Simple monitor comparison")
    .Target("agent-a", selector => selector.Source("agent-a"))
    .Against("agent-b", selector => selector.Source("agent-b"))
    .Within(scope => scope.Window("DeviceOffline"))
    .Using(comparators => comparators.Overlap().Residual().Missing().Coverage())
    .RunLive(TemporalPoint.ForPosition(25));

Console.WriteLine("Simple device monitor");
Console.WriteLine("agent-a closed windows: " + closed.Count);
Console.WriteLine("agent-a windows open at position 8: " + openAtEight.Count);
Console.WriteLine("overlap rows: " + comparison.OverlapRows.Count);
Console.WriteLine("agent-a-only rows: " + comparison.ResidualRows.Count);
Console.WriteLine("agent-b-only rows: " + comparison.MissingRows.Count);
Console.WriteLine("provisional rows: " + comparison.ProvisionalRowFinalities().Count);

void PrintScenario()
{
    Console.WriteLine(
        """
        Scenario
        --------
        agent-a: healthy | offline 05..15 | offline 20..live
        agent-b: healthy |   offline 07..12

        Questions:
        - what did agent-a record?
        - what was open at a historical horizon?
        - where did agent-a and agent-b overlap or diverge?

        """);
}

void Ingest(string agent, int minute, bool isOnline)
{
    pipeline.Ingest(
        new DeviceSignal("device-17", isOnline, start.AddMinutes(minute)),
        source: agent);
}

public sealed record DeviceSignal(
    string DeviceId,
    bool IsOnline,
    DateTimeOffset Timestamp);
