# Comparison Guide

Spanfold comparisons answer staged questions over recorded windows:

- target: the source or stage to treat as the baseline
- against: one or more sources or stages to compare with the target
- scope: the window family and temporal axis to analyse
- normalization: how recorded windows become comparable temporal ranges
- comparators: the row sets to emit

The API keeps these stages visible:

```csharp
var result = pipeline.History // Start from the recorded window history.
    .Compare("Provider QA") // Name the comparison for exports and diagnostics.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select the baseline source.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select the comparison source.
    .Within(scope => scope.Window("DeviceOffline")) // Limit analysis to one window family.
    .Using(comparators => comparators.Overlap().Residual().Coverage()) // Request agreement, residual, and coverage rows.
    .Run(); // Execute the comparison.
```

```rust
let result = pipeline
    .history()
    .compare("Provider QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .overlap()
    .residual()
    .coverage()
    .run();
```

Use descriptor selectors such as `Source`, `WindowName`, `Key`, `Partition`,
`PositionRange`, and `TimeRange` whenever a plan needs to be exported or
reviewed by tooling. Runtime selectors are useful locally, but deterministic
JSON export rejects them because a delegate cannot be represented as portable
data.

## Exact Spans or Episodes?

Use an exact window comparison when the decision depends on active coverage:
the precise overlap, missed interval, residual duration, or transition drift.
Use episode analysis when the decision depends on occurrences: whether two
sources saw the same outage, whether one occurrence was fragmented, or whether
several occurrences were merged.

The two views are complementary. For a reference outage `[10:00, 10:30)` and
detector fragments `[10:02, 10:10)` and `[10:12, 10:28)`, exact comparison
preserves the late start, early recovery, and inactive gap. With
`StitchGapsUpTo(TimeSpan.FromMinutes(2))`, episode analysis retains those two
fragments but treats them as one multi-fragment occurrence. Lowering the stitch
tolerance below two minutes forms two detector episodes and can classify the
reference occurrence as `Split`.

Stitch tolerance works within a side and changes episode formation. Relation
tolerance works across the formed target and against episode sets; it connects
nearby occurrences without changing their active coverage. Relations are
classified from complete connected components as `OneToOne`, `Split`, `Merge`,
`Complex`, `UnmatchedTarget`, or `UnmatchedAgainst`, rather than by selecting a
nearest pair.

Fragments are authoritative for active magnitude. The envelope spans elapsed
time from the earliest fragment start to the latest fragment end. Neutral
episode summaries keep target/against language; use `AsReference()` only when
the target side is deliberately the reference. The API is available through
`Spanfold.Episodes` in the .NET preview and through `WindowHistory::form_episodes`
and `WindowHistory::compare_episodes` in Rust.

See the public [Episode analysis guide](episode-analysis.html) for the paired
C# and Rust APIs, all six component kinds, live finality, lineage boundaries,
and portable schema version 1.

## Ordered Journeys Across Window Families

Use an ordered sequence when each step is a different interpreted state and
the question is whether one lane followed a journey such as `Warning` →
`Offline` → `Recovered`. Sequence matching is deliberately narrower than a
complex-event language: it accepts literal named window-family steps and an
optional processing-position gap, with no branching, negation, loops, or
causal claim.

```csharp
using Spanfold.Sequences;

var matches = pipeline.History
    .MatchSequence("incident journey")
    .Step("Warning")
    .Then("Offline")
    .Then("Recovered")
    .WithMaximumGap(5)
    .Run()
    .Matches;
```

```rust
let matches = pipeline
    .history()
    .match_sequence("incident journey")
    .step("Warning")
    .then("Offline")
    .then("Recovered")
    .with_maximum_gap(5)
    .run()?;
```

Steps correlate only within one key, source, and partition lane. Rust uses its
exact string key identity. In .NET, the first step anchors the lane and its
configured history key comparer is applied to every later step; source and
partition still use exact equality.

Ordering uses window onset, so later steps may overlap earlier evidence. A
transition is eligible when its onset is at or after the previous step's onset.
Its inactive gap is `max(0, next start - previous effective end)`, and
`WithMaximumGap(...)` applies that inclusive limit to every transition. A
match starts at the first onset and completes at the latest effective end
across its evidence.

Matching is deterministic earliest-completion greedy. Candidates for each step
are ordered by effective end, then onset, then record ID. The first compatible
unused candidate is selected for each later step; evidence is consumed only
after a complete chain is found, and a committed window is never reused.
Spanfold does not enumerate alternative matches.

Historical `Run()` requires selected evidence to be closed. Use
`RunLive(TemporalPoint.ForPosition(...))` in .NET or
`run_live(TemporalPoint::position(...))` in Rust for current history. Live
matching reuses history snapshots: future evidence is excluded, active evidence
is clipped to the horizon, and a match is provisional when any selected step
is provisional.

## Temporal Model

Processing-position comparisons are the default. Positions are assigned during
ingestion and are deterministic for a replay of the same event order.

Event-time comparisons require timestamps:

```csharp
var result = pipeline.History // Start from recorded windows.
    .Compare("Event-time QA") // Name the event-time comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select provider A as target.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select provider B as comparison.
    .Within(scope => scope.Window("DeviceOffline")) // Scope to one window family.
    .Normalize(normalization => normalization.OnEventTime()) // Compare on event timestamps instead of positions.
    .Using(comparators => comparators.Overlap()) // Emit rows where both sources were active.
    .Run(); // Execute the comparison.
```

```rust
let result = pipeline
    .history()
    .compare("Event-time QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline").on_event_time())
    .normalization(ComparisonNormalizationPolicy::event_time())
    .overlap()
    .run();
```

In .NET, scope and normalization must use the same temporal axis; mixed-axis
plans are rejected because processing positions and timestamps cannot be
compared without an explicit mapping. Rust stores one axis on the plan, so the
last applied `scope(...)` or `normalization(...)` axis wins. Apply them
consistently, as in the example above, rather than expecting Rust to diagnose
two independently retained axis choices.

Ranges are half-open: `[start, end)`. The start is included and the end is
excluded. Touching windows create adjacent segments, not overlaps.

## Open Windows

Open windows are rejected by default in historical comparison. This avoids
silently treating an ongoing window as final.

For live or horizon-based analysis, choose an explicit end:

```csharp
var prepared = pipeline.History // Start from recorded windows.
    .Compare("Live QA") // Name the live preparation.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select the baseline source.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select the comparison source.
    .Within(scope => scope.Window("DeviceOffline")) // Analyze one window family.
    .Normalize(normalization => normalization.ClipOpenWindowsTo(TemporalPoint.ForPosition(100))) // Bound open windows at position 100.
    .Using(comparators => comparators.Coverage()) // Request coverage metrics.
    .Prepare(); // Stop after selection and normalization for inspection.
```

```rust
let prepared = pipeline
    .history()
    .compare("Live QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .normalization(ComparisonNormalizationPolicy::clip_open_windows_to(
        TemporalPoint::position(100),
    ))
    .coverage()
    .prepare();
```

The resulting range records that its end came from the horizon policy, not from
a closed source window.

## Direct History Queries

Use direct history queries when the question is about recorded state history,
not cross-source comparison. This is useful for single-lane analyzers,
debugging, and tests that need to inspect windows by key, lane, partition,
segment, or tag.

```csharp
var windows = pipeline.History.Query() // Start a read-only query over recorded history.
    .Window("DeviceOffline") // Restrict the query to one window family.
    .Key("device-1") // Restrict the query to one logical key.
    .Lane("provider-a") // Restrict the query to one lane, stored as Source.
    .Partition("partition-1") // Restrict the query to one runtime partition.
    .Segment("lifecycle", "Incident") // Require an analytical segment value.
    .Tag("fleet", "critical") // Require descriptive metadata.
    .ClosedWindows(); // Return matching closed source windows.
```

```rust
let windows = pipeline
    .history()
    .query()
    .where_window("DeviceOffline")
    .where_key("device-1")
    .where_source("provider-a")
    .where_partition("partition-1")
    .where_segment("lifecycle", "Incident")
    .where_tag("fleet", "critical")
    .closed_windows();
```

In .NET, `Lane(...)` is a readability alias for `Source(...)`, and the identity
is stored in `WindowRecord.Source`. Rust exposes the same identity through
`where_source(...)` and `WindowRecord::source()`. Lane is often the clearer
domain term when the same key is observed by several feeds, analyzers, or
pipeline stages.

For a current-state read model, evaluate history at an explicit horizon:

```csharp
var snapshot = pipeline.History.SnapshotAt(TemporalPoint.ForPosition(100)); // Evaluate recorded history at position 100.
var open = snapshot.Query() // Start a read-only query over the horizon snapshot.
    .Window("DeviceOffline") // Restrict the query to one window family.
    .Lane("provider-a") // Restrict the query to one lane.
    .OpenWindows(); // Return records active at the horizon.

var openQuickCheck = pipeline.History.Query() // Start from the recorded history.
    .Window("DeviceOffline") // Restrict the query to one window family.
    .Lane("provider-a") // Restrict the query to one lane.
    .OpenWindowsAt(TemporalPoint.ForPosition(100)); // Return records active at position 100.

foreach (var record in open) // Inspect each active snapshot record.
{
    Console.WriteLine($"{record.Window.Key}: {record.Range.Start} to {record.Range.End}"); // Print the clipped horizon range.
}

var byLifecycle = snapshot.Query() // Reuse the same horizon snapshot.
    .Window("DeviceOffline") // Keep the summary scoped to one window family.
    .Windows() // Materialize final and provisional records.
    .SummarizeBySegment("lifecycle"); // Group counts and measured length by lifecycle.
```

```rust
let horizon = TemporalPoint::position(100);
let snapshot = pipeline.history().snapshot_at(horizon.clone())?;
let open = snapshot
    .query()
    .where_window("DeviceOffline")
    .where_source("provider-a")
    .open_windows();

let open_quick_check = WindowHistoryQuery::new(pipeline.history().windows())
    .where_window("DeviceOffline")
    .where_source("provider-a")
    .open_windows_at(horizon)?;

for record in &open {
    println!(
        "{}: {:?} to {:?}",
        record.window.key(),
        record.range.start(),
        record.range.end()
    );
}

let by_lifecycle = snapshot
    .query()
    .where_window("DeviceOffline")
    .summarize_by_segment("lifecycle")?;
```

Snapshot records preserve the source window and add the range that was visible
at the horizon. A record whose source window had not ended by the horizon is
marked provisional and clipped to that horizon. The underlying history is not
mutated.

Summaries are deliberately small. Both runtimes report record and
final/provisional counts plus measured processing-position length. .NET also
reports measured event-time duration; Rust's current `WindowGroupSummary` does
not aggregate timestamp duration. Spanfold does not impose a reporting schema
on top of those groups.

## Late Annotations

Use annotations when explanatory metadata arrives after a window has already
opened. Annotation is append-only and external to the recorded window: it does
not mutate the source record, split the range, or change comparison output.

```csharp
var openWindow = pipeline.History.Query() // Start a direct history query.
    .Window("DeviceOffline") // Restrict the query to one window family.
    .Lane("provider-a") // Restrict the query to one lane.
    .LatestWindow() // Select the latest matching source window.
    ?? throw new InvalidOperationException("No matching window was recorded.");

var annotation = pipeline.History.Annotate( // Attach metadata to the window start identity.
    openWindow, // Use the source window being explained.
    "reason", // Name the annotation.
    "maintenance", // Store the explanatory value.
    TemporalPoint.ForPosition(105)); // Record when the annotation became known.

var annotations = pipeline.History.AnnotationsFor(openWindow); // Read annotations back in append order.
var knownAnnotations = pipeline.History.AnnotationsKnownAt( // Read point-in-time-safe annotations.
    openWindow, // Use the same source window.
    TemporalPoint.ForPosition(110)); // Include annotations known by position 110.
```

```rust
let open_window = history
    .query()
    .where_window("DeviceOffline")
    .where_source("provider-a")
    .latest()
    .expect("a matching window");
let target = WindowAnnotationTarget::from_window(&open_window);

let annotation = history.annotate(
    target.clone(),
    "reason",
    "maintenance",
    Some(TemporalPoint::position(105)),
);
let annotations = history.annotations_for(&target);
let known_annotations =
    history.annotations_known_at(&target, TemporalPoint::position(110));
```

Rust annotation methods operate on `&mut WindowHistory`. An `EventPipeline`
exposes its history read-only, so applications that need to append annotations
must own or import a mutable history rather than mutate it through
`pipeline.history()`.

The annotation target excludes the window end. If metadata is attached while a
window is open, the same annotation remains associated after that window closes.
Repeated annotations with the same name append revisions instead of overwriting
earlier metadata.

`AnnotationsKnownAt(...)` excludes annotations without a comparable known-at
point. That keeps audit reads from accidentally using explanatory metadata that
was not available at the decision horizon.

## Known-At Safety

Known-at filtering answers "what records were available at this processing
position?" It is availability time, not event time. Closed windows become
available when their close position has been processed; open windows are
available from their start position.

```csharp
var prepared = pipeline.History // Start from recorded windows.
    .Compare("Decision audit") // Name the point-in-time audit.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select the target source.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select the comparison source.
    .Within(scope => scope.Window("DeviceOffline")) // Audit one window family.
    .Normalize(normalization => normalization.KnownAtPosition(42)) // Exclude windows unavailable at position 42.
    .Using(comparators => comparators.Overlap()) // Emit agreement rows.
    .Prepare(); // Inspect prepared data and diagnostics without comparator execution.
```

```rust
let prepared = pipeline
    .history()
    .compare("Decision audit")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .known_at_position(42)
    .overlap()
    .prepare();
```

Records unavailable at the known-at point are excluded with diagnostics so
backtests and decision audits do not silently leak future information.

## Live Snapshots

Use `RunLive(horizon)` when the last window may still be open. Spanfold clips open
windows to the supplied evaluation horizon and marks rows that depend on those
windows as provisional. Closed-window rows remain final, and the same comparison
converges with batch execution once all windows close.

```csharp
var result = pipeline.History // Start from current recorded history.
    .Compare("Live QA") // Name the live comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select provider A as target.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select provider B as comparison.
    .Within(scope => scope.Window("DeviceOffline")) // Scope to device-offline windows.
    .Using(comparators => comparators.Residual()) // Emit target-only rows.
    .RunLive(TemporalPoint.ForPosition(100)); // Clip open windows to position 100.
```

```rust
let result = pipeline
    .history()
    .compare("Live QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .residual()
    .run_live(TemporalPoint::position(100));
```

The result carries `EvaluationHorizon` and row finality metadata so consumers can
separate current-state insight from final historical evidence.

## Lane Liveness And Silence

Use `LaneLivenessTracker` when sparse reporting, heartbeat loss, or insufficient
signal should be represented as ordinary recorded windows. The tracker is
deterministic and explicit: consumers call `Observe(...)` when a lane reports
and `Check(...)` at a horizon where silence should be evaluated.

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

```rust
let started_at = TemporalPoint::timestamp_ticks(started_at_ticks);
let mut liveness = LaneLivenessTracker::for_lanes(
    started_at.clone(),
    30 * ticks_per_second,
    ["provider-a", "provider-b"],
)?;

let mut silence_pipeline = for_events::<LaneLivenessSignal>()
    .record_windows()
    .with_event_time(|signal| signal.occurred_at.magnitude())
    .track_window(
        "LaneSilent",
        |signal| signal.lane.clone(),
        |signal| signal.is_silent,
    )
    .build()?;

for signal in liveness.observe("provider-a", started_at.clone())? {
    silence_pipeline.ingest(signal, Some("liveness"), None)?;
}

let horizon = TemporalPoint::timestamp_ticks(
    started_at.magnitude() + 45 * ticks_per_second,
);
for signal in liveness.check(horizon)? {
    silence_pipeline.ingest(signal, Some("liveness"), None)?;
}
```

Rust liveness thresholds are integer magnitudes on the tracker's temporal axis;
for timestamp tracking, callers choose and consistently apply their tick unit.

The tracker emits only liveness state changes, not every heartbeat. A lane that
never reports can still become silent after the configured threshold. A later
observation emits recovery and closes the silence window. Spanfold can then compare,
query, snapshot, summarize, or export those windows like any other recorded
state.

Spanfold deliberately does not own scheduling for this feature. That belongs in the
host application, job runner, actor, or stream processor that already owns lane
health checks.

## Live Revisions

Use `ComparisonChangelog.Create(previous.RowFinalities, current.RowFinalities)`
to audit how live row metadata changed between snapshots. Revised entries
supersede earlier row versions, and retracted entries remove rows that no longer
exist in the current snapshot. `ComparisonChangelog.Replay(...)` rebuilds the
active row-finality view from a prior snapshot plus its changelog entries.

For practical usage patterns, see
[Live finality and changelog](live-finality-and-changelog.md).

## Plan Criticism

Runtime criticism flags plans that are structurally valid but analytically risky:
runtime-only selectors, unrestricted scopes, point-in-time lookup without
known-at safety, open durations without a horizon, live clipping without a
horizon, and incompatible timestamp clocks. Non-strict execution carries these
diagnostics as warnings. Use `Strict()` when warnings should block alignment and
comparator rows.

## Extension Metadata

Domain packages can describe their own selectors, comparator declarations, and
metadata keys with `ComparisonExtensionBuilder`. Core Spanfold stays domain-neutral;
extension descriptors document how a package attaches to plans, while result
`ExtensionMetadata` keeps compact domain metadata serializable and explainable.

## Inspectability

Use `Validate()` before execution when building plans dynamically. Use
`Prepare()` when you need to inspect selected, excluded, and normalized windows.
Use `Run()` when comparator rows are needed.

```csharp
var prepared = pipeline.History // Start from recorded windows.
    .Compare("Provider QA") // Name the comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select the target source.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select the comparison source.
    .Within(scope => scope.Window("DeviceOffline")) // Limit the scope to one window family.
    .Using(comparators => comparators.Overlap()) // Request overlap rows.
    .Prepare(); // Materialize selected, excluded, and normalized windows.

var explanation = prepared.Explain(); // Render deterministic diagnostic text.
```

```rust
let prepared = pipeline
    .history()
    .compare("Provider QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .overlap()
    .prepare();

let explanation = prepared.explain();
```

`Explain()` returns deterministic diagnostic text. It is not generated prose.
`ExportJson()` returns deterministic JSON for CI artifacts, issue reports, and
tooling workflows. In .NET, `ExportJsonLines()` lazily enumerates result lines;
in Rust, `export_result_json_lines(...)` returns `Result<Vec<String>,
ComparisonExportError>` and `write_result_json_lines(...)` streams to a writer.
.NET's path overloads write debug HTML and LLM context atomically. Rust's
`export_result_debug_html(...)` returns a `String`, while
`export_result_llm_context(...)` returns a fallible `String`; configured
`run_with_exports(...)` writes both artifacts. LLM context contains analysis
instructions, a concise summary, Markdown orientation, the full result JSON,
and row documents that can be chunked.

```csharp
var result = pipeline.History // Start from recorded windows.
    .Compare("Provider QA") // Name the comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select the target source.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select the comparison source.
    .Within(scope => scope.Window("DeviceOffline")) // Limit the scope to one window family.
    .Using(comparators => comparators.Overlap().Residual().Missing()) // Request visible agreement and divergence rows.
    .Run(); // Execute the comparison.

result.ExportDebugHtml("artifacts/provider-qa.html"); // Write a self-contained HTML graph for debugging.
result.ExportLlmContext("artifacts/provider-qa.llm.json"); // Write agent-readable context and full data.
```

```rust
let result = pipeline
    .history()
    .compare("Provider QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .overlap()
    .residual()
    .missing()
    .run();

std::fs::create_dir_all("artifacts")?;
std::fs::write(
    "artifacts/provider-qa.html",
    export_result_debug_html(&result),
)?;
std::fs::write(
    "artifacts/provider-qa.llm.json",
    export_result_llm_context(&result)?,
)?;
```

Plain `Run()`/`run()` execution does not write artifacts. Let configuration
decide whether to call an exporter after the immutable result exists; Rust also
offers `run_with_exports(...)` as an explicit execution-and-export boundary:

```csharp
var resultWithDebug = pipeline.History // Start from recorded windows.
    .Compare("Provider QA") // Name the comparison.
    .Target("provider-a", selector => selector.Source("provider-a")) // Select the target source.
    .Against("provider-b", selector => selector.Source("provider-b")) // Select the comparison source.
    .Within(scope => scope.Window("DeviceOffline")) // Limit the scope to one window family.
    .Using(comparators => comparators.Overlap().Residual()) // Request agreement and target-only rows.
    .Run();

resultWithDebug.ExportDebugHtml("artifacts/provider-qa.html");
resultWithDebug.ExportLlmContext("artifacts/provider-qa.llm.json");
```

```rust
let result_with_debug = pipeline
    .history()
    .compare("Provider QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .overlap()
    .residual()
    .run_with_exports(
        &ComparisonDebugHtmlOptions::to_file("artifacts/provider-qa.html"),
        &ComparisonLlmContextOptions::to_file("artifacts/provider-qa.llm.json"),
    )?;
```

Use debug HTML at workflow boundaries, test failure boundaries, notebook
boundaries, incident handoff boundaries, or support handoff boundaries. Avoid
writing it on every ingestion event. Use LLM context when an agent needs the
complete evidence graph, exact row data, and a compact orientation in one file.

Use `ErrorDiagnostics()`, `WarningDiagnostics()`,
`ProvisionalRowFinalities()`, and `FinalRowFinalities()` when a consumer only
needs a filtered view of the result metadata.

## Performance Guidance

Plans are cheap. Preparation enumerates recorded history and normalization work.
Comparator execution materializes result rows. Avoid exporting or explaining
results in ingestion hot paths; build those artifacts at workflow boundaries,
test failure boundaries, notebook boundaries, or support handoff points.

## Source Matrix

Use a source matrix when the same pairwise comparison needs to be read across
several sources:

```csharp
var matrix = pipeline.History.CompareSources( // Build a directional source matrix.
    "Provider matrix", // Name the matrix for reports.
    "DeviceOffline", // Compare one window family.
    ["provider-a", "provider-b", "provider-c"]); // Include these sources as rows and columns.
```

```rust
let matrix = pipeline.history().compare_sources(
    "Provider matrix",
    "DeviceOffline",
    &[
        "provider-a".to_owned(),
        "provider-b".to_owned(),
        "provider-c".to_owned(),
    ],
);
```

Cells are directional. The row source is the target and the column source is
the comparison source. In .NET, diagonal identity cells do not run comparators:
their row counts are zero and coverage is `1` when the source has windows.
Duplicate source identities are rejected. Rust computes diagonal metrics from
the source's closed processing-position activity; open windows and timestamp
windows are ignored. It also drops blank identities and keeps the first of each
duplicate identity. After those runtime-specific input rules, missing sources
are still emitted as explicit cells so reports do not silently drop an expected
provider.

## Runtime Boundary Segments

Segments are analytical boundary dimensions. When a segment value changes while
the active predicate remains true, Spanfold closes the current window and opens a
new one at the same processing position or event timestamp.

```csharp
var pipeline = EventPipeline // Start a Spanfold pipeline definition.
    .For<DeviceStateChanged>() // Configure the event type.
    .RecordWindows() // Store windows for comparison.
    .Window("DeviceOffline", window => window // Define the source window.
        .Key(update => update.DeviceId) // Track each device independently.
        .ActiveWhen(update => update.IsOffline) // Keep it open while the device is offline.
        .Segment("lifecycle", lifecycle => lifecycle // Split on lifecycle.
            .Value(update => update.Lifecycle) // Store the lifecycle value.
            .Child("stage", stage => stage // Nest stage under lifecycle.
                .Value(update => update.Stage))) // Split on stage changes.
        .Tag("fleet", update => update.FleetId)) // Attach metadata without splitting.
    .RollUp( // Preserve selected segments at zone level.
        "ZoneOffline", // Name the parent window.
        update => update.ZoneId, // Group devices by zone.
        children => children.ActiveCount > 0, // A zone is active when any device is offline.
        segments => segments // Project the child segment context.
            .Preserve("lifecycle") // Keep lifecycle.
            .Preserve("stage")) // Keep stage.
    .RollUp("RegionOffline", update => update.RegionId, children => children.ActiveCount > 0) // Preserve zone segments at region level.
    .Build(); // Build the pipeline.
```

```rust
let pipeline = for_events::<DeviceStateChanged>()
    .record_windows()
    .window_with_metadata(
        "DeviceOffline",
        |update| update.device_id.clone(),
        |update| update.is_offline,
        |update| {
            vec![
                WindowSegment::new("lifecycle", update.lifecycle.clone())
                    .expect("valid segment"),
                WindowSegment::new("stage", update.stage.clone())
                    .and_then(|segment| segment.with_parent("lifecycle"))
                    .expect("valid child segment"),
            ]
        },
        |update| {
            vec![WindowTag::new("fleet", update.fleet_id.clone())
                .expect("valid tag")]
        },
    )
    .roll_up_with_segment_projection(
        "ZoneOffline",
        |update| update.zone_id.clone(),
        |children| children.any_active(),
        |segments| segments.preserve("lifecycle").preserve("stage"),
    )
    .roll_up(
        "RegionOffline",
        |update| update.region_id.clone(),
        |children| children.any_active(),
    )
    .build()?;
```

Runtime partition, segment, and tag have different meanings:

- partition isolates runtime state
- segment splits active windows and participates in comparison scope
- tag describes a window without creating a boundary

Comparison scopes can filter by segment and tag:

```csharp
var escalatedIncidents = pipeline.History // Start from recorded windows.
    .Compare("Escalated incident coverage") // Name the comparison.
    .Target("source-a", selector => selector.Source("source-a")) // Select the target source.
    .Against("source-b", selector => selector.Source("source-b")) // Select the comparison source.
    .Within(scope => scope // Scope the comparison.
        .Window("DeviceOffline") // Use device-offline windows.
        .Segment("lifecycle", "Incident") // Require incident lifecycle.
        .Segment("stage", "Escalated") // Require escalated stage.
        .Tag("fleet", "critical")) // Require critical fleet metadata.
    .Using(comparators => comparators.Overlap().Residual()) // Emit agreement and target-only rows.
    .Run(); // Execute the comparison.
```

```rust
let escalated_incidents = pipeline
    .history()
    .compare("Escalated incident coverage")
    .target_selector(ComparisonSelector::for_source("source-a"))
    .against_selector(ComparisonSelector::for_source("source-b"))
    .scope(
        ComparisonScope::window("DeviceOffline")
            .segment("lifecycle", "Incident")
            .segment("stage", "Escalated")
            .tag("fleet", "critical"),
    )
    .overlap()
    .residual()
    .run();
```

Roll-up segment projection lets parent windows keep the dimensions that matter
at their level and ignore lower-level boundaries:

```csharp
.RollUp( // Roll device windows up to a zone.
    "ZoneOffline", // Name the parent window.
    update => update.ZoneId, // Select the parent key.
    children => children.ActiveCount > 0, // Keep the parent active while any child is active.
    segments => segments // Shape the parent segment context.
        .Preserve("lifecycle") // Keep lifecycle boundaries.
        .Drop("stage") // Do not split the zone when only stage changes.
        .Rename("lifecycle", "operatingMode") // Rename the parent dimension.
        .Transform("lifecycle", value => value?.ToString()?.ToUpperInvariant())) // Normalize values.
```

```rust
.roll_up_with_segment_projection(
    "ZoneOffline",
    |update| update.zone_id.clone(),
    |children| children.any_active(),
    |segments| {
        segments
            .preserve("lifecycle")
            .drop("stage")
            .rename("lifecycle", "operatingMode")
            .transform("lifecycle", |value| match value {
                PrimitiveValue::String(text) => PrimitiveValue::String(text.to_uppercase()),
                other => other.clone(),
            })
    },
)
```

## Cohorts

Use `AgainstCohort(...)` when the comparison side is a group. Cohort activity
rules collapse member sources into one derived comparison lane before
comparators run. This is different from summing pairwise residuals, which can
overcount.

```csharp
var result = pipeline.History // Start from recorded segmented windows.
    .Compare("Source A vs cohort") // Name the comparison.
    .Target("source-a", selector => selector.Source("source-a")) // Treat source A as target.
    .AgainstCohort("cohort", cohort => cohort // Define the cohort side.
        .Sources("source-b", "source-c") // Include member sources.
        .Activity(CohortActivity.Any())) // The cohort is active if either member is active.
    .Within(scope => scope.Window("DeviceOffline")) // Compare one window family.
    .Using(comparators => comparators.Residual()) // Emit target-only rows against the cohort.
    .Run(); // Execute the comparison.

var unmatchedLength = result.ResidualRows.TotalPositionLength(); // Sum target-only processing positions.

var evidence = result.CohortEvidence(); // Parse cohort evidence into typed records.
var uncovered = evidence.Where(row => !row.IsActive); // Find segments the cohort did not cover.
```

```rust
let result = pipeline
    .history()
    .compare("Source A vs cohort")
    .target_selector(ComparisonSelector::for_source("source-a"))
    .against_cohort(
        "cohort",
        ["source-b", "source-c"],
        CohortActivity::Any,
    )
    .scope(ComparisonScope::window("DeviceOffline"))
    .residual()
    .run();

let unmatched_length: i64 = result
    .rows
    .residual
    .iter()
    .map(|row| row.range.end - row.range.start)
    .sum();
let evidence = result.cohort_evidence();
let uncovered = evidence.iter().filter(|row| !row.is_active);
```

Available cohort activity rules:

- `CohortActivity.Any()` requires at least one active member.
- `CohortActivity.All()` requires every declared member.
- `CohortActivity.None()` requires no active members.
- `CohortActivity.AtLeast(n)` requires at least `n` active members.
- `CohortActivity.AtMost(n)` requires no more than `n` active members.
- `CohortActivity.Exactly(n)` requires exactly `n` active members.

Spanfold stores cohort evidence in result extension metadata. The evidence includes
the rule, required active count, actual active count, active member sources, and
whether the cohort lane was active. `CohortEvidence()` turns that metadata into
typed records for code, while `ExportJson()`, `ExportMarkdown()`, and
`ExportDebugHtml(...)` include the same evidence for humans.

## Hierarchy Explanation

Use hierarchy comparison to explain parent rollup activity from child
contribution windows:

```csharp
var hierarchy = pipeline.History.CompareHierarchy( // Explain parent activity from child windows.
    "Region explanation", // Name the hierarchy report.
    parentWindowName: "RegionImpacted", // Select the parent roll-up window.
    childWindowName: "DeviceOffline"); // Select the child contribution window.
```

```rust
let hierarchy = pipeline.history().compare_hierarchy(
    "Region explanation",
    "RegionImpacted",
    "DeviceOffline",
);
```

Rows are emitted as explained parent activity, unexplained parent duration, or
orphan child duration. Current lineage is inferred from matching source and
partition for the supplied parent and child window names. Parent and child keys
may differ because rollups often aggregate several child keys into one parent
key. .NET detects matching open parent or child windows and emits
`HierarchyOpenWindowsWithoutHorizon` while excluding their unbounded duration.
Rust's current hierarchy helper reads `closed_windows()` only and matches parent
and child evidence by source, partition, compatible temporal axis, and timestamp
clock. Matching open windows do not contribute rows and do not produce that
diagnostic.

## Redacted agent context

When a result crosses a support, CI, or external-agent boundary, prefer the
value-redacted context export. It includes row counts, stable row IDs, finality,
and diagnostic codes, but excludes plan names, keys, sources, partitions, tags,
segments, and diagnostic text:

```csharp
var safeContext = result.ExportRedactedAgentContext();
```

Rust does not currently expose a redacted agent-context exporter. Its JSON,
Markdown, debug HTML, and LLM-context exports contain evidence-bearing values;
apply an application-owned data policy before sharing them outside a trusted
boundary. Do not substitute `export_result_llm_context(...)` for redaction.

The regular JSON, Markdown, HTML, and LLM exports are evidence-bearing artifacts
and must be treated as sensitive data unless the caller has already applied an
appropriate data policy.
