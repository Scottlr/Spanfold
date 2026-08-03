# Performance Notes

Spanfold keeps ingestion, comparison, and export costs separate.

## Hot Paths

- Event ingestion owns mutable runtime state and should avoid export, explain,
  and snapshot work.
- Window recording appends window records during ingestion when
  `RecordWindows()` is enabled.
- Comparison preparation materializes selected and normalized records.
- Alignment builds deterministic temporal segments for comparator execution.
- Export and explain are workflow-boundary operations, not ingestion hot paths.

## Benchmarked Areas

The benchmark project covers:

- ingestion with window recording
- ingestion with boundary segments, non-boundary tags, projected roll-ups, and
  window recording
- preparation
- alignment
- overlap, residual, missing, coverage, containment, lead-lag, and multi
  comparator execution
- live residual execution with a horizon
- segment-filtered residual execution
- `Any()` and `AtLeast(n)` cohort residual execution
- live segment/cohort residual execution
- JSON and Markdown export overhead
- deterministic Episode formation across geometric history sizes
- sparse Episode relation-graph construction across geometric episode counts
- reference-scorecard interpretation from an already materialized comparison

The comparison benchmark also includes `DenseSingleScope`, a checked-in shape
with one device key and alternating states across both sources. This keeps the
alignment workload dense enough to expose per-scope scaling regressions.

Run the .NET suite with:

```bash
dotnet run -c Release --project benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj # Run the full benchmark suite.
```

For focused work, filter by class or method:

```bash
dotnet run -c Release --project benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter "*ComparisonBenchmarks.RunLiveResidual*" # Run one benchmark target.
dotnet run -c Release --project benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter "*SegmentCohortBenchmarks*" # Run segment/cohort benchmarks.
```

## Episode Baseline

The Episode workloads use the same deterministic processing-position shapes in
.NET and Rust:

- Formation uses 128, 1,024, and 8,192 closed windows. Each episode key has
  eight four-position fragments separated by a six-position gap; a stitch
  tolerance of six therefore produces 16, 128, and 1,024 episodes. The timed
  public `Run`/`run` journey includes plan materialization and validation,
  record ordering and normalization, grouping, fragment sorting, stitching,
  episode materialization, and set-summary materialization. Histories and
  configured builders are created in setup, and their immutable plans are
  built once there to validate the input before measurement. Neither public API
  currently accepts an already-built Episode plan for execution.
- Sparse relation construction uses 64, 256, and 1,024 single-fragment
  episodes per side, with a unique matching key on each side. Target episode
  `i` spans `[20i, 20i + 4]`. Three of every four detection episodes span
  `[20i + 1, 20i + 5]`; the fourth spans `[20i + 10, 20i + 14]`. With zero
  relation tolerance this yields a fixed 75% matched-key rate, while edge
  density across all possible cross-side pairs falls from about 1.17% to
  0.073%. The input therefore never makes every pair overlap. The timed public
  comparison includes plan materialization and validation, formation of both
  episode sets, lineage checks, compatibility and fragment-relation checks,
  connected-component construction, relation metrics, and comparison-summary
  materialization.
- `InterpretMaterializedReferenceScorecard` / `interpret_materialized_reference_scorecard`
  starts from the 1,024-per-side comparison materialized in setup. It
  intentionally measures only the cheap directional interpretation of the
  precomputed comparison summary and construction of the scorecard; it does not
  repeat formation or graph construction.

The baseline was measured on 2026-08-03 on an Apple M4 Pro (12 physical and
logical cores), macOS 26.5.2 (25F84). .NET used SDK 10.0.102, runtime 10.0.2,
BenchmarkDotNet 0.15.2, Arm64 RyuJIT, concurrent workstation GC, and the
`Short` job (three warmups and three measured iterations). Rust used rustc and
Cargo 1.95.0 for `aarch64-apple-darwin`, Criterion 0.8.2, and Criterion's
default 3-second warmup, 100-sample, approximately 5-second measurement per
case.

From the repository root, the exact .NET commands were:

```bash
dotnet build packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -c Release -p:RestoreSources=https://api.nuget.org/v3/index.json
RestoreSources=https://api.nuget.org/v3/index.json dotnet run -c Release --no-build --project packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter '*Episode*' --job Short --artifacts /tmp/spanfold-p1-bdn --noOverwrite > /tmp/spanfold-p1-bdn-3.log 2>&1
```

The `RestoreSources` override was needed only to avoid unrelated additional
NuGet sources in the measuring machine's user configuration; it did not alter
the repository or benchmark job. From `packages/rust`, the exact Rust commands
were:

```bash
cargo build --release --bench spanfold_benchmarks
cargo bench --bench spanfold_benchmarks -- episode > /tmp/spanfold-p1-criterion.log 2>&1
```

### .NET baseline

| Operation | Scale | Mean | Allocated |
| --- | ---: | ---: | ---: |
| Form episodes | 128 windows | 62.61 us | 346.93 KB |
| Form episodes | 1,024 windows | 751.00 us | 3,395.54 KB |
| Form episodes | 8,192 windows | 17.273 ms | 31,862.94 KB |
| Build sparse relation graph | 64 episodes/side | 234.7 us | 872.88 KB |
| Build sparse relation graph | 256 episodes/side | 1.604 ms | 3,912.86 KB |
| Build sparse relation graph | 1,024 episodes/side | 17.722 ms | 17,650.52 KB |
| Interpret materialized reference scorecard | 1,024 episodes/side | 7.087 ns | 88 B |

### Rust baseline

Criterion reports a confidence interval; the middle estimate is shown here.

| Operation | Scale | Time |
| --- | ---: | ---: |
| Form episodes | 128 windows | 67.680 us |
| Form episodes | 1,024 windows | 581.83 us |
| Form episodes | 8,192 windows | 5.1662 ms |
| Build sparse relation graph | 64 episodes/side | 213.83 us |
| Build sparse relation graph | 256 episodes/side | 1.0562 ms |
| Build sparse relation graph | 1,024 episodes/side | 7.8685 ms |
| Interpret materialized reference scorecard | 1,024 episodes/side | 4.3632 ns |

These absolute .NET and Rust numbers are not a language comparison. Although
the histories are conceptually aligned and were measured on the same machine,
the frameworks use different warmup, sampling, runtime, allocation, and code
generation models. Compare scale trends within one runtime or compare later
measurements from the same runtime and command.

The sparse relation workload is the evidence-backed P2 indexing candidate. A
4x increase from 256 to 1,024 episodes per side increased relation time by
11.1x in .NET and 7.4x in Rust even though graph edge density decreased. The
materialized scorecard path is negligible and should not be optimized. P2
should first index relation candidates by existing compatibility dimensions
before fragment checks, then use these checked-in cases as the before/after
gate; formation-specific work should remain separate unless its own scale
trend is the target.

## Current Optimization Work

The first benchmark-backed optimization target was comparison alignment. The
original alignment path used LINQ grouping, ordering, and per-group array
materialization before segment construction. The current path builds one
sortable array of normalized windows, sorts it deterministically, and processes
contiguous scope groups in place.

## Optimization Priorities

1. Keep selector and normalization costs explicit during preparation.
2. Avoid rebuilding dictionaries or arrays inside comparator row loops.
3. Keep row materialization deterministic, even when optimizing grouping.
4. Treat export allocation as acceptable at reporting boundaries, but keep it out
   of ingestion paths.
5. Prefer adding benchmark coverage before optimizing a comparator.
