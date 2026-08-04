[![Spanfold](https://raw.githubusercontent.com/Scottlr/Spanfold/main/docs/assets/brand/spanfold-logo-readme-light.svg)](https://github.com/Scottlr/Spanfold)

# Spanfold

Spanfold is a C# library for recording temporal state windows and comparing them
across sources, providers, or pipeline stages.

It is for systems where the important question is not only "what is the latest
value?", but:

- when was this state active?
- who observed it?
- who missed it?
- where did sources overlap, diverge, lag, or leave gaps?
- what was knowable at a specific point in time?

Spanfold turns event predicates into recorded windows, then gives you a staged
comparison API for auditing those windows.

## Concrete Use Cases

- Compare two monitoring providers and find where one reported an outage that
  the other missed.
- Audit a detector or model decision using only the windows that were knowable
  at the decision point.
- Run live analysis over still-open windows and separate final rows from
  provisional rows.

## What Category Is This?

Spanfold sits between event processing and analytics.

It is not a general stream processor, metrics library, or database abstraction.
It records the periods where a state was active, then compares those periods as
first-class temporal data.

The core flow is:

1. Define when a keyed state is active.
2. Ingest events.
3. Record open and closed windows.
4. Compare windows across sources or stages.
5. Export rows, summaries, diagnostics, and explanations.

## Install the preview

Install the core library, optional artifact and testing layers, and CLI tool:

```bash
dotnet add package Spanfold --version 0.2.0-preview.1
dotnet add package Spanfold.Artifacts --version 0.2.0-preview.1
dotnet add package Spanfold.Testing --version 0.2.0-preview.1
dotnet tool install --global Spanfold.Cli --version 0.2.0-preview.1
```

`Spanfold.Testing` is optional. It is useful in consumer test suites when you want
small comparison fixtures without running a full pipeline.

## First Example: Record Windows

Define the event shape, the key that owns state, and the predicate that says
when the window is active.

```csharp
using Spanfold;
using Spanfold.Comparison;

var pipeline = EventPipeline // Start a Spanfold pipeline definition.
    .For<DeviceSignal>() // Configure the event type that will be ingested.
    .RecordWindows() // Store opened and closed windows for comparison.
    .TrackWindow( // Define one state-driven window.
        "DeviceOffline", // Name the state span that will be recorded.
        key: signal => signal.DeviceId, // Track each device independently.
        isActive: signal => !signal.IsOnline); // Open the window while the device is offline.

pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a"); // Open provider A's window.
pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a"); // Close provider A's window.
pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b"); // Open provider B's window.
pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b"); // Close provider B's window.

public sealed record DeviceSignal(string DeviceId, bool IsOnline); // Define the event shape.
```

`RecordWindows()` is the important switch. It keeps the temporal evidence that
later comparisons, exports, and tests operate on.

### Stabilize noisy window transitions

Use separate entry and exit observations when a noisy signal should not open or
close a window immediately:

```csharp
var pipeline = EventPipeline
    .For<DeviceSignal>()
    .RecordWindows()
    .TrackWindow(
        "DeviceOffline",
        signal => signal.DeviceId,
        signal => !signal.IsOnline,
        options => options.Stabilize(
            exitWhen: signal => signal.IsOnline,
            enterAfter: 2,
            exitAfter: 3));
```

Confirmation is tracked independently for each window key, source, and
partition. A false observation resets the applicable consecutive count. The
event reaching the threshold supplies the transition boundary and, when
opening, the window metadata. While an exit is pending, the committed window
and its roll-up membership remain unchanged. Omitting `Stabilize` preserves the
immediate active-predicate behavior.

## Compare Recorded Windows

Comparison is staged in the order an analyst usually asks the question:
baseline, comparison source, scope, normalization, metric.

```csharp
var result = pipeline.History // Use the recorded window history.
    .Compare("Provider QA") // Name the comparison for reports and exports.
    .Target("provider-a", selector => selector.Source("provider-a")) // Treat provider A as the baseline.
    .Against("provider-b", selector => selector.Source("provider-b")) // Compare provider B against it.
    .Within(scope => scope.Window("DeviceOffline")) // Limit the question to one window family.
    .Using(comparators => comparators // Choose the row sets to emit.
        .Overlap() // Emit rows where both sources agreed the state was active.
        .Residual() // Emit provider-A-only rows.
        .Missing() // Emit provider-B-only rows.
        .Coverage()) // Emit magnitude and coverage summaries.
    .Run(); // Execute the historical comparison.

foreach (var row in result.ResidualRows) // Inspect target-only rows.
{
    Console.WriteLine($"{row.WindowName} {row.Key} missed from {row.Range.Start} to {row.Range.End}"); // Print each missed segment.
}
```

When a consumer needs to persist or project evidence, use the result-owned
typed row/finality views instead of creating ordinal IDs or recomputing the
private identity hash:

```csharp
foreach (var entry in result.ResidualRowsWithFinality())
{
    Console.WriteLine($"{entry.Metadata.RowId} {entry.Metadata.Finality}: {entry.Row.Range.Start}..{entry.Row.Range.End}");
}
```

Equivalent views are available for overlap, missing, coverage, gap,
symmetric-difference, containment, lead/lag, and as-of rows. The row ID is an
opaque identifier owned by the producing C# result and should be preserved as
provided; it is not currently promised to match Rust IDs or remain unchanged
across identity-scheme revisions. `CoverageRow` describes one aligned target
segment, while `CoverageSummaries` is the authority for grouped aggregate
coverage.

The core result is structured data:

- diagnostics
- selected, excluded, and normalized windows
- aligned temporal segments
- comparator rows
- summaries
- finality metadata

Exports are an optional package concern. Add `Spanfold.Artifacts`, then import
`Spanfold.Artifacts` for JSON, JSON Lines, Markdown, debug HTML, parsed artifact
models, and verifiable audit bundles.

For visual debugging, export the result as a standalone HTML artifact:

```csharp
using Spanfold.Artifacts;

result.ExportDebugHtml("artifacts/provider-qa.html"); // Write a browser-readable timeline and segment graph.
```

Comparison execution never writes files. This keeps the analytical result
boundary deterministic; export only after a result exists:

```csharp
var resultWithDebug = pipeline.History // Start from recorded windows.
    .Compare("Provider QA") // Name the comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select the target source.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select the comparison source.
    .Within(scope => scope.Window("DeviceOffline")) // Limit analysis to one window family.
    .Using(comparators => comparators.Overlap().Residual()) // Request agreement and target-only rows.
    .Run();
resultWithDebug.ExportDebugHtml("artifacts/provider-qa.html");
```

The debug HTML file is useful when you need to see why a comparison produced a
row: which source window was active, where the overlap started, where a gap
appeared, and whether live rows are still provisional.

## Compare Occurrences as Episodes

Exact comparison answers where active coverage differed. Episode analysis adds
an occurrence-level question: did both sides observe the same incident, even
when one side reported it as several fragments? The .NET preview surface lives
in `Spanfold.Episodes`; the Rust library provides the corresponding Episode
formation, relation, summary, and reference-scorecard capabilities through its
idiomatic API.

```csharp
using Spanfold.Episodes;

var result = pipeline.History
    .CompareEpisodes("Provider QA")
    .Target("reference", selector => selector.Source("provider-a"))
    .Against("detector", selector => selector.Source("provider-b"))
    .Within(scope => scope.Window("DeviceOffline", TemporalAxis.Timestamp))
    .Normalize(normalization => normalization.OnEventTime())
    .StitchGapsUpTo(TimeSpan.FromMinutes(2))
    .RelateWithin(TimeSpan.FromSeconds(30))
    .Run();

Console.WriteLine(result.Summary.TargetMatchRate);
Console.WriteLine(result.AgainstEpisodes.Summary.MultiFragmentEpisodeRate);
Console.WriteLine(result.Summary.SplitTargetRate);

var scorecard = result.AsReference();
Console.WriteLine(scorecard.Recall);
Console.WriteLine(scorecard.Precision);
```

Suppose the reference outage is `[10:00, 10:30)` and the detector reports
`[10:02, 10:10)` and `[10:12, 10:28)`. A two-minute stitch tolerance forms one
multi-fragment detector episode, so the occurrence is not classified as a
split. Exact comparison still shows the late start, early recovery, and
two-minute inactive gap; episode analysis says the detector saw the same
occurrence and retains both source fragments as evidence. Lower the stitch
tolerance below the two-minute gap and the detector forms two episodes, making
the reference occurrence a `Split` relation.

The tolerances answer different questions. `StitchGapsUpTo` operates within
each side and changes episode formation and counts. `RelateWithin` operates
across already-formed episodes and only decides which occurrences are related;
neither tolerance changes the underlying active coverage.

Relations are computed from the complete bipartite component graph, not by
reducing evidence to nearest pairs:

| Relation | Meaning |
| --- | --- |
| `OneToOne` | One target episode relates to one against episode. |
| `Split` | One target episode relates to multiple against episodes. |
| `Merge` | Multiple target episodes relate to one against episode. |
| `Complex` | Multiple episodes on both sides belong to one component. |
| `UnmatchedTarget` | A target episode has no related against episode. |
| `UnmatchedAgainst` | An against episode has no related target episode. |

Fragments are the authoritative active evidence. An episode envelope measures
elapsed extent from the first fragment start to the last fragment end, while
active magnitude is the union of fragment ranges. For live analysis,
`RunLive(horizon)` marks horizon-dependent episodes and relations provisional
until they can settle relative to that horizon. It does not claim watermark or
late-record completeness.

`EpisodeComparisonResult.Summary` uses neutral target/against terminology.
Call `AsReference()` only when the target side is intentionally authoritative;
that explicit interpretation provides recall and precision.

## Match Ordered Window Sequences

Use `Spanfold.Sequences` when one lane must pass through literal named window
families in onset order. Matches use earliest-completion greedy selection,
never reuse committed evidence, and can carry provisional live windows.

```csharp
using Spanfold.Sequences;

var journeys = pipeline.History
    .MatchSequence("incident journey")
    .Step("Warning")
    .Then("Offline")
    .Then("Recovered")
    .WithMaximumGap(5)
    .Run()
    .Matches;
```

## Assess, Trace, Revise, and Audit

`Spanfold.Assessment` turns acceptance criteria into named, portable rules:

```csharp
using Spanfold.Assessment;

var specification = AssessmentSpecification.Create("provider-gate", rules => rules
    .MinimumCoverage(0.99)
    .MaximumResidualMagnitude(5, AssessmentAggregation.Total)
    .RequireFinalRows());
var assessment = result.Assess(specification);
```

Every violation can carry canonical `ComparisonRowReference` evidence. Follow
that evidence through the result without switching on strings:

```csharp
var residual = result.ResidualRowsWithFinality().First();
ComparisonRowTrace<ResidualRow> trace = result.TraceRow(residual);
```

`ComparisonRevision.Between(previous, current)` reports added, revised, and
retracted rows plus coverage and lead/lag summary deltas. Supplying both
assessments also reports introduced, changed, and resolved violations.

For a durable handoff, write and verify an audit bundle:

```csharp
using Spanfold.Artifacts;

var bundle = AuditBundleWriter.Write("artifacts/provider-gate", result, assessment);
var verification = AuditBundleReader.Open(bundle.Path).Verify();
```

Verification checks file sizes and SHA-256 digests against the manifest. It
proves bundle integrity, not producer authenticity. Row IDs are scoped to the
manifest identity domain; the current .NET contract does not claim row-ID
parity with the Rust package.

The CLI exposes the same boundaries:

```bash
spanfold check fixture.json --spec assessment.json
spanfold suite fixture.json --suite assessment-suite.json
spanfold audit fixture.json --out artifacts/run-42
spanfold verify-bundle artifacts/run-42
spanfold diff artifacts/baseline artifacts/run-42
spanfold episodes episode-plan.json windows.jsonl --format json
```

Portable timestamp-known-at input remains intentionally deferred until the
.NET and Rust runtimes share one explicit clock and availability contract.
Portable Episode analysis schema version 1 is likewise processing-position
only; see the repository's
[portable Episode analysis](../../features/episodes/portable-analysis.md)
contract and shared provider/detector example.

## Query Recorded Windows Directly

Use direct history queries when you need to inspect one lane or key without
building a comparison plan.

```csharp
var windows = pipeline.History.Query() // Start a read-only query over recorded windows.
    .Window("DeviceOffline") // Restrict the query to one window family.
    .Key("device-1") // Restrict the query to one logical key.
    .Lane("provider-a") // Restrict the query to one lane, stored as Source.
    .Segment("lifecycle", "Incident") // Require an analytical segment value.
    .Tag("fleet", "critical") // Require descriptive metadata.
    .ClosedWindows(); // Materialize matching closed windows.

var latest = pipeline.History.Query() // Start another recorded-history query.
    .Window("DeviceOffline") // Query one window family.
    .Lane("provider-a") // Query one lane.
    .LatestWindow(); // Return the latest matching open or closed window.
```

For a live read model without cross-source comparison, create a snapshot at an
explicit horizon:

```csharp
var snapshot = pipeline.History.SnapshotAt(TemporalPoint.ForPosition(100)); // Evaluate history at position 100.
var open = snapshot.Query() // Query the horizon snapshot.
    .Window("DeviceOffline") // Inspect one window family.
    .Lane("provider-a") // Inspect one lane.
    .OpenWindows(); // Return records active at the horizon.

var openQuickCheck = pipeline.History.Query() // Start from the recorded history.
    .Window("DeviceOffline") // Restrict the query to one window family.
    .Lane("provider-a") // Restrict the query to one lane.
    .OpenWindowsAt(TemporalPoint.ForPosition(100)); // Return records active at position 100.

var byLifecycle = snapshot.Query() // Reuse the same horizon snapshot.
    .Window("DeviceOffline") // Keep the summary scoped to one window family.
    .Windows() // Materialize final and provisional records.
    .SummarizeBySegment("lifecycle"); // Group counts and measured length by lifecycle.

var annotation = pipeline.History.Annotate( // Attach metadata discovered after the window opened.
    latest!, // Annotate the latest matching source window.
    "reason", // Name the annotation.
    "maintenance", // Store the explanatory value.
    TemporalPoint.ForPosition(105)); // Record when the annotation became known.

var knownAnnotations = pipeline.History.AnnotationsKnownAt( // Read point-in-time-safe annotations.
    latest!, // Use the same source window.
    TemporalPoint.ForPosition(110)); // Include annotations known by position 110.
```

## What Makes Spanfold Different?

Spanfold's main primitive is not an event, metric, or table row. It is a recorded
state window with comparison semantics.

That gives you:

- explicit open and closed windows
- source-aware comparison
- half-open temporal ranges
- overlap, residual, missing, coverage, gap, containment, lead/lag, and as-of
  comparators
- known-at filtering for point-in-time safety
- live horizons for ongoing windows
- deterministic explanations and exports

## Why Not Just X?

### Event Sourcing

Event sourcing is good for storing durable facts and rebuilding state. Spanfold is
for analyzing the periods where state was active after those facts have been
interpreted.

Use event sourcing to preserve the facts. Use Spanfold when you need to compare
"provider A thought this device was offline from 10 to 20" against "provider B
thought it was offline from 12 to 18".

### Stream Processors

Stream processors are good for online computation, routing, enrichment, and
continuous aggregation. Spanfold is smaller and more focused: it records state
windows and compares them.

Use a stream processor to run a distributed pipeline. Use Spanfold when your C#
code needs an inspectable temporal comparison artifact.

### Observability And Metrics Tools

Metrics tools are good for dashboards, counters, histograms, and alerts. They
usually compress time into aggregates.

Spanfold keeps the individual windows and emits comparison rows. That matters when
you need to answer "which exact window was missed?" rather than "what was the
average error rate?"

### Storing Windows Directly In A Database

A database can store windows, but it will not give you Spanfold's comparison
model by itself: staged plans, source selectors, normalization, live finality,
known-at filtering, diagnostics, and deterministic exports.

Use the database for persistence. Use Spanfold to define and execute the temporal
comparison logic.

## Comparator Families

- `Overlap()` shows agreement.
- `Residual()` shows target-only duration.
- `Missing()` shows comparison-only duration.
- `Coverage()` shows magnitude and coverage summaries.
- `Gap()` shows empty spaces inside an observed scope.
- `SymmetricDifference()` shows disagreement in both directions.
- `Containment()` checks whether one state stays inside another state.
- `LeadLag(...)` measures transition timing drift.
- `AsOf(...)` performs point-in-time lookup without future leakage.

See [Comparator reference](https://github.com/Scottlr/Spanfold/blob/main/docs/comparator-reference.md)
for row shapes and details.

## Live Windows

Historical comparisons reject open windows by default. Live comparisons require
an explicit horizon and mark rows that depend on open windows as provisional.

```csharp
var live = pipeline.History // Use the current recorded history.
    .Compare("Live provider QA") // Name the live comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Keep provider A as baseline.
    .Against("provider-b", selector => selector.Source("provider-b")) // Compare provider B against it.
    .Within(scope => scope.Window("DeviceOffline")) // Analyze one window family.
    .Using(comparators => comparators.Residual()) // Ask for baseline-only duration.
    .RunLive(TemporalPoint.ForPosition(100)); // Clip open windows to position 100.

foreach (var finality in live.ProvisionalRowFinalities()) // Inspect rows that may change later.
{
    Console.WriteLine($"{finality.RowId} is provisional: {finality.Reason}"); // Explain why the row is provisional.
}

live.ExportDebugHtml("artifacts/live-provider-qa.html"); // Capture the current live window graph for debugging.
```

The same plan converges with batch execution when all source windows close.

## Lane Liveness And Silence

Use `LaneLivenessTracker` when sparse reporting should become normal Spanfold
windows. The tracker emits state-change events from explicit observations and
horizon checks; it does not run timers or own distributed heartbeat monitoring.

```csharp
var startedAt = DateTimeOffset.UtcNow; // Choose when liveness tracking starts.
var liveness = LaneLivenessTracker.ForLanes( // Create deterministic liveness state.
    startedAt, // Set the start timestamp.
    TimeSpan.FromSeconds(30), // Mark a lane silent after 30 seconds without reports.
    "provider-a", // Track provider A.
    "provider-b"); // Track provider B.

var silencePipeline = EventPipeline // Build a normal Spanfold pipeline for liveness events.
    .For<LaneLivenessSignal>() // Consume liveness state-change events.
    .RecordWindows() // Record silence windows.
    .WithEventTime(signal => signal.OccurredAt) // Use the actual silence/recovery time.
    .TrackWindow("LaneSilent", window => window // Record one silence window family.
        .Key(signal => signal.Lane) // Track each lane independently.
        .ActiveWhen(signal => signal.IsSilent)); // Open while the lane is silent.

foreach (var signal in liveness.Observe("provider-a", startedAt)) // Record a provider A observation.
{
    silencePipeline.Ingest(signal, source: "liveness"); // Feed state changes into Spanfold.
}

foreach (var signal in liveness.Check(startedAt.AddSeconds(45))) // Evaluate silence at a horizon.
{
    silencePipeline.Ingest(signal, source: "liveness"); // Open silence windows for expired lanes.
}
```

## Boundary Segments

Use segments when a value should split an active window while the state remains
active. This records the analytical shape during ingestion instead of slicing
windows later.

```csharp
var pipeline = EventPipeline // Start a Spanfold pipeline definition.
    .For<DeviceStateChanged>() // Configure the event type that will be ingested.
    .RecordWindows() // Store open and closed windows for comparison.
    .Window("DeviceOffline", window => window // Define one device-state window.
        .Key(update => update.DeviceId) // Track each device independently.
        .ActiveWhen(update => update.IsOffline) // Keep the window open while the device is offline.
        .Segment("lifecycle", lifecycle => lifecycle // Split when the operating lifecycle changes.
            .Value(update => update.Lifecycle) // Attach the current lifecycle value.
            .Child("stage", stage => stage // Nest stage under lifecycle.
                .Value(update => update.Stage))) // Split again when the stage changes.
        .Tag("fleet", update => update.FleetId)) // Attach metadata without splitting.
    .RollUp( // Roll device windows up to their zone.
        "ZoneOffline", // Name the parent window.
        update => update.ZoneId, // Group devices by zone.
        children => children.ActiveCount > 0, // A zone is offline when any device is offline.
        segments => segments // Keep only the dimensions useful at zone level.
            .Preserve("lifecycle") // Keep lifecycle boundaries.
            .Preserve("stage")) // Keep nested stage boundaries.
    .Build(); // Build the pipeline.

public sealed record DeviceStateChanged( // Define the event shape.
    string DeviceId, // Device identity.
    string ZoneId, // Parent zone identity.
    bool IsOffline, // Whether the device is currently offline.
    string Lifecycle, // Normal, maintenance, incident, or another lifecycle.
    string? Stage, // Stage inside the lifecycle, when any.
    string FleetId); // Descriptive metadata.
```

If `Lifecycle` changes while `IsOffline` is still true, Spanfold closes the old
window and opens the new one at the same position. Ranges stay half-open, so
there is no gap or overlap.

Roll-ups preserve segment context by default, so device windows can roll up to
zone and region windows without merging lifecycle evidence. Segment projection
lets a parent intentionally keep, drop, rename, or transform child dimensions:

```csharp
.RollUp( // Roll device windows up to their zone.
    "ZoneOffline", // Name the parent window.
    update => update.ZoneId, // Group devices by zone.
    children => children.ActiveCount > 0, // A zone is offline when any device is offline.
    segments => segments // Shape the parent segment context.
        .Preserve("lifecycle") // Keep lifecycle on the parent.
        .Drop("stage") // Ignore lower-level stage changes at zone level.
        .Rename("lifecycle", "operatingMode") // Emit the parent dimension under a clearer name.
        .Transform("lifecycle", value => value?.ToString()?.ToUpperInvariant())) // Normalize values.
```

## Cohort Comparison

Use cohorts when the comparison side is a group rather than one source. An
any-member cohort is active whenever at least one declared member source is
active. `All()` and `AtLeast(n)` let you ask stricter consensus questions.

```csharp
var unmatched = pipeline.History // Start from recorded segmented windows.
    .Compare("Source A unmatched during escalation") // Name the comparison.
    .Target("source-a", selector => selector.Source("source-a")) // Treat source A as the target.
    .AgainstCohort("cohort", cohort => cohort // Compare against the whole cohort.
        .Sources("source-b", "source-c", "source-d") // Include these member sources.
        .Activity(CohortActivity.Any())) // Any active member covers the cohort.
    .Within(scope => scope // Restrict the analytical question.
        .Window("DeviceOffline") // Use device-offline windows.
        .Segment("lifecycle", "Incident") // Only incident lifecycle windows.
        .Segment("stage", "Escalated") // Only escalated incident windows.
        .Tag("fleet", "critical")) // Only critical-fleet windows.
    .Using(comparators => comparators.Residual()) // Emit target-only rows against the cohort.
    .Run(); // Execute the comparison.

var total = unmatched.ResidualRows.TotalPositionLength(); // Sum unmatched processing-position length.

var evidence = unmatched.CohortEvidence(); // Parse cohort evidence into typed records.
var inactive = evidence.Where(row => !row.IsActive); // Inspect segments the cohort did not cover.
```

Spanfold emits compact cohort evidence metadata for each aligned segment, including
the rule, required active count, actual active count, and whether the cohort was
active. `CohortEvidence()` turns that metadata into typed records for code, and
JSON, Markdown, and debug HTML exports include the same evidence for humans.

## Known-At Safety

Use `KnownAtPosition(...)` when a backtest, replay, or audit must only use
windows that were available at a decision point.

```csharp
var audit = pipeline.History // Start from recorded windows.
    .Compare("Decision audit") // Name the point-in-time audit.
    .Target("detector", selector => selector.Source("detector")) // Treat detector output as the target.
    .Against("monitor", selector => selector.Source("monitor")) // Compare monitor history against it.
    .Within(scope => scope.Window("DeviceOffline")) // Audit the device-offline state.
    .Normalize(normalization => normalization.KnownAtPosition(42)) // Exclude records unavailable at position 42.
    .Using(comparators => comparators.AsOf( // Use a point-in-time lookup comparator.
        AsOfDirection.Previous, // Only match records known before the target point.
        TemporalAxis.ProcessingPosition, // Measure distance on processing positions.
        toleranceMagnitude: 10)) // Reject matches more than 10 positions away.
    .Run(); // Execute the leakage-safe audit.
```

Known-at is availability time. It is separate from event time.

## Testing And Fixtures

Use `Spanfold.Testing` when a test needs a compact comparison history.

```csharp
using Spanfold.Testing; // Import fixture and assertion helpers.

var history = new WindowHistoryFixtureBuilder() // Create a compact recorded history.
    .AddClosedWindow( // Add provider A's segmented window.
        "DeviceOffline", // Use the same window name a pipeline would record.
        "device-1", // Set the logical key.
        1, // Start at processing position 1.
        5, // End before processing position 5.
        window => window // Configure window metadata.
            .Source("provider-a") // Attach the source identity.
            .Segment("lifecycle", "Incident") // Attach a boundary segment.
            .Tag("fleet", "critical")) // Attach a descriptive tag.
    .AddClosedWindow("DeviceOffline", "device-1", 3, 7, source: "provider-b") // Add provider B's window.
    .Build(); // Build a WindowHistory.

var result = history.Compare("Fixture QA") // Create a fixture comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select provider A as target.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select provider B as comparison.
    .Within(scope => scope.Window("DeviceOffline")) // Limit the fixture to one window family.
    .Using(comparators => comparators.Overlap()) // Emit agreement rows.
    .Run(); // Execute the comparison.

SpanfoldAssert.IsValid(result); // Fail the test if Spanfold produced error diagnostics.
```

## Operations Example

The sample in `samples/Spanfold.OperationsExample` compares service degradation
windows across monitoring providers. It demonstrates provider residuals,
coverage, lead/lag, live open windows, and Markdown export.

```bash
dotnet run --project samples/Spanfold.OperationsExample/Spanfold.OperationsExample.csproj # Run the operations monitoring sample.
```

More samples are available in the
[repository samples guide](https://github.com/Scottlr/Spanfold/blob/main/packages/dotnet/samples/README.md), ranging
from simple device monitoring to logistics, security audit, industrial
telemetry, distributed quorum analysis, and advanced spacecraft thermal
analysis.

## More Detail

- [Comparison guide](https://github.com/Scottlr/Spanfold/blob/main/docs/comparison-guide.md)
- [Comparator reference](https://github.com/Scottlr/Spanfold/blob/main/docs/comparator-reference.md)
- [Live finality and changelog](https://github.com/Scottlr/Spanfold/blob/main/docs/live-finality-and-changelog.md)
- [Fixture schema](https://github.com/Scottlr/Spanfold/blob/main/docs/fixture-schema.md)
- [Performance notes](https://github.com/Scottlr/Spanfold/blob/main/docs/performance-notes.md)
