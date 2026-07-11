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

Run the .NET suite with:

```bash
dotnet run -c Release --project benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj # Run the full benchmark suite.
```

For focused work, filter by class or method:

```bash
dotnet run -c Release --project benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter "*ComparisonBenchmarks.RunLiveResidual*" # Run one benchmark target.
dotnet run -c Release --project benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj -- --filter "*SegmentCohortBenchmarks*" # Run segment/cohort benchmarks.
```

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
