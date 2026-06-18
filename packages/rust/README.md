# Spanfold Rust

Rust 1.95.0 / Rust 2024 implementation of Spanfold's high-throughput library
and CLI surface.

The Rust package now covers the main comparison contract: typed temporal
primitives, window histories, fixture parsing, staged preparation/alignment,
core and advanced comparators, cohort/source-matrix/hierarchy analytics,
deterministic exports, audit bundles, liveness helpers, and testing utilities.

Private implementation planning specs live under `packages/rust/specs/`.

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
    .build();

pipeline.ingest(
    DeviceStatus { device_id: "device-17".into(), is_online: false },
    Some("provider-a"),
    None,
);

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

## Commands

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
| Core temporal model and window records | Conformance-passing |
| Direct history queries, snapshots, grouping summaries | Implemented |
| Window annotations and known-at annotation filtering | Implemented |
| Fixture parsing and validation | Conformance-passing |
| Comparison preparation and deterministic alignment | Conformance-passing |
| Overlap, residual, missing, coverage, gap, symmetric difference | Conformance-passing |
| Containment, lead/lag, as-of | Conformance-passing |
| Known-at filtering, live horizons, row finality, changelog | Conformance-passing |
| Cohorts, source matrix, hierarchy, nested roll-ups | Implemented |
| JSON, JSON Lines, Markdown, debug HTML, LLM context exports and configured run exports | Conformance-passing |
| Fixture CLI, window JSONL audit CLI, audit bundles | Conformance-passing |
| Event JSONL/CSV import and audit CLI | Implemented |
| Liveness helpers | Implemented |
| Testing helpers, snapshot normalization, virtual clocks | Implemented |
| Pipeline emissions, batch ingestion, event-time recording, configured metadata, callbacks, boundary reasons, roll-up segment projection | Implemented |
| Criterion/throughput benchmark suite | Implemented |

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
