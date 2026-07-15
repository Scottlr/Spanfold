using Spanfold;
using Spanfold.Comparison;

var start = DateTimeOffset.Parse("2031-11-04T00:00:00Z");

var pipeline = EventPipeline
    .For<SpacecraftTelemetry>()
    .RecordWindows()
    .WithEventTime(sample => sample.Timestamp)
    .Window("ThermalControlAnomaly", window => window
        .Key(sample => sample.ComponentId)
        .ActiveWhen(sample => sample.TemperatureKelvin > sample.TemperatureLimitKelvin || sample.CoolantFlowGramsPerSecond < 1.5)
        .Segment("missionPhase", segment => segment
            .Value(sample => sample.MissionPhase)
            .Child("attitudeMode", child => child.Value(sample => sample.AttitudeMode)))
        .Tag("instrument", sample => sample.InstrumentId))
    .RollUp(
        "InstrumentThermalConstraint",
        sample => sample.InstrumentId,
        children => children.ActiveCount >= 2,
        segments => segments
            .Preserve("missionPhase")
            .Preserve("attitudeMode"))
    .RollUp(
        "SpacecraftThermalConstraint",
        sample => sample.SpacecraftId,
        children => children.ActiveCount > 0,
        segments => segments.Preserve("missionPhase"))
    .Build();

PrintScenario();

// Component anomalies roll up to instrument constraints, then spacecraft
// constraints. The comparison asks whether model-predicted constraints were
// observed by flight telemetry, and the live view keeps an open burn constraint.
Ingest("thermal-model", 0, "cruise", "sun-point", "camera", "radiator-a", 285, 290, 2.4);
Ingest("flight-telemetry", 0, "cruise", "sun-point", "camera", "radiator-a", 284, 290, 2.5);
Ingest("thermal-model", 8, "approach", "slew", "camera", "radiator-a", 296, 290, 1.2);
Ingest("thermal-model", 9, "approach", "slew", "camera", "coolant-loop-a", 291, 290, 1.1);
Ingest("flight-telemetry", 12, "approach", "slew", "camera", "radiator-a", 297, 290, 1.3);
Ingest("flight-telemetry", 14, "approach", "slew", "camera", "coolant-loop-a", 292, 290, 1.2);
Ingest("thermal-model", 25, "approach", "science-point", "camera", "radiator-a", 287, 290, 2.1);
Ingest("thermal-model", 26, "approach", "science-point", "camera", "coolant-loop-a", 286, 290, 2.2);
Ingest("flight-telemetry", 32, "approach", "science-point", "camera", "radiator-a", 288, 290, 2.0);
Ingest("flight-telemetry", 34, "approach", "science-point", "camera", "coolant-loop-a", 287, 290, 2.2);
Ingest("thermal-model", 45, "orbital-insertion", "burn", "spectrometer", "detector-b", 305, 300, 1.0);
Ingest("thermal-model", 46, "orbital-insertion", "burn", "spectrometer", "cooler-b", 302, 300, 1.1);

var instrumentRows = pipeline.History
    .Compare("Thermal model versus flight telemetry")
    .Target("thermal-model", selector => selector.Source("thermal-model"))
    .Against("flight-telemetry", selector => selector.Source("flight-telemetry"))
    .Within(_ => ComparisonScope.Window("InstrumentThermalConstraint", TemporalAxis.Timestamp).Segment("missionPhase", "approach"))
    .Normalize(normalization => normalization.OnEventTime())
    .Using(comparators => comparators.Overlap().Residual().LeadLag(LeadLagTransition.Start, TemporalAxis.Timestamp, TimeSpan.FromMinutes(10).Ticks).Containment())
    .Run();

var spacecraftLive = pipeline.History
    .Compare("Live spacecraft thermal constraint")
    .Target("thermal-model", selector => selector.Source("thermal-model"))
    .Against("flight-telemetry", selector => selector.Source("flight-telemetry"))
    .Within(scope => scope.Window("SpacecraftThermalConstraint"))
    .Using(comparators => comparators.Residual().Coverage())
    .RunLive(TemporalPoint.ForPosition(13));

var snapshot = pipeline.History.SnapshotAt(TemporalPoint.ForPosition(13));
var phaseSummary = snapshot.Query()
    .Window("ThermalControlAnomaly")
    .Windows()
    .SummarizeBySegment("missionPhase");

var liveAnomaly = pipeline.History.Query()
    .Window("ThermalControlAnomaly")
    .Lane("thermal-model")
    .LatestWindowAt(TemporalPoint.ForPosition(13));

if (liveAnomaly is not null)
{
    pipeline.History.Annotate(
        liveAnomaly.Window,
        "analysisNote",
        "burn attitude constraint requires follow-up thermal vacuum model",
        TemporalPoint.ForPosition(13));
}

Console.WriteLine("Space mission advanced thermal analysis");
Console.WriteLine("instrument overlap rows: " + instrumentRows.OverlapRows.Count);
Console.WriteLine("instrument residual rows: " + instrumentRows.ResidualRows.Count);
Console.WriteLine("lead/lag rows: " + instrumentRows.LeadLagRows.Count);
Console.WriteLine("spacecraft provisional rows: " + spacecraftLive.ProvisionalRowFinalities().Count);
Console.WriteLine("phase summaries: " + phaseSummary.Count);
Console.WriteLine("latest live anomaly annotations: " + (liveAnomaly is null ? 0 : pipeline.History.AnnotationsFor(liveAnomaly.Window).Count));

void PrintScenario()
{
    Console.WriteLine(
        """
        Scenario
        --------
        component windows
          radiator-a:      model 08..25 | telemetry 12..32
          coolant-loop-a:  model 09..26 | telemetry 14..34
          detector-b:      model 45..live
          cooler-b:        model 46..live

        hierarchy
          component anomaly
              -> instrument thermal constraint when >= 2 components active
                  -> spacecraft thermal constraint when any instrument constrained

        analysis
          approach phase: compare model against telemetry on event time
          burn phase:     live open constraint with provisional rows

        """);
}

void Ingest(
    string source,
    int minute,
    string missionPhase,
    string attitudeMode,
    string instrument,
    string component,
    double temperature,
    double limit,
    double coolantFlow)
{
    pipeline.Ingest(
        new SpacecraftTelemetry(
            "voyager-analogue",
            instrument,
            component,
            missionPhase,
            attitudeMode,
            temperature,
            limit,
            coolantFlow,
            start.AddMinutes(minute)),
        source: source);
}

public sealed record SpacecraftTelemetry(
    string SpacecraftId,
    string InstrumentId,
    string ComponentId,
    string MissionPhase,
    string AttitudeMode,
    double TemperatureKelvin,
    double TemperatureLimitKelvin,
    double CoolantFlowGramsPerSecond,
    DateTimeOffset Timestamp);
