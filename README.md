<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/brand/spanfold-logo-readme-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/assets/brand/spanfold-logo-readme-light.svg">
    <img src="docs/assets/brand/spanfold-logo-readme-light.svg" alt="Spanfold" width="280">
  </picture>
</p>

# Spanfold

**Temporal interval comparison for application state.**

When a predicate changes — a service goes down, a threshold is crossed, a status flips — Spanfold records that period as a window. When you have multiple sources reporting the same condition, Spanfold tells you exactly where they agreed, diverged, lagged, or left gaps.

## Install the preview

Install the latest published .NET preview from NuGet.org or the current Rust
crate from crates.io:

```bash
dotnet add package Spanfold --version 0.1.0-preview.2
cargo add spanfold@0.1.1
```

The repository's .NET source is versioned `0.2.0-preview.1`, but that version
has not been published to NuGet.org.

## Quick Start

```csharp
using Spanfold;

// 1. Define: what condition are you tracking, and for which key?
var pipeline = Spanfold.Spanfold
    .For<MonitorEvent>()
    .RecordWindows()
    .TrackWindow("Outage",
        key:       e => e.ServiceId,
        isActive:  e => e.Status == "down");

// 2. Ingest events from each source
pipeline.Ingest(new MonitorEvent("orders", "down"), source: "provider-a");
pipeline.Ingest(new MonitorEvent("orders", "up"), source: "provider-a");
pipeline.Ingest(new MonitorEvent("orders", "down"), source: "provider-b");
pipeline.Ingest(new MonitorEvent("orders", "up"), source: "provider-b");

// 3. Compare: who saw what, when, and for how long?
var result = pipeline.History
    .Compare("Outage audit")
    .Target("provider-a", s => s.Source("provider-a"))
    .Against("provider-b", s => s.Source("provider-b"))
    .Within(scope => scope.Window("Outage"))
    .Using(c => c.Overlap().Residual().Missing().Coverage())
    .Run();

// result.OverlapRows   — periods both sources agreed on
// result.ResidualRows  — periods A reported that B missed
// result.MissingRows   — periods B reported that A missed

public sealed record MonitorEvent(string ServiceId, string Status);
```

---

## What It Solves

You have multiple systems — monitoring providers, pipeline stages, model versions, detectors — all reporting observations about the same thing. You want to know:

- **When** did each source think this condition was true, and for how long?
- **Where** did sources agree, diverge, lag, or leave gaps?
- **What** was knowable at a specific point in time, without leaking future data?

A latest-value store tells you the current state. An event log tells you what happened. Spanfold tells you **when each source believed what, and where those beliefs differed**.

---

## Why Not SQL Interval Joins or Ad Hoc Code?

Interval comparison looks simple until you account for partial overlaps, gaps within coverage, multiple windows on the same key, lead/lag timing, live windows, and known-at filtering. Each question adds another query or another special case.

**SQL interval join — overlap only:**

```sql
SELECT a.service_id,
       GREATEST(a.start, b.start) AS overlap_start,
       LEAST(a.end,   b.end)   AS overlap_end
FROM   outage_windows_a a
JOIN   outage_windows_b b
  ON   a.service_id = b.service_id
 AND   a.start < b.end
 AND   a.end   > b.start
-- Residual (A-only) needs a NOT EXISTS query.
-- Gap detection, lead/lag, coverage % each need another query.
-- Known-at filtering (no future leakage) needs timestamp join bookkeeping.
-- Live/provisional windows need finality state tracked separately.
```

**Spanfold — all of the above in one comparison plan:**

```csharp
var result = pipeline.History
    .Compare("Outage audit")
    .Target("provider-a", s => s.Source("provider-a"))
    .Against("provider-b", s => s.Source("provider-b"))
    .Within(scope => scope.Window("Outage"))
    .Using(c => c
        .Overlap()   // both agreed
        .Residual()  // A saw, B missed
        .Missing()   // B saw, A missed
        .Gap()       // empty periods inside observed scope
        .LeadLag(LeadLagTransition.Start, TemporalAxis.ProcessingPosition, 1) // transition timing drift between sources
        .Coverage()) // magnitude and coverage percentage
    .Run();
// Known-at filtering and live horizons are built into the comparison model.
```

---

## Active Predicates Express More Than Boolean Fields

A window opens when its active predicate changes from `false` to `true` and
closes when it changes back to `false`. The predicate itself returns a boolean,
but it can express conditions over any event data:

| Condition type | Example |
|----------------|---------|
| Boolean field | `isUp == true` |
| Threshold | `cpuPercent > 80` |
| Enum / status | `status == "degraded"` |
| Numeric range | `latencyMs >= 500 && latencyMs < 2000` |

---

## Comparator Families

| Comparator     | What it measures                                       |
|----------------|--------------------------------------------------------|
| Overlap        | Duration where both sides agreed                       |
| Residual       | Target-only duration (what the comparison side missed) |
| Missing        | Comparison-only duration (what the target missed)      |
| Coverage       | Magnitude and coverage percentage                      |
| Gap            | Empty spaces inside an observed scope                  |
| Symmetric diff | Disagreement in both directions                        |
| Containment    | Whether one period stays inside another                |
| Lead / Lag     | Transition timing drift between sources                |
| As-of          | Point-in-time lookup without future leakage            |

→ [Comparator reference](docs/comparator-reference.md) · [Comparison guide](docs/comparison-guide.md)

---

## Use Cases

### Monitoring provider outage comparison

You have two or more monitoring providers watching the same service. When an outage occurs, each provider may report it at a slightly different time, recover at a different time, or miss it entirely. Spanfold records each provider's outage windows and emits structured rows showing exactly where they agreed, where one reported a period the other didn't, and how large each discrepancy was.

→ [Provider outage comparison](docs/use-cases.html#monitoring)

### Pipeline stage divergence

A processing pipeline passes events through multiple stages — ingestion, enrichment, classification, alerting. When a condition appears at one stage but not another, or arrives late, or disappears before the final stage, Spanfold records a window at each stage and compares them to show where state diverged, lagged, or dropped.

→ [Pipeline stage divergence](docs/use-cases.html#data-pipelines)

### Backtesting without future leakage

Auditing a past decision means using only what was knowable at the time — not data that arrived later. Spanfold's known-at filtering separates when a state was observed from when it was available to the system, so backtests, replays, and decision-point audits do not accidentally include future observations even when replaying historical data.

→ [No-future-leakage backtesting](docs/use-cases.html#decision-audit)

---

## Core Concepts

**Windows** — a half-open period where a predicate held for a key. Can be closed, still open, or clipped to a live horizon.

**Sources and lanes** — where an observation came from: providers, monitors, gateways, pipeline stages, model versions, or any other reporting lane.

**Segments and tags** — segments split a window when analytical context changes; tags attach metadata without splitting.

**Comparisons** — a staged plan: target side, comparison side, scope, normalization, and comparator families. Produces structured temporal evidence.

**Episodes (.NET preview and Rust)** — stitch nearby windows on each side into occurrences, then classify the complete relation graph as one-to-one, split, merge, complex, or unmatched. Fragments remain the active evidence; episode envelopes describe elapsed occurrence extent.

**Ordered sequences (.NET preview and Rust)** — match literal named window-family steps within one key/source/partition lane, with deterministic non-reuse, optional processing-position gaps, and provisional live evidence.

**Known-at safety** — separates when a state happened from when it was observable. Prevents future leakage in backtests and replays.

**Live horizons** — an explicit cutoff for evaluating still-open windows. Preserves provisional row metadata so live and final rows are distinguishable.

---

## Why Not Just X?

**Latest-state tracking** — answers what is true now. Spanfold answers when it was true, who saw it, and whether another lane missed it.

**Event sourcing** — stores durable facts and rebuilds state. Spanfold analyzes the periods where that state held after those facts have been interpreted.

**Stream processors** — handle online computation, routing, enrichment, and aggregation. Spanfold is narrower: it records interpreted state windows and compares their temporal evidence.

**Observability and metrics tools** — aggregate time into counters, histograms, and dashboards. Spanfold keeps individual windows and emits comparison rows with full temporal structure.

**A database with interval storage** — can persist windows, but will not provide staged comparison plans, normalization, live finality, known-at filtering, diagnostics, or deterministic exports.

---

## .NET Package

NuGet.org currently publishes the core C# API and testing helpers at
`0.1.0-preview.2`:

```bash
dotnet add package Spanfold --version 0.1.0-preview.2
dotnet add package Spanfold.Testing --version 0.1.0-preview.2
```

The `Spanfold.Artifacts` package and the current `Spanfold.Cli`
`0.2.0-preview.1` tool are not published. Use those projects from a repository
checkout. After creating a fixture described by the
[fixture schema](docs/fixture-schema.md), run the documented comparison command
through the source project:

```bash
dotnet run --project packages/dotnet/src/Spanfold.Cli/Spanfold.Cli.csproj -- compare fixture.json --format json
```

→ [.NET package README](packages/dotnet/README.md)

Episode analysis can sit on top of the same recorded history when analysts care
about occurrences as well as exact coverage:

```csharp
using Spanfold.Episodes;

var episodes = pipeline.History
    .CompareEpisodes("Provider QA")
    .Target("reference", selector => selector.Source("provider-a"))
    .Against("detector", selector => selector.Source("provider-b"))
    .Within(scope => scope.Window("DeviceOffline"))
    .StitchGapsUpTo(2L)
    .RelateWithin(1L)
    .Run();
```

→ [Episode analysis guide](docs/episode-analysis.html) · [C# package workflow](packages/dotnet/README.md#compare-occurrences-as-episodes)

The .NET and Rust CLIs can also execute the same versioned Episode analysis
document over flat window JSON Lines and emit aligned deterministic JSON or
Markdown. Runtime-specific Episode IDs are not part of that portable contract.

→ [Portable Episode analysis guide](docs/episode-analysis.html#portable-document) · [schema contract](features/episodes/portable-analysis.md)

## Rust Package

A Rust 2024 library that supports the core comparison and Episode contracts,
plus a CLI for portable comparison and audit workflows. The library provides
idiomatic builder methods, typed temporal records, selector-backed comparison
plans, deterministic exports, testing helpers, and event-driven window
recording.

```bash
cargo add spanfold@0.1.1
cargo install spanfold-cli --version 0.1.1
```

→ [Rust package README](packages/rust/README.md)

---

## Repository Layout

```text
packages/
  dotnet/
    src/
    tests/
    samples/
    benchmarks/
    Spanfold.slnx
  rust/
    crates/
    Cargo.toml
docs/
  index.html
  assets/
```

## Working With Packages

Run the .NET reference tests:

```bash
dotnet test packages/dotnet/Spanfold.slnx
```

Run the Rust port tests and lints:

```bash
cd packages/rust
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
```

## Documentation

- [Public site](docs/index.html)
- [Get started](docs/get-started.html)
- [Get started with C#](docs/get-started-csharp.html)
- [Get started with Rust](docs/get-started-rust.html)
- [C# and Rust portability guide](docs/cross-language-guide.html)
- [Episode analysis guide](docs/episode-analysis.html)
- [Import existing history guide](docs/import-existing-history.html)
- [Live stream operations guide](docs/live-stream-operations.html)
- [Use cases](docs/use-cases.html)
- [Visual Auditing](docs/visualiser.html)
- [API reference](docs/api.html)
- [C# API reference](docs/api-csharp.html)
- [Rust API reference](docs/api-rust.html)
- [Comparator reference](docs/comparator-reference.md)
- [Comparison guide](docs/comparison-guide.md)
- [Machine-readable documentation index](docs/llms.txt)
- [Documentation site contributor guide](docs/README.md)
