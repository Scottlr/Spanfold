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
- dense single-scope lead/lag and as-of transition matching across geometric
  transition counts

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

### P2 relation candidate indexing

P2 replaced the all-pairs target-against compatibility scan with a private
against-side candidate index. The index uses the existing compatibility
identity: window family, logical key, partition, temporal axis, and timestamp
clock. .NET delegates key equality and hashing to the configured per-window
key comparer; Rust keys are exact strings. Candidate lists retain against-side
episode order, and actual fragment overlap/proximity remains the final edge
test.

The after measurement used the same machine and runtime configuration as the
baseline above. Only the checked-in sparse relation cases were rerun. From the
repository root, the exact .NET commands were:

```bash
dotnet build packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -c Release -p:RestoreSources=https://api.nuget.org/v3/index.json
RestoreSources=https://api.nuget.org/v3/index.json dotnet run -c Release --no-build --project packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter '*EpisodeRelationBenchmarks.BuildSparseRelationGraph*' --job Short --artifacts /tmp/spanfold-p2-bdn-final --noOverwrite > /tmp/spanfold-p2-bdn-final.log 2>&1
```

From `packages/rust`, the exact Rust commands were:

```bash
cargo build --release --bench spanfold_benchmarks
cargo bench --bench spanfold_benchmarks -- episode_relation_sparse > /tmp/spanfold-p2-criterion-final.log 2>&1
```

| .NET scale | Before mean | After mean | Speedup | Before allocated | After allocated |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 64 episodes/side | 234.7 us | 213.9 us | 1.10x | 872.88 KB | 881.69 KB |
| 256 episodes/side | 1.604 ms | 1.117 ms | 1.44x | 3,912.86 KB | 3,948.19 KB |
| 1,024 episodes/side | 17.722 ms | 8.522 ms | 2.08x | 17,650.52 KB | 17,787.36 KB |

| Rust scale | Before middle estimate | After middle estimate | Speedup |
| ---: | ---: | ---: | ---: |
| 64 episodes/side | 213.83 us | 208.14 us | 1.03x |
| 256 episodes/side | 1.0562 ms | 900.17 us | 1.17x |
| 1,024 episodes/side | 7.8685 ms | 3.9065 ms | 2.01x |

The 256-to-1,024 scale ratio fell from 11.1x to 7.6x in .NET and from
7.4x to 4.3x in Rust. The larger representative case is about twice as fast in
both runtimes, while the 64-per-side case improved slightly in .NET and was
effectively unchanged in Rust. .NET allocation increased by less than 1.1%
because the index materializes private lookup buckets. P2 therefore passes the
acceptance gate without changing Episode formation, public APIs, relation
evidence, component semantics, or summary interpretation.

## Transition Comparator Baseline

The lead/lag and as-of workloads use the same deterministic processing-position
shape in .NET and Rust. Each case contains 64, 256, or 1,024 closed target
windows and the same number of against windows. Every record shares the `State`
window, `dense-scope` key, `partition-0` partition, and processing-position
axis, so one candidate list grows with the parameter instead of the workload
being divided across high-cardinality groups.

Against start `i` is at `10i`, and its paired target starts one position later
at `10i + 1`; both windows are four positions long. A tolerance of two therefore
gives every target a real nearest `LeadLag(Start)` match with delta `+1` and a
real `AsOf(Previous)` match at distance `1`. Setup constructs the histories and
reusable configured builders, runs each comparison once, and checks the row
count, match identity, delta or distance, status, and tolerance result. The
timed methods only run the public comparison journey and return or black-box
the materialized result. There are six cases in each runtime: two comparator
operations at three geometric scales.

The baseline was measured on 2026-08-03 on the same Apple M4 Pro and macOS
26.5.2 (25F84) environment as the Episode baseline. .NET used SDK 10.0.102,
runtime 10.0.2, BenchmarkDotNet 0.15.2, Arm64 RyuJIT, concurrent workstation
GC, and the `Short` job. Rust used rustc and Cargo 1.95.0 for
`aarch64-apple-darwin`, Criterion 0.8.2, and Criterion's default 3-second
warmup, 100-sample, approximately 5-second measurement per case.

From the repository root, the exact .NET commands were:

```bash
dotnet build packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -c Release -p:RestoreSources=https://api.nuget.org/v3/index.json
RestoreSources=https://api.nuget.org/v3/index.json dotnet run -c Release --no-build --project packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter '*TransitionComparatorBenchmarks*' --job Short --artifacts /tmp/spanfold-p3-bdn --noOverwrite
```

From `packages/rust`, the exact Rust commands were:

```bash
cargo build --release --bench spanfold_benchmarks
cargo bench --bench spanfold_benchmarks -- transition_comparators
```

### .NET baseline

| Operation | Transitions/side | Mean | Allocated | Previous-scale ratio |
| --- | ---: | ---: | ---: | ---: |
| Lead/lag start | 64 | 211.3 us | 545.25 KB | - |
| Lead/lag start | 256 | 1.4607 ms | 2,175.34 KB | 6.91x |
| Lead/lag start | 1,024 | 15.8069 ms | 8,726.42 KB | 10.82x |
| As-of previous | 64 | 201.8 us | 549.11 KB | - |
| As-of previous | 256 | 1.6852 ms | 2,191.10 KB | 8.35x |
| As-of previous | 1,024 | 14.4275 ms | 8,790.46 KB | 8.56x |

### Rust reference baseline

Criterion reports a confidence interval; the middle estimate is shown here.

| Operation | Transitions/side | Time | Previous-scale ratio |
| --- | ---: | ---: | ---: |
| Lead/lag start | 64 | 742.76 us | - |
| Lead/lag start | 256 | 3.0056 ms | 4.05x |
| Lead/lag start | 1,024 | 12.341 ms | 4.11x |
| As-of previous | 64 | 745.00 us | - |
| As-of previous | 256 | 3.0258 ms | 4.06x |
| As-of previous | 1,024 | 12.653 ms | 4.18x |

These absolute .NET and Rust numbers are not a language comparison. The public
journeys are conceptually aligned and were measured on the same machine, but
the harnesses, runtimes, allocation models, and implementations differ. The
useful comparison is each operation's scale trend within its own runtime.

P3b .NET transition-candidate indexing is justified. From 256 to 1,024
transitions per side, a 4x input increase raised .NET lead/lag time by 10.82x
and as-of time by 8.56x, while allocation remained close to linear. The Rust
reference, whose transition lookup already uses partition points over sorted
candidates, rose by 4.11x and 4.18x respectively. Together with the current
.NET full candidate-list scan per target, this is evidence for replacing that
scan with a private sorted-candidate lookup while preserving scope identity,
deterministic tie-breaking, tolerance, direction, diagnostics, and public APIs.
The checked-in cases should be the before/after gate. No Rust optimization is
indicated by this baseline.

### .NET indexed transition lookup

The after measurement used the same six-case `Short` workload and environment:

```bash
RestoreSources=https://api.nuget.org/v3/index.json dotnet run -c Release --no-build --project packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter '*TransitionComparatorBenchmarks*' --job Short --artifacts /tmp/spanfold-p3b-bdn-final --noOverwrite
```

| Operation | Transitions/side | Before mean | After mean | Speedup | After allocated |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lead/lag start | 64 | 211.3 us | 194.1 us | 1.09x | 545.25 KB |
| Lead/lag start | 256 | 1.4607 ms | 1.1902 ms | 1.23x | 2,175.34 KB |
| Lead/lag start | 1,024 | 15.8069 ms | 14.2611 ms | 1.11x | 8,726.42 KB |
| As-of previous | 64 | 201.8 us | 206.2 us | 0.98x | 549.11 KB |
| As-of previous | 256 | 1.6852 ms | 1.2025 ms | 1.40x | 2,191.04 KB |
| As-of previous | 1,024 | 14.4275 ms | 12.2167 ms | 1.18x | 8,790.46 KB |

P3b passes its gate modestly. At 1,024 transitions per side, the indexed lookup
made lead/lag 1.11x faster and as-of 1.18x faster; their combined time improved
1.14x. The 64-transition lead/lag case improved 1.09x, while the as-of case was
2.2% slower, which is not a meaningful regression in this three-iteration
`Short` measurement. Allocation was effectively unchanged because the lookup
reuses the existing sorted candidate lists and does not materialize per-target
candidate collections.

The full public journey remains superlinear: from 256 to 1,024 transitions per
side, the after ratio is 11.98x for lead/lag and 10.16x for as-of. This change
removes the measured full candidate scan and provides a material absolute
large-case improvement, but it does not establish that transition comparison
now scales linearly. Further transition work is not justified by P3b alone and
would require separate measurement of the remaining costs.

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
