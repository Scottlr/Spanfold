# Spanfold Rust

Rust 1.95.0 / Rust 2024 preview of Spanfold's library and CLI surface. The API
is intentionally pre-1.0 and may change between minor releases.

The Rust package now covers the main comparison contract: typed temporal
primitives, window histories, fixture parsing, staged preparation/alignment,
core and advanced comparators, cohort/source-matrix/hierarchy analytics,
Episode formation and relation analysis, deterministic exports, audit bundles,
liveness helpers, and testing utilities.

## Install

```bash
cargo add spanfold@0.1.1
cargo install spanfold-cli --version 0.1.1
```

The library crate is `spanfold`. The installed CLI command is also `spanfold`.
See the [Rust changelog](https://github.com/Scottlr/Spanfold/blob/main/packages/rust/CHANGELOG.md)
for release details.

## Rust API

```rust
use spanfold::for_events;

#[derive(Clone)]
struct DeviceStatus {
    device_id: String,
    is_online: bool,
}

let mut pipeline = for_events::<DeviceStatus>()
    .record_windows()
    .track_window(
        "DeviceOffline",
        |event| event.device_id.clone(),
        |event| !event.is_online,
    )
    .build()?;

pipeline.ingest(
    DeviceStatus { device_id: "device-17".into(), is_online: false },
    Some("provider-a"),
    None,
).expect("ingest event");

let result = pipeline
    .history()
    .compare("Provider QA")
    .target_source("provider-a")
    .against_source("provider-b")
    .scope_window("DeviceOffline")
    .overlap()
    .residual()
    .missing()
    .coverage()
    .run();
```

### Stabilize noisy window transitions

Configure consecutive entry and exit confirmation on the typed source-window builder:

```rust
let mut pipeline = for_events::<DeviceStatus>()
    .record_windows()
    .window(
        "DeviceOffline",
        |event| event.device_id.clone(),
        |event| !event.is_online,
    )
    .stabilize(|event| event.is_online, 2, 3)
    .build()?;
```

Counts are independent per window key, source, and partition, and a false
observation resets the applicable count. The confirming event supplies the
transition boundary and opening metadata. Pending exits preserve the committed
window and roll-up membership. Without `stabilize`, transitions remain
immediate and exit when the active predicate becomes false.

## Rust Selectors

Rust supports selector-backed comparison plans as first-class API. Selectors can
be serializable descriptors for portable exports or runtime-only predicates for
local execution.

```rust
use spanfold::ComparisonSelector;

let target = ComparisonSelector::for_source("provider-a")
    .and(ComparisonSelector::for_key("device-17"));
let against = ComparisonSelector::for_source("provider-b");

let result = history
    .compare("Provider QA")
    .target_selector(target)
    .against_selector(against)
    .scope_window("DeviceOffline")
    .overlap()
    .run();
```

## Ordered Sequences

Match literal named window families in onset order within one exact
key/source/partition lane. Complete matches consume their evidence and live
matching preserves provisional finality.
This API landed after the published crates.io `0.1.1` release; use current
repository source until a later crate release explicitly includes it.

```rust
let journeys = history
    .match_sequence("incident journey")
    .step("Warning")
    .then("Offline")
    .then("Recovered")
    .with_maximum_gap(5)
    .run()?;
```

The current repository source surface, including exact lane identity,
candidate selection, gap arithmetic, lineage, live finality, and the
published-crate boundary, is covered in the
[ordered cross-window sequences guide](https://scottlr.github.io/Spanfold/ordered-sequences.html).

## Repository development commands

```bash
cargo test --all
cargo run -p spanfold-cli -- --help
cargo run -p spanfold-cli -- compare ../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/basic-overlap.json --format json
cargo run -p spanfold-cli -- import-events events.jsonl --map import-map.json --out windows.jsonl
cargo run -p spanfold-cli -- audit-events events.jsonl --map import-map.json --target provider-a --against provider-b --out artifacts/audit
cargo run -p spanfold-cli -- import-events events.csv --map csv-import-map.json --out windows.jsonl
cargo bench -p spanfold --bench spanfold_benchmarks
```

## Parity Status

| Area | Status |
| --- | --- |
| Core temporal model and window records | Implemented; cross-language conformance gate pending |
| Direct history queries, snapshots, grouping summaries | Implemented |
| Window annotations and known-at annotation filtering | Implemented |
| Fixture parsing and validation | Implemented; cross-language conformance gate pending |
| Comparison preparation and deterministic alignment | Implemented; cross-language conformance gate pending |
| Overlap, residual, missing, coverage, gap, symmetric difference | Implemented |
| Containment, lead/lag, as-of | Implemented |
| Known-at filtering, live horizons, row finality, changelog | Implemented |
| Cohorts, source matrix, hierarchy, nested roll-ups | Implemented |
| Episode formation, relation graphs, summaries, reference scorecards | Implemented |
| Ordered cross-window sequences with live finality | Implemented |
| JSON, JSON Lines, Markdown, debug HTML, LLM context exports and configured run exports | Implemented; conformance gate pending |
| Fixture CLI, window JSONL audit CLI, audit bundles | Implemented |
| Event JSONL/CSV import and audit CLI | Implemented |
| Liveness helpers | Implemented |
| Testing helpers, snapshot normalization, virtual clocks | Implemented |
| Pipeline emissions, batch ingestion, event-time recording, configured metadata, callbacks, boundary reasons, roll-up segment projection | Implemented |
| Criterion/throughput benchmark suite | Implemented; published workload baselines pending |

## Event Import Map

`import-events` and `audit-events` consume JSON Lines or header-row CSV events
with a declarative map. Selectors are field paths/column names and predicates
are fixed comparisons, so maps do not execute user code.

```json
{
  "input": "jsonl",
  "source": "source",
  "position": "position",
  "windows": [
    {
      "name": "DeviceOffline",
      "key": "deviceId",
      "active": { "field": "status", "equals": "offline" },
      "segments": [{ "name": "region", "field": "region" }],
      "tags": [{ "name": "severity", "field": "severity" }]
    }
  ]
}
```

## Runtime Boundary

Spanfold can ingest events, keep open windows, run live comparisons at explicit
horizons, and mark provisional evidence. It is not a stream processor, hosted
observability backend, durable queue, or scheduler; callers own IO, timers, and
persistence.
