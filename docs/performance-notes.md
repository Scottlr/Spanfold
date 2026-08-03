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
- direct overlap and residual history queries across incompatible scopes, with
  a dense same-scope control

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

## Rust ingestion observation reuse

The affected Rust ingestion case records 8,192 deterministic events across 128
selections and four sources. Each selection/source lane repeatedly opens, changes
its `phase` and `period` boundary segments, updates only its `state` tag, and
closes. The source window has four segment dimensions and one tag; its market
roll-up preserves `phase` and `period` and renames `phase` to `trading_phase`.
This exercises active and metadata selectors, segment-change close/reopen,
tag-only updates, segment projection, child membership, roll-up state, emissions,
and history recording.

The affected Criterion case prepares deterministic events and source strings
outside measurement. `iter_batched` clones that prepared input and constructs a
fresh pipeline in its untimed setup; the timed routine only loops over the 8,192
`ingest` calls and returns the resulting pipeline to `black_box`. The existing
simple case remains unchanged and includes its historical fixture construction
and ingestion journey, so its before/after values are comparable to each other
but not directly to the affected case.

The measurements were taken on 2026-08-03 on an Apple M4 Pro (12 cores), macOS
26.5.2 (25F84), using rustc and Cargo 1.95.0 for `aarch64-apple-darwin`,
Criterion 0.8.2, and Criterion's default 3-second warmup, 100 samples, and
approximately 5-second target measurement. From `packages/rust`, the exact
commands were run before and after the production change:

```bash
cargo bench --bench spanfold_benchmarks -- ingest_8192
cargo bench --bench spanfold_benchmarks -- metadata_rollup_8192
```

Criterion reports a confidence interval; the middle estimate is used for the
ratio.

| Workload | Before | After | After/before | Speedup |
| --- | ---: | ---: | ---: | ---: |
| Simple ingestion, 8,192 events | 13.242 ms `[13.174, 13.339]` | 13.285 ms `[13.252, 13.318]` | 1.003x | 0.997x |
| Metadata and projected roll-up ingestion, 8,192 events | 70.245 ms `[70.105, 70.384]` | 67.033 ms `[66.616, 67.609]` | 0.954x | 1.048x |

The gate passes. Reusing preflighted active, segment, and projected roll-up
observations reduced the representative affected workload by 4.57%. The simple
path moved by 0.32%, which is not a meaningful regression. The change preserves
the preflight-before-mutation boundary, consumes projected observations without
recloning them, and caches the immutable definition-tree record bound at pipeline
construction. No public API, event-time, watermark, callback emission, or
lifecycle ownership changes were made.

## Rust comparison grouping reuse

The affected comparison case contains 1,024 preserved duplicate windows for the
target source and for each of three sources in an `Any` cohort. All 4,096 windows
share a window family, key, partition, and range, so the full comparison exercises
duplicate diagnostics, cohort activity, deterministic record-ID evidence, and the
real `execute_compare` path. The history and comparison builder are constructed
outside measurement. The timed `run_overlap` routine performs preparation,
alignment, comparison, evidence materialization, and result construction. The
`align` case is the representative control and performs preparation plus public
alignment over the same input. An untimed assertion confirms that the run remains
valid, emits one overlap row with 1,024 target and 3,072 against record IDs, and
retains the `DuplicateWindow` diagnostic.

Before this change, successful full execution called `group_normalized_windows`
once through alignment and then immediately called it again for comparator
execution, including transition comparators. Both passes cloned the deterministic
five-part group keys and every normalized record-ID vector. Full execution now
creates that private grouping state once and lends it to alignment and any
comparator that needs it. The public `align` API still owns its single grouping
pass. Public APIs, ordering, diagnostics, duplicate and cohort semantics, evidence,
and export shapes are unchanged.

The measurements were taken on 2026-08-03 on an Apple M4 Pro (12 cores), macOS
26.5.2 (25F84), using rustc and Cargo 1.95.0 for `aarch64-apple-darwin`, Criterion
0.8.2, and Criterion's default 3-second warmup, 100 samples, and approximately
5-second target measurement. From `packages/rust`, the exact command was run
before and after the production change:

```bash
cargo bench --bench spanfold_benchmarks -- dense_duplicate_cohort
```

Criterion reports a confidence interval; the middle estimate is used for the
ratio.

| Workload | Before | After | After/before | Speedup |
| --- | ---: | ---: | ---: | ---: |
| Public alignment control | 5.2623 ms `[5.2270, 5.3119]` | 5.2751 ms `[5.2304, 5.3264]` | 1.002x | 0.998x |
| Full duplicate-cohort overlap | 14.585 ms `[14.481, 14.711]` | 13.954 ms `[13.863, 14.088]` | 0.957x | 1.045x |

The gate passes. Criterion measured a statistically significant 4.33% reduction
for the affected full comparison (`p = 0.00`), while the alignment control moved
by 0.24% and Criterion detected no performance change (`p = 0.71`). This bounded
reuse removes one complete grouping/allocation pass. It does not change
preparation, eliminate record-ID materialization required by aligned rows, or
claim improvements for history queries or cohort evidence construction.

## Rust direct history-query scope indexing

The affected direct-query workload contains 4,096 closed processing-position
windows with the same range and window family but a distinct key per record;
sources alternate between `target` and `against`. It isolates the cost of
rejecting impossible cross-scope pairs: `find_overlaps` returns no pairs, while
`find_residuals("target")` returns the 2,048 unchanged target ranges. The fixture
and untimed result-shape assertions are constructed outside measurement, and the
timed routines call the public `WindowHistory::find_overlaps` and
`WindowHistory::find_residuals` methods directly.

The dense control contains 64 target and 64 against records. Every record has
the same window, key, partition, processing-position range, axis, and clock
domain. All 128 windows overlap, producing 8,128 overlap pairs, and the against
records completely cover every target residual. This control keeps genuine
same-scope candidate and output work visible.

The measurements were taken on 2026-08-03 on an Apple M4 Pro, macOS 26.5.2
(25F84), using rustc and Cargo 1.95.0 for `aarch64-apple-darwin`, Criterion
0.8.2, and Criterion's default 3-second warmup, 100 samples, and approximately
5-second target measurement. From `packages/rust`, the identical command was
run before and after the production change:

```bash
cargo bench --bench spanfold_benchmarks -- history_direct_queries
```

Criterion reports a confidence interval; the middle estimate is used for the
ratio.

| Workload | Before | After | Speedup |
| --- | ---: | ---: | ---: |
| Overlaps, 4,096 incompatible scopes | 27.349 ms `[26.967, 27.844]` | 1.0652 ms `[1.0580, 1.0757]` | 25.67x |
| Residuals, 4,096 incompatible scopes | 42.079 ms `[41.854, 42.378]` | 687.01 us `[682.34, 694.02]` | 61.25x |
| Overlaps, 128 dense same-scope records | 2.4491 ms `[2.4375, 2.4604]` | 2.3930 ms `[2.3834, 2.4025]` | 1.02x |
| Residuals, 128 dense same-scope records | 99.153 us `[97.754, 100.83]` | 42.364 us `[41.600, 43.303]` | 2.34x |

The gate passes. The private borrowed scope index groups candidates by the
existing compatibility identity: window family, key, partition, temporal axis,
and timestamp clock. Candidate vectors retain original record order;
`find_overlaps` also retains the original first-record outer order and only
considers later record indexes, while `find_residuals` retains target order and
comparison subtraction order. Public APIs, serialized shapes, half-open overlap
semantics, source exclusion, range values, axes, and clock identities are
unchanged. Criterion reported improvements in both affected cases and both
controls (`p = 0.00`).

The index only removes impossible cross-scope comparisons. A genuinely dense
same-scope overlap result still contains a quadratic number of pairs and must
clone and return them; dense residual subtraction likewise still examines the
eligible same-scope comparison sequence. This change does not claim to remove
that inherent output or candidate cost.

## Rust cohort activity evidence reuse

The affected workload runs the public comparison path over one target window and
1,536 staggered against windows: 64 windows for each of 24 cohort sources. The
windows overlap for 512 processing positions and start 32 positions apart, so
many windows from the same source remain active across each of 1,919 aligned
segments. Fixture and builder construction are outside measurement. Untimed
assertions cover a 12-of-24 threshold, inactive edge segments, a fully active
middle segment, sorted parsed `cohort_evidence()` sources, and retained
against-record-ID evidence. A one-source version of the same workload is the
control.

Before this change, each aligned boundary updated the active against-index set,
then scanned every active index and rebuilt a `BTreeSet` to recover the distinct
active source identities. Alignment now updates a private source reference count
as against windows enter and leave. Each public aligned segment still owns its
sorted source vector, against record IDs, and target record IDs; only the repeated
source deduplication is reused across boundaries. A focused regression protects
the case where one of two overlapping windows for a source leaves while the other
must keep that source active.

The measurements were taken on 2026-08-03 on an Apple M4 Pro, macOS 26.5.2
(25F84), using rustc and Cargo 1.95.0 for `aarch64-apple-darwin` and Criterion
0.8.2. Criterion used its default 3-second warmup and 100 samples. The affected
case used estimated measurement intervals of 6.99 seconds before and 6.60 seconds
after; the control used 5.18 seconds before and 5.12 seconds after. From
`packages/rust`, the identical command was run before and after the production
change:

```bash
cargo bench --bench spanfold_benchmarks -- cohort_activity
```

| Workload | Before | After | After/before | Speedup |
| --- | ---: | ---: | ---: | ---: |
| Staggered 24-source cohort | 70.832 ms `[70.300, 71.649]` | 65.814 ms `[65.470, 66.197]` | 0.929x | 1.076x |
| Staggered one-source control | 519.50 us `[516.55, 522.42]` | 503.40 us `[500.20, 506.54]` | 0.969x | 1.032x |

The gate passes. Criterion reported a statistically significant 7.08% affected-
path improvement (`p = 0.00`) and a 2.83% control improvement (`p = 0.00`), with
no control regression. Public APIs, alignment and row order, lexicographic source
order, duplicate-window/source counts, all cohort activity rules, diagnostics,
extension metadata, parsed cohort evidence, record-ID evidence, exports, and
temporal enter/leave semantics are unchanged.

This reuse is bounded to source membership during comparison alignment. It does
not remove the contractually required per-segment source vector or record-ID
materialization, change comparator grouping, optimize direct history queries, or
address result/export materialization.

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
