# Live Finality And Changelog

Spanfold separates historical comparison from live comparison.

Historical comparison uses `Run()` and rejects open windows by default. Live
comparison uses `RunLive(horizon)` and clips open windows to an explicit
evaluation horizon.

## Finality

Rows emitted by `RunLive(...)` carry finality metadata:

- `Final` means the row only depends on closed source windows.
- `Provisional` means the row depends on at least one open source window clipped
  to the evaluation horizon.

Use helpers when a report or agent only needs one view:

```csharp
var live = history.Compare("Live QA") // Start a comparison over recorded history.
    .Target("provider-a", selector => selector.Source("provider-a")) // Treat provider A as the baseline.
    .Against("provider-b", selector => selector.Source("provider-b")) // Compare provider B against it.
    .Within(scope => scope.Window("DeviceOffline")) // Limit analysis to one window family.
    .Using(comparators => comparators.Residual()) // Emit target-only rows.
    .RunLive(TemporalPoint.ForPosition(100)); // Clip open windows to a deterministic horizon.

if (live.HasProvisionalRows()) // Check whether any rows depend on open windows.
{
    foreach (var finality in live.ProvisionalRowFinalities()) // Iterate only provisional metadata.
    {
        Console.WriteLine($"{finality.RowId}: {finality.Reason}"); // Explain why each row can change.
    }
}
```

```rust
let live = history
    .compare("Live QA")
    .target_selector(ComparisonSelector::for_source("provider-a"))
    .against_selector(ComparisonSelector::for_source("provider-b"))
    .scope(ComparisonScope::window("DeviceOffline"))
    .residual()
    .run_live(TemporalPoint::position(100));

if live.has_provisional_rows() {
    for finality in live.provisional_row_finalities() {
        println!("{}: {}", finality.row_id, finality.reason);
    }
}
```

## Changelog

In .NET, `ComparisonChangelog.Create(...)` accepts the previous and current
`RowFinalities`. In Rust, `create_changelog(...)` accepts slices of the previous
and current `row_finalities`. Both compare row-finality metadata between
snapshots and emit deterministic entries for:

- new row metadata (`Added` in both runtimes)
- finality or reason changes (`Revised` in .NET and `Updated` in Rust)
- removed metadata (`Retracted` in both runtimes)

`ComparisonChangelog.Replay(...)` in .NET and `replay_changelog(...)` in Rust
rebuild the current row-finality view from a prior view plus its changelog
entries.

```csharp
var entries = ComparisonChangelog.Create(
    previous.RowFinalities,
    current.RowFinalities);
var replayed = ComparisonChangelog.Replay(previous.RowFinalities, entries);
```

```rust
let entries = create_changelog(
    &previous.row_finalities,
    &current.row_finalities,
);
let replayed = replay_changelog(&previous.row_finalities, &entries);
```

This is useful for dashboards, agents, notebooks, and audit logs that need to
explain why a live answer changed.

## Late event corrections

The .NET `BoundedWatermarkTracker` can sit before an application-owned pipeline
when a source provides trustworthy per-lane event-time progress. A corrected
decision identifies both the replacement revision and the accepted revision to
retract. After applying that source correction, produce a new live snapshot and
use `ComparisonChangelog.Create(...)` over the two results' `RowFinalities` to
derive row-level finality changes. The watermark tracker does not mutate
comparison rows directly.

Rust supports the resulting live snapshots, row finality, changelog creation,
and replay, but it does not currently expose `BoundedWatermarkTracker` or an
equivalent late-event acceptance/correction helper. A Rust application must own
that upstream policy and feed the corrected history into a new live comparison;
the changelog API only explains changes between the resulting row-finality
views.

See [bounded watermarks and late correction](bounded-watermarks.md) for the
acceptance boundaries, correction horizon, and objective limitations.

## Fixture Live Horizon

CLI fixture plans can include `liveHorizonPosition`:

```json
{
  "plan": {
    "name": "Live Provider QA",
    "targetSource": "provider-a",
    "againstSources": [ "provider-b" ],
    "scopeWindow": "DeviceOffline",
    "comparators": [ "residual" ],
    "strict": false,
    "liveHorizonPosition": 100
  }
}
```

When this value is present, the fixture runner executes the comparison as a live
snapshot. Windows with `"endPosition": null` are treated as open windows.
