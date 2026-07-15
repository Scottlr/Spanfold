using Spanfold;
using Spanfold.Comparison;

var start = DateTimeOffset.Parse("2026-04-21T12:00:00Z");

var pipeline = EventPipeline
    .For<ReplicaState>()
    .RecordWindows()
    .WithEventTime(state => state.Timestamp)
    .Window("ReplicaUnavailable", window => window
        .Key(state => state.ShardId)
        .ActiveWhen(state => !state.IsAvailable)
        .Segment("region", segment => segment.Value(state => state.Region))
        .Tag("replicaRole", state => state.Role))
    .Build();

PrintScenario();

// The primary is compared against a cohort of secondaries. The cohort rule is
// stricter than pairwise comparison: both secondary lanes must be unavailable.
Ingest("replica-a", 0, true, "primary", "region-1");
Ingest("replica-b", 0, true, "secondary", "region-1");
Ingest("replica-c", 0, true, "secondary", "region-2");
Ingest("replica-a", 10, false, "primary", "region-1");
Ingest("replica-b", 12, false, "secondary", "region-1");
Ingest("replica-c", 15, false, "secondary", "region-2");
Ingest("replica-b", 25, true, "secondary", "region-1");
Ingest("replica-a", 30, true, "primary", "region-1");
Ingest("replica-c", 35, true, "secondary", "region-2");

var quorumGap = pipeline.History
    .Compare("Primary unavailable without quorum corroboration")
    .Target("primary", selector => selector.Source("replica-a"))
    .AgainstCohort("replica cohort", cohort => cohort
        .Sources("replica-b", "replica-c")
        .Activity(CohortActivity.AtLeast(2)))
    .Within(scope => scope.Window("ReplicaUnavailable"))
    .Using(comparators => comparators.Residual().Coverage())
    .Run();

var matrix = pipeline.History.CompareSources(
    "Replica source matrix",
    "ReplicaUnavailable",
    ["replica-a", "replica-b", "replica-c"]);

Console.WriteLine("Distributed quorum analysis");
Console.WriteLine("primary residual rows against cohort: " + quorumGap.ResidualRows.Count);
Console.WriteLine("coverage summaries: " + quorumGap.CoverageSummaries.Count);
Console.WriteLine("source-matrix cells: " + matrix.Cells.Count);

void PrintScenario()
{
    Console.WriteLine(
        """
        Scenario
        --------
        replica-a: unavailable 10..30
        replica-b:   unavailable 12..25
        replica-c:      unavailable 15..35

        cohort rule: replica-b AND replica-c are unavailable
        residual:    primary unavailable while the cohort rule is false

        """);
}

void Ingest(string source, int second, bool isAvailable, string role, string region)
{
    pipeline.Ingest(
        new ReplicaState("shard-42", role, region, isAvailable, start.AddSeconds(second)),
        source: source);
}

public sealed record ReplicaState(
    string ShardId,
    string Role,
    string Region,
    bool IsAvailable,
    DateTimeOffset Timestamp);
