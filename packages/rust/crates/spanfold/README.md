# Spanfold

`spanfold` is a Rust library for recording temporal state windows and comparing
their histories. It provides typed recording pipelines, deterministic alignment,
temporal comparators, audit exports, and helpers for live and known-at analysis.

The API is pre-1.0 and may change between minor releases.

## Install

```bash
cargo add spanfold
```

## Example

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
    DeviceStatus {
        device_id: "device-17".into(),
        is_online: false,
    },
    Some("provider-a"),
    None,
)?;

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

# Ok::<(), spanfold::SpanfoldError>(())
```

## Capabilities

- Event-driven state-window recording with processing-time and event-time axes
- Deterministic normalization and alignment of temporal histories
- Overlap, residual, missing, coverage, containment, lead/lag, and as-of analysis
- Cohort, source-matrix, hierarchy, and nested roll-up analytics
- JSON, JSON Lines, Markdown, debug HTML, LLM-context, and audit-bundle exports
- Known-at filtering, live horizons, finality, liveness, and testing helpers

For command-line workflows, install
[`spanfold-cli`](https://crates.io/crates/spanfold-cli). The installed command is
named `spanfold`.

See the [repository](https://github.com/Scottlr/Spanfold) and
[Rust changelog](https://github.com/Scottlr/Spanfold/blob/main/packages/rust/CHANGELOG.md)
for source, examples, and release notes.
