using Spanfold;
using Spanfold.Artifacts;
using Spanfold.Comparison;

var start = DateTimeOffset.Parse("2026-04-19T12:00:00Z");

var pipeline = EventPipeline
    .For<ServiceHealthSignal>()
    .RecordWindows()
    .WithEventTime(signal => signal.Timestamp)
    .Window(
        "ServiceDegraded",
        key: signal => signal.ServiceId,
        isActive: signal => !signal.IsHealthy)
    .RollUp(
        "RegionImpacted",
        key: signal => signal.RegionId,
        isActive: children => children.AnyActive())
    .Build();

PrintScenario();

// The child service window rolls up to a region-level impact window. The first
// comparison is historical; the second clips an open monitor-a window live.
Ingest("monitor-a", 0, isHealthy: true);
Ingest("monitor-b", 1, isHealthy: true);
Ingest("monitor-a", 2, isHealthy: false);
Ingest("monitor-b", 4, isHealthy: false);
Ingest("monitor-b", 8, isHealthy: true);
Ingest("monitor-a", 10, isHealthy: true);

var historical = pipeline.History
    .Compare("Monitor degradation QA")
    .Target("monitor-a", selector => selector.Source("monitor-a"))
    .Against("monitor-b", selector => selector.Source("monitor-b"))
    .Within(scope => scope.Window("ServiceDegraded"))
    .Using(comparators => comparators
        .Overlap()
        .Residual()
        .Coverage()
        .LeadLag(LeadLagTransition.Start, TemporalAxis.ProcessingPosition, toleranceMagnitude: 2))
    .Run();

Console.WriteLine("Historical monitor QA");
Console.WriteLine("overlap rows: " + historical.OverlapRows.Count);
Console.WriteLine("monitor-a residual rows: " + historical.ResidualRows.Count);
Console.WriteLine("coverage summaries: " + historical.CoverageSummaries.Count);
Console.WriteLine("lead/lag rows: " + historical.LeadLagRows.Count);

Ingest("monitor-a", 14, isHealthy: false);

var live = pipeline.History
    .Compare("Live degradation QA")
    .Target("monitor-a", selector => selector.Source("monitor-a"))
    .Against("monitor-b", selector => selector.Source("monitor-b"))
    .Within(scope => scope.Window("ServiceDegraded"))
    .Using(comparators => comparators.Residual())
    .RunLive(TemporalPoint.ForPosition(20));

Console.WriteLine();
Console.WriteLine("Live monitor QA");
Console.WriteLine("residual rows: " + live.ResidualRows.Count);
Console.WriteLine("provisional rows: " + live.ProvisionalRowFinalities().Count);
Console.WriteLine();
Console.WriteLine(live.ExportMarkdown());

void PrintScenario()
{
    Console.WriteLine(
        """
        Scenario
        --------
        monitor-a: degradation 02..10, then 14..live
        monitor-b: degradation 04..08

        hierarchy:
          ServiceDegraded(checkout-api)
              -> RegionImpacted(eu-west)

        """);
}

void Ingest(string provider, int minuteOffset, bool isHealthy)
{
    pipeline.Ingest(
        new ServiceHealthSignal(
            ClusterId: "cluster-101",
            RegionId: "eu-west",
            ServiceId: "checkout-api",
            Provider: provider,
            IsHealthy: isHealthy,
            Timestamp: start.AddMinutes(minuteOffset)),
        source: provider);
}

public sealed record ServiceHealthSignal(
    string ClusterId,
    string RegionId,
    string ServiceId,
    string Provider,
    bool IsHealthy,
    DateTimeOffset Timestamp);
