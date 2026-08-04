# Spanfold

`spanfold` is a Rust library for recording temporal state windows and comparing
their histories. It provides typed recording pipelines, deterministic alignment,
temporal comparators, Episode analysis, audit exports, and helpers for live and
known-at analysis.

The API is pre-1.0 and may change between minor releases.

## Install

```bash
cargo add spanfold@0.1.1
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

## Authoritative row identity and finality

Comparison results own the association between each typed row and its row ID,
finality, reason, version, and supersession metadata. Consume that association
through the borrowed family views rather than inventing positional IDs or
recomputing hashes:

```rust
use spanfold::{ComparisonResult, ComparisonRowMetadataError};

fn persist_overlaps(result: &ComparisonResult) -> Result<(), ComparisonRowMetadataError> {
    for entry in result.overlap_rows_with_finality()? {
        println!(
            "{} {:?} {}..{}",
            entry.metadata.row_id,
            entry.metadata.finality,
            entry.row.range.start,
            entry.row.range.end,
        );
    }
    Ok(())
}
```

Equivalent views are available for residual, missing, coverage, gap,
symmetric-difference, containment, lead/lag, and as-of rows. The IDs are opaque
identifiers assigned by the producing Rust result; persist them exactly rather
than relying on the current private hashing scheme or assuming .NET parity.

`result.rows` is the canonical grouped row collection. Existing family fields
remain zero-copy compatibility views in 0.1.1. A `CoverageRow` describes one
aligned target segment and is normally wholly covered or uncovered; use
`result.coverage_summaries` for grouped aggregate coverage ratios.

## Capabilities

- Event-driven state-window recording with processing-time and event-time axes
- Deterministic normalization and alignment of temporal histories
- Overlap, residual, missing, coverage, containment, lead/lag, and as-of analysis
- Cohort, source-matrix, hierarchy, and nested roll-up analytics
- Episode formation, relation graphs, neutral summaries, and reference scorecards
- JSON, JSON Lines, Markdown, debug HTML, LLM-context, and audit-bundle exports
- Known-at filtering, live horizons, finality, liveness, and testing helpers
- Borrowed typed row/finality views with authoritative opaque row IDs

For command-line workflows, install
[`spanfold-cli`](https://crates.io/crates/spanfold-cli). The installed command is
named `spanfold`.

See the [repository](https://github.com/Scottlr/Spanfold) and
[Rust changelog](https://github.com/Scottlr/Spanfold/blob/main/packages/rust/CHANGELOG.md)
for source, examples, and release notes.
