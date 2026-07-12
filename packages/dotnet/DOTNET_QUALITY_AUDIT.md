# C#/.NET Quality Audit Issue Catalog

> **Historical audit snapshot.** The `Open` labels below describe revision
> `7f39c18`; they are not the current status of `main`. Findings were addressed
> through the linked NET-* pull-request series, and the final current-state
> review is tracked in repository history rather than by rewriting this
> evidence snapshot.

**Audit date:** 2026-07-12  
**Audited revision:** `7f39c18` (`main`)  
**Scope:** the complete C#/.NET surface under `packages/dotnet`, including the core library, testing package, CLI, samples, benchmarks, tests, package metadata, root documentation, and .NET CI gates  
**Disposition at audited revision:** **Not showcase-quality and not ready for a high-confidence OSS release.**

This is an intentionally aggressive principal-level review. It traces actual state, comparison, export, CLI, and packaging paths rather than treating passing tests or public XML documentation as proof of correctness. The review assumes keys and segment values can be non-trivial objects, events can fail during user projections, histories can grow, live snapshots can change shape, and exported schemas will become compatibility contracts.

No build, test run, smoke test, diagnostic script, temporary utility, or dry run was created or executed for this audit. That is deliberate: this was a review-only task, and the repository instructions prohibit unnecessary validation work. Findings are based on static code and data-flow inspection. All findings are **Open** unless their status is edited in this catalog.

Severity follows the requested scale:

- **P0 Critical:** catastrophic compromise or loss with no credible containment. No P0 finding was substantiated.
- **P1 High:** credible silent corruption, materially false analytical output, unstable identity, or a release-blocking contract failure.
- **P2 Medium:** substantial reliability, performance, API, security, or maintainability defect.
- **P3 Low:** real release-quality debt with limited immediate impact.
- **Nit:** localized cleanup with little independent risk.

## Bottom line

The main problem is not formatting or use of modern C#. The problem is that public promises exceed the invariants enforced by the implementation. Custom equality stops at the runtime dictionary boundary. Roll-up membership is not modeled robustly. “Serializable” plans do not contain executable selector data. Several behavioral options are inert. Segment context disappears from important result and export paths. Live changelogs use list positions as identity and replay revisions incorrectly. Core algorithms repeatedly rescan whole interval sets and retain state without a lifecycle.

The package should not be presented as deterministic, portable, leakage-safe, or scalable until those claims are true for the end-to-end path, not just for the fluent happy path.

## Required release gates

1. Repair runtime identity, roll-up membership, and partial-commit behavior.
2. Replace formatted-string identity with structural keys and stable occurrence/row identities.
3. Correct known-at, live finality, and changelog semantics.
4. Remove or implement every exported behavioral option and speculative extension surface.
5. Preserve segment/domain context through rows and every export format.
6. Replace interval rescans with a real sweep and define retention for long-lived state.
7. Narrow the public construction surface before external consumers depend on it.
8. Make package, API-compatibility, documentation, and CLI claims enforceable in CI.

---

## 1. Correctness and reliability findings

### NET-COR-001 — P1 High — Custom key equality is lost when history records a close

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Runtime/WindowRuntime.cs:12-16,31-35,59-73`; `packages/dotnet/src/Spanfold/Internal/Keys/RuntimeStateKeyComparer.cs:12-24`; `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:567-607`; `packages/dotnet/src/Spanfold/Internal/Recording/WindowRecordingKey.cs:3-8`; `packages/dotnet/tests/Spanfold.Tests/Runtime/CustomKeyComparerTests.cs:7-23`.

Runtime state uses the configured key comparer, but `WindowHistory.openWindows` uses default record/object equality. With `StringComparer.OrdinalIgnoreCase`, opening `"Selection-1"` and closing `"selection-1"` closes runtime state but fails to remove the recorded open window. The close is silently discarded at `WindowHistory.cs:590-593`. The existing comparer test only checks that a duplicate open is suppressed; it never closes or inspects history.

**Failure/impact:** runtime callbacks say the window closed while recorded history says it remains open forever. Later live comparisons, snapshots, and exports are false.  
**Smallest correction:** carry the configured comparer/canonical key into recording, or close history by a stable runtime occurrence identifier rather than reconstructing a key with different equality. Add one regression covering open, case-variant close, and recorded closure.  
**Broader opportunity:** make logical identity a first-class value shared by runtime, recording, querying, and comparison.  
**Confidence:** High.

### NET-COR-002 — P1 High — Roll-up child membership ignores the child key comparer

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Runtime/RollUpRuntime.cs:12-16,36-50,271-290`; `packages/dotnet/src/Spanfold/Internal/Definitions/WindowDefinition.cs:83-118`.

The parent-state dictionary uses the parent definition's comparer, but `ParentState.Children` is a default `Dictionary<object, bool>`. A source window can legitimately define case-insensitive or domain-specific child equality, yet the roll-up stores case variants as separate children.

**Failure/impact:** active counts and `TotalCount` diverge from source-window identity; `AllActive`, exact-count, and threshold predicates can stay open or closed incorrectly.  
**Smallest correction:** construct child membership with the child definition's comparer, which means `RollUpRuntime` must receive the child identity policy when it is built.  
**Confidence:** High.

### NET-COR-003 — P1 High — Roll-ups skip predicate evaluation when known membership changes without an activity transition

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Runtime/RollUpRuntime.cs:44-70`; `packages/dotnet/src/Spanfold/Windows/ChildActivityView.cs:19-30`; `packages/dotnet/src/Spanfold/Internal/Runtime/WindowRuntime.cs:31-35,117-131`.

`ObserveChild` writes `parent.Children[childKey]` and returns without evaluating the parent predicate whenever `childChanged` is false. A previously unknown inactive child has `childChanged == false`, but adding it changes `TotalCount`. An `AllActive()` parent with one active child therefore remains open after a second known inactive child appears.

**Failure/impact:** roll-up state contradicts the documented meaning of “every known child.”  
**Smallest correction:** distinguish “membership changed” from “activity changed” and reevaluate whenever either can change the predicate input.  
**Confidence:** High.

### NET-COR-004 — P1 High — An active child cannot migrate between parent keys

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Runtime/WindowRuntime.cs:117-131`; `packages/dotnet/src/Spanfold/Internal/Runtime/RollUpRuntime.cs:36-66,125-166`.

The parent key is recomputed from every current event. If a child remains active while its parent key changes, `childChanged` is false. The child is added as active under the new parent, the new parent is not evaluated, and there is no remembered child-to-old-parent relationship to deactivate the old parent. Segment-transition handling also evaluates both old and new segment contexts using the current event's parent key.

**Failure/impact:** the old parent can remain open forever while the new parent never opens. A device moving zone while offline is enough to trigger it.  
**Smallest correction:** retain each child's previous parent identity and process a move as an atomic old-parent removal plus new-parent addition.  
**Broader opportunity:** represent roll-up membership explicitly instead of inferring all lineage from the current event.  
**Confidence:** High.

### NET-COR-005 — P1 High — Ingestion is not atomic and callback failure is indistinguishable from ingestion failure

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Pipeline/EventPipeline.cs:80-108`; `packages/dotnet/src/Spanfold/Internal/Runtime/WindowRuntime.cs:31-131`; `packages/dotnet/src/Spanfold/Internal/Definitions/RollUpSegmentProjection.cs:47-60`.

`processingPosition` advances before any user selector runs. Each window runtime mutates in sequence. Event time is evaluated only after all runtime mutations. History is committed before callbacks. A later predicate, key selector, segment projection, event-time selector, or callback can therefore throw after earlier state has changed. A callback exception also stops remaining callbacks, but the event and history are already committed.

**Failure/impact:** retrying after an exception observes a different state and can lose emissions or duplicate effects. The caller receives no indication that the event committed before the exception.  
**Smallest correction:** pre-evaluate fallible event observations before mutation and return a distinct post-commit callback failure that carries the committed ingestion result. At minimum invoke all callbacks and aggregate failures after state/history commit.  
**Broader opportunity:** use a two-phase observe/commit model for a single event.  
**Confidence:** High.

### NET-COR-006 — P1 High — Segment identity uses an ambiguous delimiter protocol

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Runtime/RollUpRuntime.cs:226-246`; `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:823-843`; `packages/dotnet/src/Spanfold/Internal/Comparison/Alignment/ComparisonAligner.cs:193-232`; `packages/dotnet/src/Spanfold/Definitions/SegmentBuilder.cs:16-23`.

Segment vectors are flattened into strings with `/`, `=`, and `;` separators. Names and values are not escaped or length-prefixed, and builders only reject whitespace-only names. Distinct structural segment vectors can therefore produce the same string. The aligner also trusts the formatted segment string without checking structural segment equality.

**Failure/impact:** unrelated segment lanes can share roll-up state, overwrite open history, or align into one analytical lane. This is silent data corruption.  
**Smallest correction:** use a structural segment-key type with element-wise equality and hashing. Do not use display text as an identity protocol.  
**Confidence:** High.

### NET-COR-007 — P1 High — Arbitrary object `ToString()` output is treated as stable identity

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowRecordId.cs:23-43,66-73`; `packages/dotnet/src/Spanfold/Internal/Comparison/Preparation/ComparisonPreparer.cs:49-56,340-347`; `packages/dotnet/src/Spanfold/Internal/Comparison/Alignment/ComparisonAligner.cs:130-190`; `packages/dotnet/src/Spanfold/Recording/WindowSnapshotQuery.cs:10-21,140-150`.

Window IDs and deterministic sort keys fall back to type name plus `ToString()`. Two unequal instances of the same type with the same/default string representation receive the same `WindowRecordId`. Mutable or throwing `ToString()` implementations can also change identity or fail queries and exports. Snapshot lookup stores records by that ID and overwrites a collision.

**Failure/impact:** record lineage, containment maps, snapshots, finality, and exports can point at the wrong window or lose one entirely. “Deterministic” is not true for the public `object` domain.  
**Smallest correction:** define the supported canonical value domain and codec, or require callers to supply a stable identity serializer/comparer. Use a collision-free occurrence identity internally even if a deterministic public fingerprint remains.  
**Confidence:** High.

### NET-COR-008 — P1 High — Known-at filtering erases facts that were already observable while a window was open

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Preparation/ComparisonPreparer.cs:30-69,319-323`; `packages/dotnet/tests/Spanfold.Tests/Comparison/KnownAtComparisonTests.cs:9-24`; `packages/dotnet/README.md:411-430`.

A closed window is considered available only at its eventual end position. At a known-at point between open and close, the entire window is excluded even though its opening and active state were already observed. The existing test explicitly codifies this loss.

**Failure/impact:** backtests and point-in-time audits omit active incidents that were known at decision time, contradicting the package's leakage-safe claim.  
**Smallest correction:** when `start <= knownAt < end`, materialize the provisional range `[start, knownAt)` with appropriate finality instead of excluding the record.  
**Broader opportunity:** model open and close observations as bitemporal revisions rather than deriving availability from the final record only.  
**Confidence:** High.

### NET-COR-009 — P1 High — Changelog replay turns every revision into `Final`

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Rows/ComparisonChangelog.cs:45-57,92-108`; `packages/dotnet/src/Spanfold/Comparison/Rows/ComparisonChangelogEntry.cs:4-18`; `packages/dotnet/tests/Spanfold.Tests/Comparison/ComparisonChangelogTests.cs:7-33,50-70`.

`Create` uses `ComparisonFinality.Revised` as a change kind and discards the current row's resulting finality. `Replay` then maps every `Revised` entry to `Final`. The tests cover only Provisional-to-Final, the direction that hides the defect.

**Failure/impact:** a Final-to-Provisional change replays as Final, corrupting audit and dashboard state.  
**Smallest correction:** separate change kind from resulting finality and preserve the current reason. Cover Final-to-Provisional and reason-only revisions with behavioral regressions.  
**Confidence:** High.

### NET-COR-010 — P1 High — Positional row IDs are not identities

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:939-978`; `packages/dotnet/src/Spanfold/Comparison/Rows/ComparisonChangelog.cs:26-57`; `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:683-689`.

Rows are identified as `rowType[index]`. Adding or removing an earlier row shifts every later identity. If a different semantic row occupies the same index with the same finality/reason, the changelog emits no change; otherwise it reports a “revision” between unrelated rows.

**Failure/impact:** live diffs, citations, and retractions attach to the wrong evidence.  
**Smallest correction:** derive row IDs from canonical row family, scope, segment range, and sorted contributing record IDs. Use the same helper in runtime, JSON, JSONL, and changelog.  
**Confidence:** High.

### NET-COR-011 — P1 High — Transition comparators and gap detection cross segment lanes

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:418-455,504-543,1063-1068,1219-1221`; `packages/dotnet/src/Spanfold/Internal/Comparison/Alignment/ComparisonAligner.cs:193-209`.

Alignment treats segment context as part of scope, but lead/lag and as-of keys include only window name, key, and partition. Gap adjacency uses the same reduced scope. A phase-A target can therefore match a phase-B comparison transition, and a “gap” can be created between adjacent segments belonging to different segment lanes.

**Failure/impact:** transition and gap rows answer a different question from overlap/residual alignment without any diagnostic.  
**Smallest correction:** include structural segment context in every comparator scope that operates on segmented windows.  
**Confidence:** High.

### NET-COR-012 — P1 High — Every gap row is marked final, including gaps bounded by open windows

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Rows/GapRow.cs:3-19`; `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:283-307,907-910,939-956`.

`GapRow` carries no lineage. Finality generation passes no contributing record IDs, so `HasProvisionalRecord` cannot return true and every gap is `Final`. Yet a gap boundary can move when an adjacent open window advances or closes.

**Failure/impact:** live consumers are told unstable evidence is final.  
**Smallest correction:** retain adjacent/bounding record IDs or an explicit provisional dependency on the gap row and use it during finality calculation.  
**Confidence:** High.

### NET-COR-013 — P1 High — Mutable pipeline and history state has no concurrency or reentrancy contract

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Pipeline/EventPipeline.cs:12-16,80-105`; `packages/dotnet/src/Spanfold/Internal/Runtime/WindowRuntime.cs:8-17`; `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:15-25,31-82`.

Positions, runtime dictionaries, roll-up dictionaries, history lists, and annotations are unsynchronized. There is no documented single-writer/no-concurrent-query contract and no fail-fast guard. Callbacks can also call `Ingest` reentrantly while the outer invocation is still delivering emissions.

**Failure/impact:** concurrent ingestion can duplicate positions or corrupt dictionaries; concurrent queries can observe inconsistent collection state; reentrant ingestion produces ambiguous ordering.  
**Smallest correction:** explicitly define single-threaded, non-reentrant instance semantics and enforce them with a cheap guard. If concurrent ingestion is a requirement, design partitioned ownership or synchronization around a stated callback policy.  
**Confidence:** High on unsafety; Medium on intended concurrency contract because none is documented.

### NET-COR-014 — P1 High — Duplicate cohort sources corrupt threshold semantics

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Cohorts/ComparisonCohortBuilder.cs:25-35,51-63`; `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:1136-1140,1167-1204`.

The builder accepts duplicate source identities. Runtime deduplicates active sources but compares that count with the raw declared-source count. `All`, `AtLeast`, and exact rules therefore use incompatible populations.

**Failure/impact:** a duplicated member can make `All` permanently false or change threshold results without adding a real source.  
**Smallest correction:** validate non-empty unique source identities in the domain constructor used by every entry path, then calculate rules from that unique set.  
**Confidence:** High.

### NET-COR-015 — P2 Medium — Liveness accepts observations earlier than an already evaluated horizon

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Liveness/LaneLivenessTracker.cs:82-101,121-151`; `packages/dotnet/tests/Spanfold.Tests/Runtime/LaneLivenessTrackerTests.cs:22-54`.

`Observe` rejects timestamps before tracker start or a lane's prior observation, but not timestamps before `lastCheckAt`. After checking horizon 100 and emitting silence, a later call can observe a heartbeat at 50, emit a retroactive recovery, and then emit silence again from a recalculated boundary.

**Failure/impact:** previously emitted liveness history is revised without a revision model, creating duplicate or contradictory silence windows.  
**Smallest correction:** reject `observedAt < lastCheckAt`, or explicitly model late observations as revisions/retractions instead of ordinary state changes.  
**Confidence:** High.

### NET-COR-016 — P2 Medium — Equal-time lead/lag matches are selected without a stable tie-break

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:440-443,475-492,685-704`; contrast `524-532` for as-of.

Lead/lag sorts comparison transitions only by point and `List<T>.Sort` is not stable. `FindNearest` keeps the first equal-distance candidate. As-of correctly adds a record-ID tie-break, but lead/lag does not.

**Failure/impact:** equal-time inputs can produce different lineage after harmless input reordering.  
**Smallest correction:** sort and tie-break by point then stable record ID, and decide whether equal candidates should also emit an ambiguity diagnostic.  
**Confidence:** High.

### NET-COR-017 — P2 Medium — Temporal delta arithmetic can overflow or throw on valid public values

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:475-489,782-798`; `packages/dotnet/src/Spanfold/Temporal/TemporalRange.cs:127-156`.

Position/tick subtraction is unchecked and `Math.Abs(long.MinValue)` throws. Public temporal points and records admit the full `long` range, so an extreme but valid pair can overflow before tolerance comparison. Range length has the same unchecked subtraction pattern.

**Failure/impact:** public input can turn comparison or aggregation into an unexpected exception or wrapped magnitude.  
**Smallest correction:** use checked arithmetic with a typed range diagnostic, or compute unsigned/saturating distance without negating `long.MinValue`.  
**Confidence:** High.

### NET-COR-018 — P2 Medium — `AnnotationsKnownAt` promises exclusion for incomparable clocks but throws instead

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:242-282`; `packages/dotnet/src/Spanfold/Temporal/TemporalPoint.cs:195-210`.

The method documentation says annotations without a comparable known-at point are excluded. The implementation checks only axis equality and then calls `CompareTo`; two timestamp points with different clock IDs throw `InvalidOperationException` instead of being excluded.

**Failure/impact:** one annotation from another clock aborts the whole query.  
**Smallest correction:** add an explicit comparability check/TryCompare path and apply the documented exclusion policy.  
**Confidence:** High.

### NET-COR-019 — P2 Medium — CLI fixture schema identity and version are never validated

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold.Cli/SpanfoldCli.cs:593-603`; `docs/fixture-schema.md:10-46`.

Validation checks that `schema` is a string and `schemaVersion` is a number, but never checks their values. A foreign schema or future incompatible version is silently interpreted as the current contract.

**Failure/impact:** versioned fixtures can be misread instead of rejected, defeating the compatibility envelope.  
**Smallest correction:** require the exact supported schema ID and version and return a contextual unsupported-version error.  
**Confidence:** High.

### NET-COR-020 — P2 Medium — CLI numeric parse failures escape the structured error boundary

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold.Cli/SpanfoldCli.cs:16-78,218-221,245-270,593-620`.

The catch filter handles IO, JSON, argument, and invalid-operation exceptions. `long.Parse`, `JsonElement.GetInt64`, and numeric overflow can throw `FormatException` or `OverflowException`, neither of which is caught.

**Failure/impact:** malformed numeric input can terminate the process with an unstructured runtime failure instead of the documented JSON error and exit code.  
**Smallest correction:** use `TryParse`/`TryGetInt64` with option or JSON-path context and include the expected numeric range.  
**Confidence:** High.

---

## 2. Architecture and maintainability findings

### NET-ARC-001 — P1 High — `Spanfold.Testing` depends on private fields, private constructors, and an internal type name

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold.Testing/WindowHistoryFixtureBuilder.cs:145-201`.

The separately packaged testing library reflects `WindowHistory(bool)`, fields named `closedWindows`/`openWindows`, and `Spanfold.Internal.Recording.WindowRecordingKey`, then constructs private storage directly.

**Failure/impact:** an otherwise compatible core refactor, trimming/AOT, or package-version skew breaks the testing package at runtime with no compile-time warning.  
**Smallest correction:** add a narrow core-owned fixture/import factory that validates records and is callable by `Spanfold.Testing`; remove all private reflection.  
**Confidence:** High.

### NET-ARC-002 — P2 Medium — `WindowHistory` is storage, query factory, annotation store, comparison facade, and analytics engine

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:13-124,127-327,330-554,557-843`.

One 927-line public class owns mutable recording state, annotations/revisions, snapshots, source matrices, hierarchy analysis, direct overlap/residual algorithms, and comparison construction. Changes to retention or storage representation therefore touch unrelated analytical concerns, and alternative persistence cannot be introduced behind a narrow seam.

**Failure/impact:** the module is difficult to evolve safely and encourages every new analytical shortcut to land on the state owner.  
**Smallest correction:** keep `WindowHistory` as the façade but move matrix, hierarchy, and direct interval algorithms into cohesive internal modules over an immutable history snapshot.  
**Broader opportunity:** deepen a single history snapshot/index abstraction rather than adding service interfaces per operation.  
**Confidence:** High.

### NET-ARC-003 — P2 Medium — Comparator execution is a string-dispatched monolith with high change fan-out

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:5-165,180-840,850-1221`; `packages/dotnet/src/Spanfold/Comparison/Builders/ComparisonComparatorBuilder.cs:8-174`.

The 1,222-line runtime parses declarations, dispatches every comparator, builds every row family, computes summaries/finality/cohort metadata, and defines all option parsing. Adding a comparator requires coordinated edits to builder strings, catalog parsing, runtime dispatch, result construction, queries/assertions, explainers, and exports.

**Failure/impact:** shotgun surgery makes contract drift—already visible in row-family spellings—likely.  
**Smallest correction:** split internal comparator algorithms and row-finality construction into cohesive modules while retaining a single typed dispatch point. Avoid one public interface per comparator unless third-party execution is actually supported.  
**Confidence:** High.

### NET-ARC-004 — P2 Medium — Canonicalization logic is duplicated and already disagrees

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowRecordId.cs:55-118`; `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:813-843`; `packages/dotnet/src/Spanfold/Internal/Runtime/RollUpRuntime.cs:226-246`; `packages/dotnet/src/Spanfold/Internal/Comparison/Alignment/ComparisonAligner.cs:130-137,212-232`; `packages/dotnet/src/Spanfold.Testing/WindowHistoryFixtureBuilder.cs:181-224`.

Identity/display encoders are independently implemented in at least five places. Some include type names, some use raw `ToString()`, some length-prefix fields, and some use ambiguous separators. The testing package reconstructs private keys with yet another copy.

**Failure/impact:** equality, ordering, IDs, recording, fixtures, and export can disagree for the same value. NET-COR-001, NET-COR-006, and NET-COR-007 are consequences.  
**Smallest correction:** introduce one internal structural value/segment identity component with separate display formatting.  
**Confidence:** High.

### NET-ARC-005 — P2 Medium — The extension API describes behavior that core cannot register or execute

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Extensions/ComparisonExtensionBuilder.cs:28-82`; `packages/dotnet/src/Spanfold/Comparison/Comparators/ComparisonComparatorCatalog.cs:3-10,50-61`; `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:35-45`; `packages/dotnet/tests/Spanfold.Tests/Comparison/ComparisonExtensionTests.cs:7-68`.

Extensions can declare selectors and comparators, but there is no registry, handler, selector AST, or execution hook. Runtime rejects every non-core comparator as unknown. Tests prove descriptor construction and manually injected metadata only, not extension behavior.

**Failure/impact:** the public API implies an extensibility story that does not exist and reserves compatibility surface prematurely.  
**Smallest correction:** remove/internalize the descriptor surface for the preview, or introduce one real cohesive registration/execution contract before documenting extension packages.  
**Confidence:** High.

### NET-ARC-006 — P2 Medium — Public prepared/aligned/result stages expose forgeable internal pipeline state

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Preparation/PreparedComparison.cs:20-38`; `packages/dotnet/src/Spanfold/Comparison/Preparation/NormalizedWindowRecord.cs:12-33`; `packages/dotnet/src/Spanfold/Comparison/Alignment/AlignedComparison.cs:12-14`; `packages/dotnet/src/Spanfold/Comparison/Alignment/AlignedSegment.cs:17-38`; `packages/dotnet/src/Spanfold/Comparison/ComparisonResult.cs:36-76`.

Internal execution artifacts have public constructors, and `PreparedComparison.Align()` will execute over any caller-forged combination of plan, ranges, sides, record IDs, and segments. `ComparisonResult` exposes a 19-parameter public constructor that can create contradictory rows, summaries, finalities, and artifacts.

**Failure/impact:** invalid states become part of the supported API, forcing defensive checks everywhere or exposing internal exceptions to consumers.  
**Smallest correction:** make construction internal and expose read-only views/factories for the states consumers genuinely need. Keep test fixture construction at the `WindowHistory` boundary.  
**Confidence:** High.

---

## 3. Performance and scalability findings

### NET-PERF-001 — P1 High — Alignment is quadratic inside each scope

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Alignment/ComparisonAligner.cs:46-120`.

For `m` windows in one scope, alignment collects and sorts up to `2m` boundaries, then scans all `m` windows for every adjacent boundary interval. The dominant work is O(m²), with new target/against lists for every segment.

**Failure/impact:** dense histories for one key or lane degrade sharply even when total repository benchmarks look acceptable.  
**Smallest correction:** implement an endpoint sweep that sorts starts/ends once and maintains active target/against IDs incrementally.  
**Confidence:** High.

### NET-PERF-002 — P1 High — Roll-up parent and child state has no eviction

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Runtime/RollUpRuntime.cs:8-17,44-50,271-290`.

Every parent key/segment context ever seen remains in `parents`; every child ever observed remains in `ParentState.Children`, even after becoming inactive. This retention occurs whether or not window history recording is enabled.

**Failure/impact:** long-running pipelines with dynamic keys leak memory, and `TotalCount` gradually becomes “children ever seen,” which can also change semantics.  
**Smallest correction:** define membership lifetime and remove inactive children/fully inactive parent state when no longer needed, or make retention explicit and bounded.  
**Confidence:** High.

### NET-PERF-003 — P1 High — Recorded history and annotations grow without a retention or drain path

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:15-25,127-187,557-607`.

Closed windows and annotations append forever. There is no bounded policy, checkpoint/drain API, external sink, or deletion lifecycle. The public documentation positions Spanfold for monitoring and live analysis, where process lifetimes can be long.

**Failure/impact:** memory use is proportional to all historical activity and eventually dominates the process.  
**Smallest correction:** document finite in-memory replay limits and add an explicit record sink/drain or bounded retention policy. Do not silently evict evidence without a caller-owned policy.  
**Confidence:** High.

### NET-PERF-004 — P1 High — Source matrices rerun the full comparison pipeline for every ordered pair

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:341-407,611-620`.

For `S` sources, non-diagonal cells build and execute S×(S-1) full comparisons. Every run copies/sorts the whole history during preparation. Even the source-presence prepass calls the allocating `Windows` property once per source.

**Failure/impact:** cost approaches O(S² × N log N) plus result allocations and becomes unusable for broad provider matrices.  
**Smallest correction:** prepare/index the requested window history once and compute pairs from a shared aligned representation, or explicitly bound/paginate the matrix API.  
**Confidence:** High.

### NET-PERF-005 — P2 Medium — Direct overlap, residual, and hierarchy analysis repeatedly rescan whole sets

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:478-554,640-784`.

`FindOverlaps` checks every pair. `FindResiduals` scans all closed windows for every target and repeatedly subtracts segments. Hierarchy analysis filters lists per scope and rescans every window for every interval boundary.

**Failure/impact:** the “simple” APIs have quadratic behavior and separate implementations that will drift from the comparison engine.  
**Smallest correction:** group/index once and reuse a shared interval-sweep primitive. If these APIs are redundant convenience aliases, implement them on the same deep module rather than maintaining separate algorithms.  
**Confidence:** High.

### NET-PERF-006 — P2 Medium — Queries and snapshots repeatedly copy and sort the full history

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:28-82`; `packages/dotnet/src/Spanfold/Recording/WindowHistoryQuery.cs:140-196,291-330`; `packages/dotnet/src/Spanfold/Recording/WindowHistorySnapshot.cs:49-61`; `packages/dotnet/src/Spanfold/Recording/WindowSnapshotQuery.cs:140-154`.

History properties allocate full arrays. Queries allocate another filtered list and sorted array. Horizon queries first create and sort a full snapshot, then build another query/dictionary and filtered array. `LatestWindow` still materializes and sorts every match.

**Failure/impact:** read-heavy use pays repeated O(N log N) work and allocation even for a single latest record.  
**Smallest correction:** create one immutable/indexed snapshot per read boundary and query it without repeated full materialization; add direct latest/index lookups where justified.  
**Confidence:** High.

### NET-PERF-007 — P2 Medium — Record IDs rebuild and hash the full record on every property access

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowRecord.cs:52-59`; `packages/dotnet/src/Spanfold/Recording/WindowRecordId.cs:23-43`.

`WindowRecord.Id` is a computed property that rebuilds a string containing the record graph and runs SHA-256 every time. Sorting, dictionaries, preparation, snapshots, diagnostics, and exports access it repeatedly.

**Failure/impact:** identity lookup becomes allocation- and crypto-heavy on every read path.  
**Smallest correction:** after fixing mutability, compute the ID once at construction or lazily cache it.  
**Confidence:** High.

### NET-PERF-008 — P2 Medium — JSON result export looks up row finality in O(R²)

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:604-689,729-742`.

Every exported row calls `GetRowFinality`, which linearly scans all finalities. Exporting R rows performs O(R²) string comparisons.

**Failure/impact:** a reporting-boundary operation can become a major bottleneck or memory-pressure amplifier on large audits.  
**Smallest correction:** build one `(rowType,rowId) -> finality` dictionary before writing rows, or store finality with the row artifact.  
**Confidence:** High.

### NET-PERF-009 — P2 Medium — LLM export duplicates the full result and serializes/reparses every row

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:154-164,203-212,279-304`; `packages/dotnet/src/Spanfold/Comparison/Export/ComparisonExportExtensions.cs:105-123`.

The LLM artifact embeds a full result, full Markdown explanation, and row documents. Each row is first serialized to a `string`, parsed into `JsonDocument`, then written again. The final artifact is also buffered through `MemoryStream.ToArray()`.

**Failure/impact:** peak memory is several multiples of the logical result and large incident bundles can exhaust memory at exactly the workflow boundary meant for large analysis.  
**Smallest correction:** write row documents directly to the existing writer and offer a streaming/file writer that does not construct every duplicate representation in memory.  
**Confidence:** High.

### NET-PERF-010 — P2 Medium — Benchmarks do not exercise the aligner's dense-scope worst case

**Status:** Open  
**Evidence:** `packages/dotnet/benchmarks/Spanfold.Benchmarks/ComparisonBenchmarks.cs:13-19`; `packages/dotnet/benchmarks/Spanfold.Benchmarks/ComparisonBenchmarkData.cs:54-64`; `docs/performance-notes.md:45-60`.

`ComparisonScenario.Large` exists but is omitted from `[Params]`. Existing shapes spread events over many devices/keys, which reduces per-scope `m` and masks the quadratic alignment path. There is no checked-in dense-single-scope scenario or result baseline.

**Failure/impact:** current benchmark coverage cannot support scalability claims or detect the dominant algorithmic regression.  
**Smallest correction:** when replacing the algorithm, add a meaningful dense-scope benchmark at realistic sizes and retain its reproducible command/result baseline.  
**Confidence:** High.

---

## 4. API and domain-design findings

### NET-API-001 — P1 High — “Serializable” selectors are labels, not portable executable plan data

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Selectors/ComparisonSelector.cs:13-28,56-68,75-237,287-328`; `packages/dotnet/src/Spanfold/Comparison/Plans/ComparisonPlan.cs:4-10,85-95`; `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:346-409`.

Executable selector predicates are delegates. Export writes only name, description, serializable flag, and optional cohort labels; it omits selector kind, operands, values, ranges, and `And`/`Or` structure. There is no import API. Worse, `ComparisonSelector.Serializable(name, description)` has no predicate and therefore matches every window because null predicate means true. The default struct has the same match-all behavior with null metadata.

**Failure/impact:** two different plans can export identically, an exported plan cannot be reconstructed, and a nominal descriptor can silently broaden to all data.  
**Smallest correction:** define a discriminated serializable selector AST as the source of truth and compile it to matching; reserve delegate selectors for explicitly runtime-only plans. Until then, remove `Serializable` and portable/persistent claims.  
**Confidence:** High.

### NET-API-002 — P1 High — Four normalization settings are exported but do not affect execution

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Normalization/ComparisonNormalizationPolicy.cs:11-36`; `packages/dotnet/src/Spanfold/Internal/Comparison/Preparation/ComparisonPreparer.cs:129-245`; `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:456-470`; `packages/dotnet/tests/Spanfold.Tests/Comparison/ComparisonNormalizationPolicyTests.cs:57-70`.

`RequireClosedWindows`, `UseHalfOpenRanges`, `CoalesceAdjacentWindows`, and `DuplicateWindowPolicy` are built, documented, and exported but never drive normalization. Public construction can even say `RequireClosedWindows=true` while `OpenWindowPolicy=ClipToHorizon`, and execution follows only the latter. Tests assert stored values, not behavior.

**Failure/impact:** consumers believe they selected policies that the engine ignores.  
**Smallest correction:** remove redundant/unsupported settings before release. Implement only policies with real required behavior; half-open ranges are already a fixed invariant and should not be a boolean.  
**Confidence:** High.

### NET-API-003 — P1 High — Segment, tag, and boundary evidence is lost from core result/export artifacts

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Alignment/AlignedSegment.cs:10-29`; `packages/dotnet/src/Spanfold/Comparison/Rows/OverlapRow.cs:6-18`; `packages/dotnet/src/Spanfold/Comparison/Rows/ResidualRow.cs:6-16`; `packages/dotnet/src/Spanfold/Comparison/Rows/GapRow.cs:11-19`; `packages/dotnet/src/Spanfold/Comparison/Rows/LeadLagRow.cs:11-37`; `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:285-290,502-583,894-907`.

Aligned segments carry segment context, but comparator rows do not. JSON `WriteAligned` omits `segment.Segments`; `WriteWindow` omits segments, tags, boundary reason, and boundary changes. The LLM artifact nevertheless instructs consumers to treat `fullResult` as the source of truth for exact segments and tags.

**Failure/impact:** segmented rows with the same window/key/partition/range are indistinguishable, and exported evidence cannot explain or reconstruct the analysis that produced it.  
**Smallest correction:** carry structural segment context on rows or a stable segment-scope ID, and serialize all domain evidence that the artifact claims to contain.  
**Confidence:** High.

### NET-API-004 — P1 High — JSON and JSONL expose incompatible row identities and JSONL drops finality

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:43-146,254-276,336-344,604-689`; `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:912-933`; `packages/dotnet/src/Spanfold.Testing/SpanfoldAssert.cs:95-109`.

Full JSON/finality uses `symmetricDifference`, `leadLag`, and `asOf`; JSONL uses `symmetric-difference`, `lead-lag`, and `asof`. JSONL row envelopes contain no finality and its summary contains only counts, so a `lead-lag[0]` row cannot join to `leadLag[0]` metadata. Testing assertions add a third stringly public spelling surface.

**Failure/impact:** consumers cannot process the advertised export formats through one contract or recover live finality from JSONL.  
**Smallest correction:** define one typed/canonical wire row-family vocabulary and include row finality/version on every row document.  
**Confidence:** High.

### NET-API-005 — P1 High — “Immutable” records expose mutable arrays directly

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/ComparisonResult.cs:3-10,200-207`; `packages/dotnet/src/Spanfold/Recording/WindowRecord.cs:32-50,66-73`; `packages/dotnet/src/Spanfold/Comparison/Alignment/AlignedSegment.cs:31-38`; `packages/dotnet/src/Spanfold/Comparison/Preparation/NormalizedWindowRecord.cs:25-32`; `packages/dotnet/src/Spanfold/Comparison/Plans/ComparisonScope.cs:93-100`; `packages/dotnet/src/Spanfold/Comparison/Selectors/ComparisonSelector.cs:143-177`.

Materializers return an incoming array unchanged. Internal results also expose arrays behind `IReadOnlyList<T>`, which callers can cast back and mutate. Mutating a record's tags/segments can change its computed ID after it has been used as a dictionary key; mutating result rows invalidates finality and changelog metadata.

**Failure/impact:** snapshots are not immutable and identity can change during their lifetime.  
**Smallest correction:** clone inputs and expose `ImmutableArray<T>` or a non-castable read-only wrapper. Do not retain caller-owned arrays.  
**Confidence:** High.

### NET-API-006 — P1 High — Timestamp clock identity cannot be attached to recorded windows

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Temporal/TemporalPoint.cs:33-41,83-89,195-210`; `packages/dotnet/src/Spanfold/Builders/EventPipelineBuilder.cs:220-231`; `packages/dotnet/src/Spanfold/Internal/Comparison/Preparation/ComparisonPreparer.cs:166-205`; `packages/dotnet/src/Spanfold/Recording/WindowHistorySnapshot.cs:98-133`.

Clocked timestamp points are public and incompatible clocks are rejected, but event recording stores only `DateTimeOffset`. Preparation and snapshots always create unclocked points. A clocked evaluation horizon is therefore incompatible with every normally recorded timestamp window and throws during comparison.

**Failure/impact:** the advertised clock-safety model cannot be used end to end through the main API.  
**Smallest correction:** either record clock identity with event time and propagate it into window records, or remove clocked public points until the model is complete.  
**Confidence:** High.

### NET-API-007 — P1 High — Public window/domain constructors admit contradictory and invalid records

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowRecord.cs:18-30`; `packages/dotnet/src/Spanfold/Recording/ClosedWindow.cs:23-50`; `packages/dotnet/src/Spanfold/Windows/WindowSegment.cs:15-18`; `packages/dotnet/src/Spanfold/Windows/WindowTag.cs:13-15`; `packages/dotnet/src/Spanfold.Testing/WindowHistoryFixtureBuilder.cs:31-50`; `packages/dotnet/src/Spanfold/Pipeline/IngestionResult.cs:8-18`.

Callers can create end-before-start windows, records marked closed by position while carrying a start timestamp but no end timestamp, end timestamps before starts, blank/null names through null suppression, null keys, and mutable/null ingestion result collections. Core comparison may later throw from `TemporalRange.Closed`, while timestamp snapshots can misclassify malformed closed windows as provisional.

**Failure/impact:** illegal states cross the public boundary and fail far from their source with inconsistent error models.  
**Smallest correction:** validate invariants in constructors/factories used by core, CLI, and testing; use typed creation failures for imported data.  
**Confidence:** High.

### NET-API-008 — P1 High — Hierarchy “lineage” is only temporal co-activity and open windows disappear

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:410-471,640-784`; `packages/dotnet/tests/Spanfold.Tests/Comparison/HierarchyComparisonTests.cs:7-65`.

Hierarchy groups only by source and partition; parent and child keys/segments are intentionally ignored. Any overlapping child in the lane marks any parent as explained. Open windows contribute to scopes but are skipped from boundaries and active IDs without a diagnostic.

**Failure/impact:** unrelated parent/child groups can be labeled `ParentExplained`, while live hierarchy state can return no rows. The method name and `MissingLineage` diagnostic overstate what is actually a co-activity check.  
**Smallest correction:** either rename/document it as temporal co-activity, or require an explicit lineage/correlation projection and a horizon for open windows.  
**Confidence:** High.

### NET-API-009 — P2 Medium — Output options are inert and the fluent builder cannot configure them

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Plans/ComparisonOutputOptions.cs:3-17`; `packages/dotnet/src/Spanfold/Comparison/Builders/WindowComparisonBuilder.cs:21-32,160-170`; `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:227-250,473-583`; `packages/dotnet/src/Spanfold/Internal/Comparison/Explain/ComparisonExplainer.cs:421-437`.

`IncludeAlignedSegments` and `IncludeExplainData` are only described/exported. Runtime always stores aligned/prepared artifacts, JSON always writes them, and explanation always includes them. The builder initializes `output` to default but exposes no configuration method.

**Failure/impact:** dead contract surface complicates compatibility and misleads users about payload control.  
**Smallest correction:** delete the options until there is a real use case, or apply them consistently to result materialization and all export/explain paths and expose one builder method.  
**Confidence:** High.

### NET-API-010 — P2 Medium — Duplicate comparator declarations duplicate rows and summaries

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Builders/ComparisonComparatorBuilder.cs:8-169`; `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:35-120`; `packages/dotnet/src/Spanfold/Comparison/Plans/ComparisonPlan.cs:173-182`.

The builder and direct plan constructor accept duplicates. Runtime executes each declaration and appends to the same row collections. `Overlap().Overlap()` therefore duplicates evidence and can double-count downstream aggregation.

**Failure/impact:** a configuration typo changes analytical totals rather than producing a diagnostic.  
**Smallest correction:** reject duplicate canonical declarations during plan validation or define comparator selection as an idempotent set while preserving deterministic order.  
**Confidence:** High.

### NET-API-011 — P2 Medium — Selector overlap duplicates side membership and permits silent self-comparison

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Preparation/ComparisonPreparer.cs:78-95,121-127`; `packages/dotnet/src/Spanfold/Comparison/Builders/WindowComparisonBuilder.cs:42-96`; `packages/dotnet/tests/Spanfold.Tests/Api/ReadmeExampleTests.cs:31-58`.

The same record is added once for every matching against selector and can also be added to target. There is no deduplication or warning. Multiple overlapping against selectors duplicate lineage IDs/transitions; target/against overlap can create guaranteed self-overlap and inflated coverage.

**Failure/impact:** broad or mistyped selectors silently bias results. Some self-comparison may be intentional, so blanket rejection would be wrong.  
**Smallest correction:** deduplicate membership per side and diagnose cross-side overlap, with an explicit opt-in for intentional self/superset comparisons.  
**Confidence:** High on duplication; Medium on desired default for cross-side overlap.

### NET-API-012 — P2 Medium — `Validate()` is not an authoritative preflight

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Plans/ComparisonPlan.cs:97-182`; `packages/dotnet/src/Spanfold/Internal/Comparison/Preparation/ComparisonPreparer.cs:21-47`; `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:35-45`; `docs/comparison-guide.md:276-280`.

Public guidance tells dynamic callers to use `Validate`, but it checks only presence and selector exportability. Unknown comparators, duplicate comparators, mixed axes, incompatible policies, clock issues, and selector overlap are discovered later—or not at all.

**Failure/impact:** tooling can approve and export a plan that execution later rejects or interprets unexpectedly.  
**Smallest correction:** define one authoritative plan validation pipeline used by `Validate`, export, preparation, CLI validation, and execution. Keep data-dependent diagnostics in preparation, but validate every plan-only invariant up front.  
**Confidence:** High.

### NET-API-013 — P2 Medium — Coverage converts exact temporal magnitudes to `double`

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:250-279,1056-1060`; `packages/dotnet/src/Spanfold/Comparison/Rows/CoverageRow.cs:14-22`; `packages/dotnet/src/Spanfold/Comparison/Summaries/CoverageSummary.cs:12-19`.

Position lengths and timestamp ticks are exact `long` values, but coverage rows/summaries store them as `double`. Values above 2^53 lose integer precision before aggregation.

**Failure/impact:** long histories and multi-year tick ranges can report subtly incorrect totals and ratios despite exact source ranges.  
**Smallest correction:** retain exact `long` magnitudes per temporal axis and use `double` or `decimal` only for the final ratio.  
**Confidence:** High.

### NET-API-014 — P2 Medium — Source-matrix input permits duplicate identities and creates ambiguous cells

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Recording/WindowHistory.cs:341-407`; `packages/dotnet/src/Spanfold/Comparison/SourceMatrix/SourceMatrixResult.cs:10-58`.

Sources are materialized but never checked for uniqueness. With `[A, A]`, off-diagonal index pairs have equal target/against identities but `IsDiagonal=false` and run self-comparisons. `TryGetCell(A,A)` returns the first matching cell and cannot address the others.

**Failure/impact:** matrix shape and lookup semantics become contradictory.  
**Smallest correction:** require unique non-null source identities using a declared comparer before building cells.  
**Confidence:** High.

### NET-API-015 — P2 Medium — Cohort evidence uses a lossy, unescaped string protocol

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Runtime/ComparisonRuntime.cs:1143-1164`; `packages/dotnet/src/Spanfold/Comparison/Cohorts/CohortEvidenceMetadataExtensions.cs:39-115`.

Runtime creates `key=value;...;activeSources=a,b` text using source `ToString()`. The typed reader splits on `;`, `=`, and `,` without escaping. Source identities containing those delimiters are truncated or split into false members.

**Failure/impact:** the typed API returns evidence different from the result that produced it.  
**Smallest correction:** store typed/structured cohort metadata and serialize it as normal JSON fields, not a mini-language.  
**Confidence:** High.

---

## 5. Security and resilience findings

### NET-SEC-001 — P1 High — Agent/debug exports have no redaction or sensitivity boundary

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:279-304,382-407,894-963`; `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonDebugHtmlExporter.cs:1079-1092,1189-1196`; `packages/dotnet/src/Spanfold/Comparison/Export/ComparisonExportExtensions.cs:105-175`; `docs/comparison-guide.md:294-340`.

Keys, source/partition identities, selector values, cohort sources, segment values, diagnostics, and extension metadata flow into JSON, HTML, Markdown, and explicitly LLM-oriented artifacts. There is no value projection, allowlist, redaction callback, sensitivity warning, or safe metadata profile.

**Failure/impact:** incident/support workflows can unintentionally send tenant identifiers, customer data, or secret-bearing tags/keys to CI artifacts or external agents.  
**Smallest correction:** add an export policy with value projection/redaction and document artifact sensitivity prominently; provide a conservative agent-export profile.  
**Confidence:** High.

### NET-SEC-002 — P2 Medium — Markdown output permits structure injection and audit spoofing

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Internal/Comparison/Explain/ComparisonExplainer.cs:10-14,67-88,500-505,514-555`.

Plan names, selector descriptions, keys, diagnostics, and metadata are appended directly after Markdown list/header prefixes. Newlines, headings, links, or HTML in user-controlled values are not escaped.

**Failure/impact:** exported reports can be visually restructured to hide or impersonate findings, especially when embedded in issue/agent workflows. This is output-integrity risk, not code execution.  
**Smallest correction:** escape Markdown text or render untrusted values in fenced/encoded scalar blocks.  
**Confidence:** High.

### NET-SEC-003 — P2 Medium — File exports and audit bundles are non-atomic

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Export/ComparisonExportExtensions.cs:137-175`; `packages/dotnet/src/Spanfold.Cli/SpanfoldCli.cs:527-552`.

Single-file exports write directly to the destination. Audit bundles overwrite four artifacts sequentially and write the manifest last in the same existing directory. Failure leaves partial files or a mixture of old and new generations.

**Failure/impact:** CI/support consumers can read a valid-looking manifest beside stale or truncated evidence.  
**Smallest correction:** write temporary files/a temporary bundle directory, then atomically replace the destination or publish the manifest only after atomically finalized artifacts are in place.  
**Confidence:** High.

---

## 6. OSS-readiness findings

### NET-OSS-001 — P1 High — The repository's primary .NET quick start does not compile

**Status:** Open  
**Evidence:** `README.md:22-49,86-102`; `packages/dotnet/src/Spanfold/Builders/EventPipelineBuilder.cs:140-170`; `packages/dotnet/src/Spanfold/Comparison/Builders/ComparisonComparatorBuilder.cs:92-121`; `packages/dotnet/tests/Spanfold.Tests/Api/ReadmeExampleTests.cs:5-58`.

The root quick start calls `TrackWindow(..., predicate: ...)`, but the parameter is `isActive`. It passes plural event collections to scalar `Ingest`. A later example calls `LeadLag()` with no required transition/axis/tolerance arguments. The “README” tests exercise different snippets and therefore do not protect the public landing page.

**Failure/impact:** the first consumer experience fails at compile time and undermines every maturity claim.  
**Smallest correction:** fix the root examples and make the actual root/package snippets executable documentation or compile-backed examples.  
**Confidence:** High.

### NET-OSS-002 — P1 High — The security sample's “known-at” audit includes the data it claims is future

**Status:** Open  
**Evidence:** `packages/dotnet/samples/Spanfold.SecurityAccessAudit/Program.cs:18-46,56-68,73-77`; `packages/dotnet/src/Spanfold/Internal/Comparison/Preparation/ComparisonPreparer.cs:319-323`.

The sample ingests six events, including closures at event-time minutes 12 and 16, before running `KnownAtPosition(8)`. Those closures occurred at processing positions 5 and 6, so the position-8 filter includes them. The printed scenario nevertheless calls position 8 the decision horizon and says later data cannot leak, conflating event-time minutes with ingestion positions.

**Failure/impact:** the security-named showcase teaches a leakage-safe pattern that does not perform the claimed audit.  
**Smallest correction:** run the decision audit at the actual ingestion point before later events arrive, using a live horizon for then-open windows, and label processing position separately from event time.  
**Confidence:** High.

### NET-OSS-003 — P1 High — CI never proves the NuGet or public API release surface

**Status:** Open  
**Evidence:** `.github/workflows/ci.yml:11-23`; `packages/dotnet/src/Spanfold/Spanfold.csproj:18-34`; `packages/dotnet/src/Spanfold.Testing/Spanfold.Testing.csproj:18-39`; `packages/dotnet/tests/Spanfold.Tests/Setup/ApiFreezeReadinessTests.cs:5-34`; `docs/package-validation.md:1-34`.

.NET CI restores and tests the solution on Ubuntu only. It does not pack either package, inspect package contents, validate the packed dependency graph, enforce API compatibility, run formatting/analyzers, or audit dependencies. `ApiFreezeReadinessTests` checks XML documentation and `IsPackable`; it does not freeze an API. “Consumer” tests use project references, so reflection/version/package defects are invisible.

**Failure/impact:** green CI can still publish a broken or accidentally breaking package.  
**Smallest correction:** add deterministic `dotnet pack` and package-content/dependency checks plus an intentional public API baseline. Add only quality gates the project commits to owning; do not create placeholder smoke tests.  
**Confidence:** High.

### NET-OSS-004 — P2 Medium — The CLI is not distributable although the root calls it part of the .NET package

**Status:** Open  
**Evidence:** `README.md:191-199`; `packages/dotnet/src/Spanfold.Cli/Spanfold.Cli.csproj:3-12`; `docs/fixture-schema.md:82-107`.

`Spanfold.Cli` is `IsPackable=false` and has no .NET tool metadata. Documentation requires a repository checkout and `dotnet run --project`, while the root says the “.NET package has” a CLI.

**Failure/impact:** package consumers cannot install the advertised command.  
**Smallest correction:** either package it as a .NET tool with versioned install/usage docs or label it unambiguously as a repository-only developer utility.  
**Confidence:** High.

### NET-OSS-005 — P2 Medium — Contributor guidance omits the .NET workflow entirely

**Status:** Open  
**Evidence:** `CONTRIBUTING.md:1-14`; `.github/workflows/ci.yml:11-23`.

The contributing guide tells contributors to run only Rust format, Clippy, and tests. It mentions preserving .NET fixtures but gives no restore/test/pack/sample command, SDK expectation, API-change process, or package validation path.

**Failure/impact:** external C# contributors cannot reproduce the repository's expected gates and are likely to learn them through CI failure or review churn.  
**Smallest correction:** document the actual minimal .NET workflow and the deliberate public-API/package rules after those gates exist.  
**Confidence:** High.

### NET-OSS-006 — P2 Medium — The security policy does not define the .NET support boundary or a concrete private contact

**Status:** Open  
**Evidence:** `SECURITY.md:1-9`; `packages/dotnet/src/Spanfold/Spanfold.csproj:4-17`.

The policy names only Rust's experimental status, gives no supported .NET versions/package versions, and says to contact “repository maintainers” without a security advisory link or address.

**Failure/impact:** reporters do not know whether .NET is supported or how to report privately without opening a public issue.  
**Smallest correction:** state the support status/version policy for each shipped package and provide one concrete private reporting channel.  
**Confidence:** High.

### NET-OSS-007 — P2 Medium — Version and schema governance is absent

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Spanfold.csproj:5-17`; `packages/dotnet/src/Spanfold.Testing/Spanfold.Testing.csproj:5-17`; `packages/dotnet/src/Spanfold/Internal/Comparison/Export/ComparisonExporter.cs:11-15`; `packages/dotnet/src/Spanfold.Cli/SpanfoldCli.cs:557-564`; `.github/workflows/ci.yml:1-41`.

Package versions and release notes are duplicated in project files. Public export and audit schemas are hard-coded at version 0 with no compatibility/migration policy or reader. There is no changelog or automated release path describing how core/testing/tool versions move together.

**Failure/impact:** consumers cannot reason about compatibility, and maintainers have no enforced way to prevent package/schema drift.  
**Smallest correction:** centralize version/repository metadata, define preview schema compatibility rules, add a changelog/release process, and version related artifacts intentionally.  
**Confidence:** High.

### NET-OSS-008 — P3 Low — SDK and dependency resolution are not reproducible enough for a showcase repository

**Status:** Open  
**Evidence:** `packages/dotnet/Directory.Build.props:1-9`; `.github/workflows/ci.yml:18-23`; `packages/dotnet/src/Spanfold/Spanfold.csproj:32-34`; `packages/dotnet/src/Spanfold.Testing/Spanfold.Testing.csproj:34-39`; `packages/dotnet/tests/Spanfold.Tests/Spanfold.Tests.csproj:9-14`; `packages/dotnet/benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj:9-11`.

CI floats `10.0.x`; local contributors have no pinned SDK feature band. Package versions are scattered across projects and restores are unlocked.

**Failure/impact:** builds can change with SDK/dependency resolution independently of a source revision, and updates are harder to review centrally.  
**Smallest correction:** pin the intended SDK policy with `global.json` and centralize dependency versions. Add lock files only if the project is prepared to maintain them consistently.  
**Confidence:** High.

### NET-OSS-009 — P3 Low — Package validation documentation relies on a temporary consumer smoke test

**Status:** Open  
**Evidence:** `docs/package-validation.md:24-30`.

The documented process asks maintainers to create a temporary console app and run a consumer smoke build. That is manual, non-repeatable release ceremony and conflicts with this repository's explicit direction to avoid smoke tests and temporary validation utilities.

**Failure/impact:** package assurance depends on an ad hoc step that CI does not enforce and maintainers will eventually skip.  
**Smallest correction:** remove the temporary-app procedure and make the real package contract—contents, dependency metadata, symbols/SourceLink, and API compatibility—a deterministic CI/release gate.  
**Confidence:** High.

---

## 7. Minor cleanup and nitpicks

### NET-MIN-001 — P3 Low — Benchmark smoke tests add shallow coupling and production hooks

**Status:** Open  
**Evidence:** `packages/dotnet/tests/Spanfold.Tests/Comparison/BenchmarkSmokeTests.cs:5-59`; `packages/dotnet/benchmarks/Spanfold.Benchmarks/ComparisonBenchmarks.cs:127-130`; `packages/dotnet/benchmarks/Spanfold.Benchmarks/SegmentCohortBenchmarks.cs:76-79`.

Tests instantiate BenchmarkDotNet classes, assert only non-empty outputs, and require public `GetDataForSmokeTest` methods in benchmark code. They do not protect a distinct business invariant and are exactly the kind of smoke coverage the repository asks not to create.

**Smallest correction:** remove the smoke tests and their public hooks. Keep only targeted correctness regressions and real benchmark scenarios.  
**Confidence:** High.

### NET-MIN-002 — P3 Low — Snapshot normalization can hide unrelated 64-character hexadecimal changes

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold.Testing/SpanfoldSnapshot.cs:10-29,50-65,87-88`.

Default normalization rewrites every 64-character lowercase hex token, not only values in record-ID fields. A consumer key, content hash, signature, or unrelated digest can change without failing a snapshot.

**Smallest correction:** normalize known JSON/property locations or explicit record-ID tokens, and make broad textual normalization opt-in.  
**Confidence:** High.

### NET-MIN-003 — P3 Low — CLI silently ignores unknown/misspelled options

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold.Cli/SpanfoldCli.cs:91-154,279-289`.

Option readers scan only names they recognize and never reject leftover arguments. A misspelling such as `--fromat` silently falls back to JSON; a missing value can be consumed as the next option token.

**Smallest correction:** parse arguments once, reject unknown/duplicate/missing-value options with contextual errors, and provide normal `--help`/`--version` handling if the CLI becomes distributable.  
**Confidence:** High.

### NET-MIN-004 — P3 Low — Sample narratives do not match the states they produce

**Status:** Open  
**Evidence:** `packages/dotnet/samples/Spanfold.IndustrialTelemetry/Program.cs:24-43,54-61,69-75,81-95`; `packages/dotnet/samples/README.md:27-33`; `packages/dotnet/samples/Spanfold.SpaceMissionResearch/Program.cs:48-63`.

The industrial sample observes both sensors at minute 0 and checks both at minute 2 with a 45-second threshold, so both become silent; only sensor B recovers. The diagram says only sensor B goes silent, and the output queries only closed silence windows, hiding sensor A's still-open silence. The samples README also says the space-mission sample combines cohorts, but that program uses ordinary target/against comparison and roll-up counts, not a comparison cohort.

**Smallest correction:** align the event sequences, printed diagrams, queries, and samples index so each example demonstrates exactly the behavior it names.  
**Confidence:** High.

### NET-MIN-005 — Nit — Repository setup test is tautological

**Status:** Open  
**Evidence:** `packages/dotnet/tests/Spanfold.Tests/Setup/RepositorySetupTests.cs:3-9`.

`Assert.True(true)` proves nothing and creates false confidence.  
**Smallest correction:** delete the test.  
**Confidence:** High.

### NET-MIN-006 — Nit — Cohort required-count logic contains a dead conditional

**Status:** Open  
**Evidence:** `packages/dotnet/src/Spanfold/Comparison/Cohorts/CohortActivity.cs:104-115`.

`"any" => memberCount == 0 ? 1 : 1` has identical branches.  
**Smallest correction:** return `1` directly after cohort non-empty validation is centralized.  
**Confidence:** High.

---

## Five highest-value improvements

1. **Make identity structural and end to end.** Carry the configured key comparer/stable occurrence identity through runtime, roll-ups, recording, queries, comparison, and export. This resolves the comparer-close bug and removes the formatted-string collision family.
2. **Repair roll-up and ingestion state transitions.** Model child membership/parent moves explicitly and stage fallible event work before committing state. This removes the most credible silent runtime corruption.
3. **Fix live temporal contracts.** Correct known-at reconstruction, semantic row IDs, gap dependencies, and changelog replay before calling live output auditable or leakage-safe.
4. **Make the comparison contract honest.** Introduce a real selector AST, remove inert normalization/output flags and fake extensions, preserve segment context, and unify JSON/JSONL row vocabulary.
5. **Replace whole-set rescans and unbounded retention.** Implement an endpoint sweep, shared indexed snapshots, explicit retention/drain behavior, and a source-matrix algorithm that prepares once.

## Systemic patterns

- **Display text is repeatedly promoted to identity.** `ToString()` and delimiter strings control equality, IDs, sorting, metadata, and exports.
- **Behavior is modeled twice.** Public flags/descriptors claim behavior while execution follows a different smaller set of fields and hard-coded branches.
- **Read-only interfaces are mistaken for immutability.** Arrays remain mutable and public constructors allow invalid stage graphs.
- **Current list position is treated as durable identity.** This is fundamentally incompatible with live/revised analytical output.
- **Whole-history materialization is the default seam.** Queries, snapshots, comparisons, matrices, and exports repeatedly copy or rescan everything.
- **Documentation and artifact vocabulary are ahead of implementation.** Portable plans, full-result exports, CLI packaging, live finality, and leakage safety are all stated more strongly than the current end-to-end behavior warrants.
- **Several tests assert shape/configuration rather than the dangerous transition.** Examples include custom comparer closure, inert policies, changelog direction, and benchmark smoke paths.

## Abstractions to remove, collapse, introduce, or deepen

### Remove or collapse now

- Remove/internalize `ComparisonExtension*` until extensions can execute.
- Remove inert normalization/output fields rather than preserving speculative compatibility in a preview API.
- Internalize public constructors for `PreparedComparison`, `AlignedComparison`, normalized records, and `ComparisonResult`.
- Collapse string row-family spellings into one typed internal/wire vocabulary.
- Remove benchmark/test smoke hooks and the tautological setup test.

### Introduce or deepen where justified

- A structural logical-identity/segment-key component, separate from display formatting.
- Stable runtime occurrence and semantic comparison-row identities.
- A discriminated selector/comparator plan representation with one authoritative validator.
- A single interval-sweep module reused by alignment, residual/overlap, hierarchy, and matrices.
- An immutable/indexed history snapshot plus an explicit history sink/retention seam.
- An export policy that owns redaction, domain-value encoding, and canonical schema names.

Do not introduce a forest of public interfaces. Most of these can be deep internal modules behind the existing fluent façade.

## Proposed restructuring sequence

1. Correct release/docs language to “experimental preview” and avoid publishing new compatibility commitments.
2. Introduce stable occurrence/structural keys internally, then repair history close and roll-up child/parent transitions.
3. Stage ingestion observations and define callback/concurrency semantics before changing more runtime behavior.
4. Repair known-at, gap finality, semantic row IDs, and changelog state representation as one live-output slice.
5. Replace selector delegates-as-data with a portable AST; remove inert options and extension descriptors at the same compatibility boundary.
6. Carry segment scope through comparator rows and unify all export schemas/vocabulary, including redaction.
7. Replace alignment with a sweep, then reuse it to remove direct-analysis and matrix rescans; add retention/indexing after identity is stable.
8. Narrow public constructors/types, replace testing reflection with a supported fixture factory, and establish a real API baseline.
9. Finish package/CLI/release CI and make the actual README snippets part of the maintained examples.

This order avoids optimizing or freezing contracts that are currently semantically wrong.

## Suspicious areas requiring more evidence before changing

- Whether concurrent ingestion is a required feature or a single-writer contract is acceptable. The current absence of either is not acceptable, but the correct implementation depends on intended ownership.
- Whether roll-up child membership means current membership, ever-seen membership, or caller-managed membership. Existing names/docs imply current known children, but retention semantics are not explicit.
- Which object types are intended to be portable keys/sources/segments and whether callers can supply a canonical codec.
- Whether hierarchy analysis is intended as true lineage or only co-activity. The current API language says lineage; the implementation cannot prove it.
- Whether clock identity should be a real recorded domain dimension or removed from the preview API.
- Expected history sizes, source counts, and live process lifetimes. The algorithms are objectively unbounded/quadratic, but target thresholds need real workload evidence.
- Whether the CLI is intended for public installation. Packaging work should not be done if it is intentionally repository-only.

## Candid overall assessment

The C# surface is not currently a defensible showcase of principal-level .NET engineering. The code is generally navigable, but navigability is not the release bar. There are too many cases where the public contract says “deterministic,” “portable,” “immutable,” “known-at safe,” or “final” while the runtime does something materially weaker.

The repository can become showcase-quality without a ceremonial architecture rewrite. The path is to reduce the public promise set, deepen a few core modules—identity, state transition, temporal comparison, interval sweep, and export—and make package evidence match the claims. Until the P1 identity/state/temporal findings are closed, the honest label is **experimental preview, not OSS-ready analytics infrastructure**.
