# Episodes Tasks

## Discovery Summary

The plan was grounded in the current .NET reference implementation under
`packages/dotnet`. `WindowHistory`, `WindowRecord`, `WindowHistorySnapshot`,
`TemporalPoint`, and `TemporalRange` are the authoritative evidence and
half-open temporal model. The staged comparison surface in
`Comparison/Builders/WindowComparisonBuilder.cs` establishes the consumer
journey to follow: selector, scope, normalization, analytical operation, and
execution. `ComparisonSelector`, `ComparisonScope`, and
`ComparisonNormalizationPolicy` already own reusable source selection,
segment/tag scoping, temporal-axis choice, known-at filtering, and open-window
policy. The private range-normalization logic currently lives in
`Internal/Comparison/Preparation/ComparisonPreparer.cs` and must become a
shared internal seam before episode formation is added, otherwise comparison
and episode analysis can disagree over the same window evidence.

Public episode contracts belong in a new `Spanfold.Episodes` namespace inside
the existing `Spanfold` project. Formation, relationship-graph, identity, and
summary algorithms belong under `Spanfold.Internal.Episodes`; they must not be
added to `WindowHistory` itself or folded into `ComparisonResult`. Behavioral
coverage belongs in `packages/dotnet/tests/Spanfold.Tests/Episodes`. The
repository has no prior `features/` planning bundle and no additional local
agent instructions beyond the request to avoid smoke tests, diagnostic
utilities, dry runs, and test work that does not protect realistic behavior.

Relevant documentation inspected includes `README.md`,
`packages/dotnet/README.md`, `docs/design.md`, `docs/comparison-guide.md`,
`docs/use-cases.html`, and `docs/concepts-advanced-analytics.html`. The current
public story already centers provider outages, pipeline divergence, live
finality, and leakage-safe analysis, making incident/provider episode analysis
the appropriate first documented journey.

## Planning Invariants

- Episodes are an occurrence-level analytical layer over recorded windows.
  They do not replace windows, aligned comparison segments, comparator rows, or
  `ComparisonResult`.
- Add `Spanfold.Episodes` to the existing core `Spanfold` assembly. Do not add a
  NuGet project or package for Episodes.
- Form episodes only after selector matching, scope filtering, known-at
  filtering, temporal-axis normalization, and explicit open-window handling.
- `ComparisonSelector.Matches(WindowRecord)` is the shared selector source of
  truth. Episodes call it directly; do not copy descriptor/predicate semantics
  or reach into `ComparisonPreparer` for selector membership.
- The normalized range rules used by Episodes and Comparison must have one
  internal source of truth. Do not copy `ComparisonPreparer.TryCreateRange()`
  into an episode runtime.
- Preserve half-open range semantics. Touching fragments have a zero gap and
  stitch when the configured tolerance is zero.
- Group formation candidates by window name, configured key equality, source,
  partition, temporal axis, and timestamp clock. Never stitch across a source,
  partition, key, axis, or incompatible clock boundary.
- Do not include segment or tag values in the default episode identity group.
  Segment changes may split source windows without ending the human-scale
  occurrence. Scope filters still apply before formation, and every fragment
  retains its original segments and tags through its `WindowRecord`.
- Respect the window definition's configured key comparer from
  `WindowHistory.KeyComparers`. Default object equality is not an acceptable
  substitute.
- When a configured comparer treats differently represented keys as equal, the
  episode's exposed `Key` and identity input must use the deterministic first
  key after canonical sorting, not whichever key a dictionary happened to see
  first.
- Preserve every selected normalized source fragment in deterministic order.
  Stitching may create an envelope but must never rewrite an internal inactive
  gap as active evidence.
- Compute active magnitude from the union of fragment ranges. Overlapping
  fragments must not be double-counted. Elapsed magnitude is the envelope
  magnitude, and internal-gap magnitude is elapsed minus active.
- Accept processing-position tolerances as `long` magnitudes and event-time
  tolerances as `TimeSpan`. Builder overloads must reject an axis/tolerance
  mismatch rather than interpreting a value on the wrong axis.
- Historical `Run()` requires usable closed evidence under the selected
  normalization policy. Live evaluation requires an explicit horizon; open
  windows are never treated as infinite.
- A plan may obtain an evaluation horizon from either an explicit
  `ClipOpenWindowsTo(...)` normalization used by `Run()` or from `RunLive(...)`,
  but never both. `EpisodeSet.EvaluationHorizon` and the effective returned plan
  must record whichever path was used, and a known-at point cannot coexist with
  an open-window horizon.
- Known-at episode formation is v1 processing-position analysis only. Reject a
  known-at policy on an event-time episode plan because Spanfold has no mapping
  from a processing availability position to a timestamp effective end.
- An episode is provisional if any fragment is provisional. During live
  evaluation, a closed episode is also provisional until the horizon is
  strictly later than its last active end plus the stitch tolerance, because a
  later fragment could still join it.
- Live finality is relative to the supplied history and evaluation horizon. It
  must not claim watermark completeness or protection from future late-arriving
  event-time records, because Spanfold has no watermark contract.
- Relate episodes using actual fragment ranges and the configured proximity
  tolerance. Envelope overlap alone must not create a relationship through an
  internal inactive gap.
- Episode matching is a bipartite relationship graph. Connected components
  classify as one-to-one, split, merge, complex, unmatched target, or unmatched
  against. Do not use greedy nearest-neighbour or arbitrary one-to-one
  assignment.
- Every episode appears in exactly one relationship component. Split means one
  target episode relates to multiple against episodes; merge means multiple
  target episodes relate to one against episode.
- Reject a comparison when the same normalized `WindowRecordId` is selected on
  both target and against sides. Self-comparison would manufacture a trivial
  match and hide overlapping selector definitions.
- A live relation is provisional while any member episode is provisional or
  while the horizon is not strictly beyond the component's latest active end
  plus relation tolerance. Relation tolerance affects association and settling,
  never episode formation or active-magnitude calculations.
- The default API and result vocabulary is neutral: target, against, matched,
  and unmatched. Precision, recall, missed-reference, and unexpected-detection
  language is available only through an explicit reference scorecard.
- Aggregate matched counts count episodes, not graph edges. Split, merge, and
  complex components must not multiply-count an episode.
- Aggregate onset, recovery, and duration distributions use unambiguous
  one-to-one relations only. Component-level metrics may still describe
  split/merge/complex relations, but they must not silently enter one-to-one
  latency percentiles.
- Public collections are materialized and read-only. Repeated enumeration must
  not rerun formation or graph construction.
- Temporal additions, differences, union totals, and summary totals must use
  explicitly overflow-safe arithmetic. Never allow wraparound to create an
  early finality boundary, negative duration, or inverted bias.
- Episode IDs and ordering are deterministic for the same normalized .NET
  evidence. IDs are opaque and scoped to the producing .NET analysis; do not
  promise Rust parity or distributed global identity.
- Use `CanonicalValueFormatter` and structural range/record data for identity
  and deterministic ordering. Do not use arbitrary `object.ToString()` as an
  identity protocol.
- All new public types and members require XML documentation because
  `Spanfold.csproj` treats `CS1591` as an error.
- Core episode execution is in-memory and side-effect free. No file I/O,
  persistence, timers, subscriptions, schedulers, background monitoring,
  dashboards, or network calls belong in this feature.
- Do not add artifact schemas, bundle members, CLI commands, comparison fixture
  fields, generic extension hooks, a policy DSL, a general CEP engine, or a new
  sample application in this bundle.
- Do not modify Rust crates or claim cross-runtime episode parity. The existing
  shared comparison fixture/export contracts remain unchanged.
- Tests must protect formation, finality, graph classification, and analyst
  metric correctness. Do not create smoke tests, diagnostic scripts, temporary
  utilities, broad snapshot churn, or benchmarks for this feature.

## Task Dependency Table

| ID | Completed | Title | Description | Github Issue # | Blocked By | Task File |
|---|---|---|---|---|---|---|
| T001 | [x] | Extract shared window-range normalization | Move temporal range and scope preparation into a neutral internal seam while preserving every existing comparison behavior and diagnostic mapping. | #81 | None | [`tasks/T001.md`](tasks/T001.md) |
| T002 | [x] | Form deterministic episodes from window history | Add the `Spanfold.Episodes` formation model, fluent builder, fragment-preserving runtime, deterministic identity, and explicit historical/live finality semantics. | #82 | T001 | [`tasks/T002.md`](tasks/T002.md) |
| T003 | [x] | Relate episode sets as a component graph | Add the `CompareEpisodes` plan and runtime that forms both sides, relates actual fragments, and classifies deterministic one-to-one, split, merge, complex, and unmatched components. | #83 | T002 | [`tasks/T003.md`](tasks/T003.md) |
| T004 | [x] | Add analyst summaries and reference scorecards | Add single-set and comparison summaries, deterministic latency distributions, internal-fragment and split/merge measures, and an explicit target-as-reference scorecard. | #84 | T003 | [`tasks/T004.md`](tasks/T004.md) |
| T005 | [x] | Document episode analytics workflows | Document provider evaluation, detector evaluation, downtime, and sessionisation using the final fluent API while explaining when exact span comparison remains preferable. | #85 | T002, T003, T004 | [`tasks/T005.md`](tasks/T005.md) |

Task details live in separate files under `tasks/`, named by task ID.

## Final Notes

- Recommended implementation order: T001, T002, T003, T004, then T005.
- Unresolved questions: artifact export, CLI input/output, cohort bucketing,
  cross-window sequences, and Rust parity are intentionally deferred until the
  core episode result proves useful. They are not hidden requirements of T005.
- Risks: custom key equality, timestamp-clock comparability, live settling at
  tolerance boundaries, overlap double-counting, and split/merge graph
  multiplicity are the highest-risk correctness areas.
- Areas that need human review before implementation: any proposal to group by
  segments/tags by default, compare episode sets produced by unrelated
  histories, assign one-to-one matches inside a complex component, expose
  precision/recall without explicit reference semantics, or extend a persisted
  schema should stop for review rather than expanding this plan.
