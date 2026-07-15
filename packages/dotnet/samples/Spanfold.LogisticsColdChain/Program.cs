using Spanfold;
using Spanfold.Comparison;

var start = DateTimeOffset.Parse("2026-04-21T06:00:00Z");

var pipeline = EventPipeline
    .For<ShipmentTelemetry>()
    .RecordWindows()
    .WithEventTime(update => update.Timestamp)
    .Window("TemperatureExcursion", window => window
        .Key(update => update.ShipmentId)
        .ActiveWhen(update => update.TemperatureCelsius is < 2.0 or > 8.0)
        .Segment("leg", segment => segment.Value(update => update.Leg))
        .Segment("custody", segment => segment.Value(update => update.Custody))
        .Tag("productClass", update => update.ProductClass))
    .Build();

PrintScenario();

// The shipment remains the same logical key, but the analytical boundaries
// change as it moves through legs and custody owners.
Ingest("sensor-gateway", 0, "warehouse", "carrier-a", 5.0);
Ingest("carrier-feed", 0, "warehouse", "carrier-a", 5.2);
Ingest("sensor-gateway", 20, "road", "carrier-a", 9.4);
Ingest("carrier-feed", 25, "road", "carrier-a", 9.1);
Ingest("sensor-gateway", 50, "road", "carrier-a", 6.0);
Ingest("carrier-feed", 60, "road", "carrier-a", 5.8);
Ingest("sensor-gateway", 80, "handoff", "carrier-b", 1.4);

var snapshot = pipeline.History.SnapshotAt(TemporalPoint.ForPosition(8));
var byLeg = snapshot.Query()
    .Window("TemperatureExcursion")
    .Windows()
    .SummarizeBySegment("leg");

var comparison = pipeline.History
    .Compare("Cold-chain telemetry agreement")
    .Target("sensor-gateway", selector => selector.Source("sensor-gateway"))
    .Against("carrier-feed", selector => selector.Source("carrier-feed"))
    .Within(_ => ComparisonScope.Window("TemperatureExcursion", TemporalAxis.Timestamp).Segment("leg", "road"))
    .Normalize(normalization => normalization.OnEventTime())
    .Using(comparators => comparators.Overlap().Residual().LeadLag(LeadLagTransition.Start, TemporalAxis.Timestamp, TimeSpan.FromMinutes(10).Ticks))
    .Run();

Console.WriteLine("Logistics cold-chain telemetry");
Console.WriteLine("leg summaries: " + byLeg.Count);
Console.WriteLine("road overlap rows: " + comparison.OverlapRows.Count);
Console.WriteLine("gateway-only rows: " + comparison.ResidualRows.Count);
Console.WriteLine("lead/lag rows: " + comparison.LeadLagRows.Count);

void PrintScenario()
{
    Console.WriteLine(
        """
        Scenario
        --------
        warehouse: safe
        road:      gateway excursion 20..50
                   carrier excursion 25..60
        handoff:   gateway excursion 80..live

        Segments split the same shipment into leg/custody windows.

        """);
}

void Ingest(string source, int minute, string leg, string custody, double temperature)
{
    pipeline.Ingest(
        new ShipmentTelemetry("shipment-2048", leg, custody, "biomedical-reagent", temperature, start.AddMinutes(minute)),
        source: source);
}

public sealed record ShipmentTelemetry(
    string ShipmentId,
    string Leg,
    string Custody,
    string ProductClass,
    double TemperatureCelsius,
    DateTimeOffset Timestamp);
