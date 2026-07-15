using Spanfold;
using Spanfold.Comparison;
using Spanfold.Liveness;

var start = DateTimeOffset.Parse("2026-04-21T08:00:00Z");

var processPipeline = EventPipeline
    .For<PumpTelemetry>()
    .RecordWindows()
    .WithEventTime(update => update.Timestamp)
    .Window("PumpCavitationRisk", window => window
        .Key(update => update.PumpId)
        .ActiveWhen(update => update.VibrationMmPerSecond > 8.0 && update.IntakePressureBar < 1.8)
        .Segment("operatingMode", segment => segment.Value(update => update.OperatingMode))
        .Tag("line", update => update.LineId))
    .Build();

var livenessPipeline = EventPipeline
    .For<LaneLivenessSignal>()
    .RecordWindows()
    .WithEventTime(signal => signal.OccurredAt)
    .TrackWindow("SensorSilent", window => window
        .Key(signal => signal.Lane)
        .ActiveWhen(signal => signal.IsSilent));

var liveness = LaneLivenessTracker.ForLanes(
    start,
    TimeSpan.FromSeconds(45),
    "sensor-a",
    "sensor-b");

PrintScenario();

// Process-risk windows and silence windows are independent histories. They can
// be queried separately or compared later as normal recorded Spanfold windows.
Observe("sensor-a", 0);
Observe("sensor-b", 0);
IngestPump("sensor-a", 0, 4.2, 2.5, "steady");
IngestPump("sensor-b", 0, 4.5, 2.4, "steady");
IngestPump("sensor-a", 1, 9.5, 1.5, "ramp-up");
CheckLiveness(2);
Observe("sensor-b", 3);
IngestPump("sensor-b", 3, 9.1, 1.4, "ramp-up");
IngestPump("sensor-a", 4, 5.0, 2.3, "steady");
IngestPump("sensor-b", 5, 4.9, 2.4, "steady");

var riskComparison = processPipeline.History
    .Compare("Pump risk sensor agreement")
    .Target("sensor-a", selector => selector.Source("sensor-a"))
    .Against("sensor-b", selector => selector.Source("sensor-b"))
    .Within(_ => ComparisonScope.Window("PumpCavitationRisk", TemporalAxis.Timestamp))
    .Normalize(normalization => normalization.OnEventTime())
    .Using(comparators => comparators.Overlap().Residual().LeadLag(LeadLagTransition.Start, TemporalAxis.Timestamp, TimeSpan.FromMinutes(3).Ticks))
    .Run();

var silence = livenessPipeline.History.Query()
    .Window("SensorSilent")
    .ClosedWindows();
var openSilence = livenessPipeline.History.Query()
    .Window("SensorSilent")
    .OpenWindows();

Console.WriteLine("Industrial telemetry");
Console.WriteLine("risk overlap rows: " + riskComparison.OverlapRows.Count);
Console.WriteLine("risk lead/lag rows: " + riskComparison.LeadLagRows.Count);
Console.WriteLine("closed silence windows: " + silence.Count);
Console.WriteLine("open silence windows: " + openSilence.Count);

void PrintScenario()
{
    Console.WriteLine(
        """
        Scenario
        --------
        process risk:
          sensor-a: cavitation risk 01..04
          sensor-b: cavitation risk 03..05

        liveness:
          both sensors go silent after 00:45; sensor-b recovers at 03:00
          while sensor-a remains silent

        This separates "bad process state" from "no signal".

        """);
}

void Observe(string lane, int minute)
{
    foreach (var signal in liveness.Observe(lane, start.AddMinutes(minute)))
    {
        livenessPipeline.Ingest(signal, source: "liveness");
    }
}

void CheckLiveness(int minute)
{
    foreach (var signal in liveness.Check(start.AddMinutes(minute)))
    {
        livenessPipeline.Ingest(signal, source: "liveness");
    }
}

void IngestPump(string source, int minute, double vibration, double pressure, string mode)
{
    processPipeline.Ingest(
        new PumpTelemetry("line-3", "pump-4", mode, vibration, pressure, start.AddMinutes(minute)),
        source: source);
}

public sealed record PumpTelemetry(
    string LineId,
    string PumpId,
    string OperatingMode,
    double VibrationMmPerSecond,
    double IntakePressureBar,
    DateTimeOffset Timestamp);
