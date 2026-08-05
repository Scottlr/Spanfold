# Rust Quality Audit

> **Historical audit snapshot.** The findings below describe revision
> `a1e9c08`; they are not the current status of `main`. Keep this catalog as
> review evidence and use subsequent repository history and release gates for
> current disposition.

**Audit date:** 2026-07-11  
**Audited revision:** `a1e9c08` (`main`)  
**Scope:** `packages/rust`, its public Rust API, CLI, benchmarks, specifications, package metadata, and Rust CI gates  
**Disposition:** **Not ready for an OSS release and not defensible as a production/high-throughput implementation yet.**

This is intentionally a hostile review. Passing formatting, default Clippy, and the current tests is treated as the floor, not evidence that the implementation is correct. The review assumes public inputs can be adversarial, timestamps can approach integer limits, clocks can differ, plans can be constructed without the fluent builder, schemas will become compatibility contracts, and workloads will be large enough to expose algorithmic mistakes.

## Bottom line

The crate has a good skeleton: Rust 2024, a pinned toolchain, `#![forbid(unsafe_code)]`, a small core dependency set, typed comparator variants, deterministic ordering intent, a streaming JSONL writer, and HTML escaping. The normal checks pass, and the code is readable locally.

That is not enough. The current implementation has release-blocking defects in plan validation, temporal semantics, state mutation, changelog replay, row identity, portable-plan serialization, known-at handling, and interval complexity. Several public options are exported but ignored. The advertised high-throughput path performs whole-definition-tree clones per event and rescans every interval for every endpoint. The CLI violates its own streaming and exit-code specifications. The passing tests do not exercise the failure modes that matter most.

The right quality label today is **experimental preview**. Calling it production, high-throughput, conformance-passing, or OSS-ready would overstate what the repository proves.

## Evidence snapshot

- Approximately 14,539 lines of Rust across the two crates, with 4,765 lines in `comparison.rs`, 2,123 in `pipeline.rs`, 1,646 in `records.rs`, and 1,223 in the CLI `main.rs`.
- 487 public item/field declarations were found in the Rust sources. The API is broad and representation-heavy.
- `cargo fmt --check`, default Clippy with warnings denied, and the current test suite pass. The suite contains 82 source-level `#[test]` functions; the earlier all-target run reported 89 executed tests including target-specific tests.
- A pedantic Clippy pass reports roughly 95 core-library warnings plus 3 CLI warnings before duplicated test-target diagnostics. The largest class is missing `# Errors` documentation, followed by avoidable formatting allocations, redundant closures, pass-by-value opportunities, precision-loss casts, and undocumented panic contracts.
- There are no doctests exercising the README examples, no property tests, no fuzz targets, no cross-language golden-output job, and no benchmark evidence at the 100k/1m scales required by the checked-in specifications.
- All findings below are open unless explicitly marked as a strength. Severity means:
  - **P0 / release blocker:** do not publish or claim production quality while open.
  - **P1 / critical:** credible silent corruption, invalid contract, panic from public input, or dominant hot-path failure.
  - **P2 / high:** substantial reliability, performance, API, or maintainability defect.
  - **P3 / medium:** real quality debt that should be resolved before a stable API.
  - **P4 / low:** polish, hygiene, or future-proofing issue.

## Required release gates

Before reconsidering OSS readiness, at minimum:

1. Make temporal compatibility explicit and impossible to bypass.
2. Validate every plan on every execution/preparation entry point.
3. Replace positional row IDs with semantic identities and repair changelog semantics.
4. Remove or implement every exported behavioral option.
5. Replace interval rescans with an actual endpoint sweep and remove per-event definition cloning.
6. Make imports and audit commands genuinely streaming, with stable typed errors and exit codes.
7. Add executable fixture expectations, cross-language artifact goldens, boundary/property tests, and meaningful performance baselines.
8. Split the god modules and narrow the public API before users depend on it.
9. Correct the README/spec claims and remove stale Python requirements now that Python is out of scope.

---

## A. Correctness, temporal semantics, and data integrity

### RUST-001 — P0 — Execution bypasses structural plan validation

**Evidence:** `crates/spanfold/src/comparison.rs:846-895`, `1656-1668`; `crates/spanfold/src/builders.rs:345-367`.

`ComparisonPlan::validate()` exists, but `compare`, `compare_live`, `prepare`, and the fluent builder's `run` path never merge those diagnostics into execution. A plan with no name, target, against side, scope, or comparator can execute and return `is_valid: true` if runtime diagnostics happen not to fail it.

**Required correction:** call one authoritative validation routine at every execution/preparation boundary, short-circuit comparator work on structural errors, and add regression tests that execute—not merely inspect—each invalid plan shape.

**Resolution (current revision):** `compare`, `compare_live`, `prepare`, and live preparation now run `ComparisonPlan::validate`; error diagnostics stop materialization. Covered by the plan-validation execution tests.

### RUST-002 — P0 — “Serializable” selectors are not portable or reconstructable

**Evidence:** `crates/spanfold/src/comparison.rs:110-223`, `235-429`; `crates/spanfold/src/export.rs:773-850`; README lines 52-71.

The executable selector AST is private, while plan export writes only `name`, `description`, `isSerializable`, and optional cohort labels. It does not serialize selector kind, operands, ranges, source values, or `And`/`Or` structure. `is_serializable` is therefore a manually asserted label, not proof that the behavior can be reconstructed. Two different executable predicates can export identically.

**Required correction:** define a serializable selector AST as the source of truth, compile it to runtime matching, and reserve closure-backed selectors for explicitly non-portable local plans. Add export/import round-trip semantics tests.

**Resolution (current revision):** serializable selector expressions are now exported recursively, while runtime closure selectors remain marked non-portable and are rejected by portable export.

### RUST-003 — P0 — `TemporalPoint` defines meaningless total ordering across axes and clocks

**Evidence:** `crates/spanfold/src/temporal.rs:5-19`.

Derived `Ord` orders points field-by-field. A processing position can be ordered against a timestamp, and timestamp values from unrelated clocks can be ordered by magnitude and then clock string. Callers and internal `BTreeSet`/sort operations can silently treat incomparable points as comparable.

**Required correction:** remove unconditional `Ord`/`PartialOrd` from the public point or make comparison return a typed compatibility error. Use an internal validated key only after axis and clock identity have been established.

**Resolution (current revision):** `TemporalPoint` has no implicit ordering; all comparison call sites use `try_cmp` and handle typed axis/clock incompatibility.

### RUST-004 — P0 — `TemporalRange::new` checks axis but not clock identity

**Evidence:** `crates/spanfold/src/temporal.rs:79-95`.

A timestamp range can start on clock A and end on clock B. Derived point ordering then decides whether it is “valid,” even though its duration has no defined meaning.

**Required correction:** require matching clock identities for timestamp endpoints and add a `ClockMismatch` error carrying both clocks.

**Resolution (current revision):** `TemporalRange::new` rejects timestamp clock mismatches with both endpoint identities preserved in `TemporalRangeError::ClockMismatch`.

### RUST-005 — P0 — Serde breaks temporal round trips and bypasses range invariants

**Evidence:** `crates/spanfold/src/temporal.rs:14-19`, `73-77`.

`TemporalPoint.clock` is serialized but `skip_deserializing`, so round-tripping a clocked timestamp silently removes its clock. `TemporalRange` derives `Deserialize`, allowing JSON to construct mixed-axis, mixed-clock, or end-before-start ranges without calling `new`.

**Required correction:** implement validated custom deserialization for both types. Use an owned/interned clock representation that can actually be read from runtime data.

**Resolution (current revision):** temporal points own optional clock strings and ranges use custom serde validation for axis, clock, ordering, and checked magnitude.

### RUST-006 — P0 — Failed window close mutates state and emits a false transition

**Evidence:** `crates/spanfold/src/pipeline.rs:1197-1236`.

`close_window_state` removes the active state first, constructs a `Closed` emission, and then silently ignores `TemporalRange::new` failure with `if let Ok`. With out-of-order event time, runtime state says closed, recorded history still contains the open window, callbacks receive a closed transition, and no error reaches the caller.

**Required correction:** construct and validate the range before any mutation, commit active/history/emission changes atomically, and return a typed ingestion error.

**Resolution (current revision):** ingestion returns `Result<IngestionResult, IngestionError>`, preflights active temporal ranges and counters, and the backwards-event regression proves position/history remain unchanged on rejection.

### RUST-007 — P0 — Changelog replay fabricates `Final` state

**Evidence:** `crates/spanfold/src/changelog.rs:43-60`, `93-121`.

Every metadata change is emitted as `Revised`; replay converts every `Revised` entry to `Final`, discarding the current row's actual finality and reason. A Final-to-Provisional transition or a reason-only provisional revision replays incorrectly. The sole test covers only Provisional-to-Final, the one direction that masks the defect.

**Required correction:** carry the resulting finality separately from the change kind, preserve the current reason, and test every finality transition plus replay equivalence.

**Resolution (current revision):** changelog entries now carry `ComparisonChangeKind` plus optional resulting finality; replay applies provisional/final state and the current reason without rewriting revisions.

### RUST-008 — P0 — Positional row IDs are not stable identities

**Evidence:** `crates/spanfold/src/comparison.rs:2048-2057`, `2205-2226`; `crates/spanfold/src/export.rs:725-759`.

IDs such as `overlap[0]` depend on current vector position. Adding, removing, or reordering an earlier row changes the identity of all following rows. Changelogs then report unrelated revisions/retractions and external citations no longer identify the same evidence.

**Required correction:** derive a canonical row identity from comparator family, normalized scope, temporal range, and contributing semantic record IDs; use the same identity everywhere.

**Resolution (current revision):** comparator rows use deterministic FNV-1a IDs over row type and serialized semantic row content; finality, JSON, JSONL, and LLM row documents share the helper.

### RUST-009 — P0 — Timestamp known-at filtering excludes every candidate

**Evidence:** `crates/spanfold/src/comparison.rs:2276-2303`, `2661-2665`.

Candidate availability is collapsed to an `i64` called `known_at_position`. If the plan's known-at point is a timestamp, the condition `known_at.axis() != ProcessingPosition` is always true, so every window is excluded regardless of its availability timestamp.

**Required correction:** retain `TemporalPoint` for candidate availability, validate axis/clock compatibility, and compare typed points. Do not substitute start/end magnitudes across different temporal meanings.

**Resolution (current revision):** known-at filtering compares the candidate availability `TemporalPoint` against the configured point with typed ordering and rejects future/incompatible candidates.

### RUST-010 — P0 — Exported behavioral options are ignored by execution

**Evidence:** `crates/spanfold/src/comparison.rs:624-645`, `733-806`; repository search shows runtime reads only for open policy, axis, coalescing, duplicate policy, and known-at. `require_closed_windows`, `use_half_open_ranges`, `include_aligned_segments`, and `include_explain_data` are only built/exported/tested.

These fields advertise behavior that does not exist. A result still includes aligned/prepared artifacts regardless of output preferences, `use_half_open_ranges = false` changes nothing, and `require_closed_windows` can contradict the actual open-window policy.

**Required correction:** either implement each option completely or delete it. Prefer removing redundant booleans when the type already fixes the behavior (all ranges are half-open).

**Resolution (current revision):** closed-window policy and output explain/alignment flags now change normalization/materialized artifacts; unsupported inclusive-range mode is rejected during validation.

### RUST-011 — P1 — Reusable scope silently drops its time axis

**Evidence:** `crates/spanfold/src/builders.rs:189-198`; `crates/spanfold/src/comparison.rs:520-578`.

`ComparisonScope::on_event_time()` sets `scope.time_axis`, but `WindowComparisonBuilder::scope()` copies every other field and omits `time_axis`. The resulting plan remains on processing position.

**Required correction:** copy the axis or consolidate scope/normalization ownership so it cannot drift. Add a test using `ComparisonScope::window(...).on_event_time()`.

**Resolution (current revision):** builder scope construction copies `ComparisonScope.time_axis`; reusable event-time scope tests cover the previously lost axis.

### RUST-012 — P1 — Axis enforcement is asymmetric

**Evidence:** `crates/spanfold/src/comparison.rs:2719-2738`.

Timestamp plans exclude processing-position records, but processing-position plans accept timestamp records. The declared normalization axis therefore does not constrain half of the state space.

**Required correction:** require exact axis equality in both directions, with a clear reject/exclude policy and diagnostic context.

**Resolution (current revision):** normalization checks candidate axis before constructing ranges and emits `MissingEventTime` or `TemporalAxisMismatch` with reject/exclude severity.

### RUST-013 — P1 — Comparison grouping omits timestamp clock identity

**Evidence:** `crates/spanfold/src/comparison.rs:1475-1476`, `2564-2587`.

The grouping key is `(window, key, partition, axis)`. Timestamp records from different clocks enter one group and are aligned, subtracted, and compared together.

**Required correction:** include validated clock identity in the group key or reject mixed-clock input before grouping.

**Resolution (current revision):** aligned grouping keys include optional timestamp clock identity, preventing unrelated clocks from coalescing.

### RUST-014 — P1 — Core result ranges erase axis and clock

**Evidence:** `crates/spanfold/src/comparison.rs:952-959`, `1487-1518`, `1580-1605`.

`RowRange` contains only two `i64`s. `WindowArtifact` names every magnitude `startPosition`, `endPosition`, and `knownAtPosition` even for timestamps. Most comparator rows therefore cannot be interpreted unambiguously from the artifact alone.

**Required correction:** export typed range endpoints (axis, magnitude, clock) and use axis-neutral field names. If a compact form is retained, make the governing temporal domain mandatory and local to each row/group.

**Resolution (current revision):** `WindowArtifact` now carries typed `start`, `end`, and `knownAt` points with axis and clock identity. Scalar `RowRange` endpoints remain outstanding under this finding.

### RUST-015 — P1 — Direct overlap and residual APIs compare unrelated temporal domains

**Evidence:** `crates/spanfold/src/records.rs:473-523`, `1269-1297`.

`find_overlaps` and `find_residuals` compare raw magnitudes without checking axis or clock. A timestamp window can overlap a processing-position window with the same window/key/partition, and residual output labels timestamp magnitudes as positions.

**Required correction:** partition by temporal domain, return typed ranges, and reject incompatible records.

### RUST-016 — P1 — Hierarchy analysis merges axes, clocks, and unrelated ranges

**Evidence:** `crates/spanfold/src/analytics.rs:226-323`.

Hierarchy boundaries are raw magnitudes grouped only by source and partition. Axis and clock are discarded, and there is no explicit lineage relation beyond window-family names plus temporal overlap. Unrelated records can be described as parent-explained evidence.

**Required correction:** require an explicit hierarchy/lineage key and typed temporal domain, then group and sweep within that validated scope.

### RUST-017 — P1 — Liveness accepts mixed clocks and destroys clock identity

**Evidence:** `crates/spanfold/src/liveness.rs:260-342`, `345-349`.

`ensure_axis` checks only the axis. Derived ordering accepts different clocks, and `add_magnitude` recreates timestamp points with `timestamp_ticks`, dropping the original clock.

**Required correction:** enforce clock equality and preserve the clock through checked temporal arithmetic.

### RUST-018 — P1 — Cohort membership math is wrong for duplicates and empty cohorts

**Evidence:** `crates/spanfold/src/comparison.rs:446-507`, `2812-2863`; fixture validation only partially protects one input path.

Active sources are deduplicated, but member count is `sources.len()`. Duplicate declared sources can make `All` permanently false. An empty `All` cohort evaluates true (`0 == 0`), and public plan construction can bypass fixture checks entirely.

**Required correction:** validate non-empty unique source identities and threshold bounds in the domain constructor used by every entry path.

**Resolution (current revision):** plan validation rejects empty/duplicate source IDs, empty cohort names, and invalid cohort count rules before execution.

### RUST-019 — P1 — Public plan fields permit contradictory selector semantics

**Evidence:** `crates/spanfold/src/comparison.rs:759-833`; `crates/spanfold/src/builders.rs:141-180`.

Non-empty `against_selectors` override selection, while `plan.against` still controls cohort alignment and metadata. Callers can select one set of windows but evaluate them using an unrelated cohort's member count/rule. `against_selector` also leaves any prior `against` value in place.

**Required correction:** represent the comparison side as one enum whose selection and aggregation semantics cannot diverge.

### RUST-020 — P1 — Negative tolerances remain constructible

**Evidence:** `crates/spanfold/src/comparison.rs:15-50`, `3073-3137`, `3218-3333`.

String parsing rejects negative tolerances, but the public `Comparator` variants accept any `i64`. Execution then uses `.abs() <= tolerance`, producing nonsensical results without validation.

**Required correction:** introduce a non-negative magnitude newtype or validate all typed comparator values before execution.

**Resolution (current revision):** lead/lag and as-of tolerance magnitudes are rejected when negative during structural validation.

### RUST-021 — P1 — Unchecked arithmetic can panic or wrap

**Evidence:** `crates/spanfold/src/temporal.rs:115-119`; `pipeline.rs:820`, `1255-1258`; `comparison.rs:2939`, `3200-3206`, `.abs()` uses near 3136/3189/3287/3363; `liveness.rs:345-349`; `records.rs:1089-1116`, `1375-1378`; `testing.rs:187-195`; `changelog.rs:53`, `81`.

Durations, deltas, counters, versions, clocks, and positions use unchecked addition/subtraction/absolute value. Debug builds panic; release builds wrap. `i64::MIN.abs()` is especially hazardous.

**Required correction:** centralize checked temporal arithmetic and choose explicit overflow behavior for counters/versions.

**Resolution (current revision):** event positions, record IDs, liveness thresholds, and range magnitudes use checked arithmetic and typed overflow errors.

### RUST-022 — P1 — Invalid deserialized history can panic comparison

**Evidence:** `crates/spanfold/src/comparison.rs:2710-2791`; `crates/spanfold/src/temporal.rs:73-77`.

`normalize_candidate` calls `TemporalRange::new(...).expect(...)`. Because `TemporalRange` can be deserialized without validation, malformed public data can reach this path and panic.

**Required correction:** make invalid ranges unconstructable and still propagate a typed preparation diagnostic instead of relying on `expect`.

**Resolution (current revision):** invalid ranges are rejected by constructors/serde and normalization records `InvalidTemporalRange` diagnostics instead of constructing unchecked values.

### RUST-023 — P1 — User projection configuration can panic during ingestion

**Evidence:** `crates/spanfold/src/pipeline.rs:1345-1384`.

Renaming two selected segments to the same projected name triggers `assert!` in the ingestion path. This is user configuration, not an impossible internal invariant.

**Required correction:** validate projection rules in `try_build`, return a precise build error, and keep runtime projection infallible.

### RUST-024 — P1 — Roll-up segment identity uses an ambiguous string encoding

**Evidence:** `crates/spanfold/src/pipeline.rs:1010-1019`, `1415-1429`.

State identity concatenates raw parent names and segment names with `/`, `=`, and `;`, then embeds `Debug` output for values. Different segment tuples can collide (for example, parent/name splits around `/`), causing unrelated roll-up state to merge.

**Required correction:** use a typed/hashable key over canonical segment tuples; never use presentation strings as identity.

### RUST-025 — P2 — Segment changes are diffed by vector position, not dimension name

**Evidence:** `crates/spanfold/src/pipeline.rs:1431-1452`.

Reordering identical named dimensions emits false changes, and insertion/removal can report a current value under the previous segment's name.

**Required correction:** validate unique segment names and diff canonical name-indexed maps while preserving a deterministic output order.

### RUST-026 — P1 — `WindowHistory` deserialization loses its internal open-window index

**Evidence:** `crates/spanfold/src/records.rs:378-395`, `602-617`.

`open_indexes` is `#[serde(skip)]`, so any deserialized history containing open windows has an empty index. Round-trip equality fails and later close/removal operations cannot find those records.

**Required correction:** custom-deserialize and rebuild/validate the index, including duplicate-ID rejection; alternatively do not deserialize the stateful aggregate.

**Resolution (current revision):** `WindowHistory` custom deserialization rebuilds `open_indexes` and rejects duplicate open IDs.

### RUST-027 — P1 — `PrimitiveValue::Float` admits non-finite values

**Evidence:** `crates/spanfold/src/primitive.rs:3-18`, `44-47`; spec `008` lines 95-111.

`From<f64>` accepts NaN and infinities. NaN is not reflexively equal, breaking filtering/deduplication, and non-finite values do not have a faithful JSON number representation.

**Required correction:** use a finite-float wrapper with fallible construction and define explicit integer/float comparison semantics.

**Resolution (current revision):** `FiniteFloat` keeps its payload private, rejects non-finite construction and deserialization, and preserves the explicit exact-range integer/float equality rule.

### RUST-028 — P2 — Missing and empty source identities collapse internally

**Evidence:** `crates/spanfold/src/comparison.rs:2246-2264`, `2564-2581`; `records.rs:1247-1256` uses a similar sentinel.

`None` becomes `""` in alignment and `"<null>"` in sorting. Those sentinels are also valid user strings. Custom selectors can therefore make absent and explicit values indistinguishable.

**Required correction:** preserve `Option` in typed keys and ordering.

**Resolution (current revision):** optional source identities remain `Option<String>` in grouping and active-source keys; missing source is no longer collapsed into an empty string.

### RUST-029 — P2 — Target and against selectors may select the same record

**Evidence:** `crates/spanfold/src/comparison.rs:2349-2395`.

A record matching both sides is normalized twice and compared with itself. This can manufacture perfect overlap/coverage and conceal a selector mistake.

**Required correction:** define whether self-comparison is legal; default to a diagnostic or rejection unless explicitly requested.

### RUST-030 — P2 — Duplicate comparator declarations duplicate rows and summaries

**Evidence:** `crates/spanfold/src/comparison.rs:1701-1775`.

The comparator vector is executed verbatim. Repeating a comparator appends duplicate evidence, alters positional IDs, and can multiply aggregate magnitudes.

**Required correction:** reject duplicate declarations or make repeated declarations semantically distinct through stable comparator instance IDs.

### RUST-031 — P1 — Plan validation is too shallow even when called

**Evidence:** `crates/spanfold/src/comparison.rs:846-895`.

Validation checks only name, target presence, against presence, window scope, comparator presence, and selector exportability. It does not validate duplicate comparators, negative tolerances, axis/horizon/known-at compatibility, cohort uniqueness/counts, contradictory open-window fields, empty identities, duplicate filters, or target/against overlap.

**Required correction:** make validation exhaustive over the domain model and return structured diagnostics with field paths.

### RUST-032 — P2 — Fixture conflicts are silently resolved by precedence

**Evidence:** `crates/spanfold/src/fixture.rs:184-227`.

If both `againstSources` and `againstCohort` are supplied, cohort silently wins. If both horizon fields are supplied, `liveHorizonPosition` silently wins. Supplying `count` for rules that do not use it is ignored.

**Required correction:** reject mutually exclusive or irrelevant fields instead of inventing precedence.

### RUST-033 — P1 — Contract fixture expectations are parsed as unknown data and ignored

**Evidence:** `crates/spanfold/src/fixture.rs:91-174`; all four .NET fixture files contain `expected`, but `RawFixture` has no such field.

`ContractFixture::execute()` does not assert fixture expectations. Tests manually inspect selected outputs, so adding or changing an expectation does not automatically fail Rust conformance.

**Required correction:** model the expected contract and implement one data-driven conformance runner that checks every field required by spec 006.

### RUST-034 — P2 — Fixture and import schemas silently accept unknown properties

**Evidence:** `crates/spanfold/src/fixture.rs:91-182`; CLI serde input types near `main.rs:962-1098`.

None of the configuration DTOs uses `#[serde(deny_unknown_fields)]`. Typos in plan fields, import selectors, predicate conditions, and window metadata silently disappear.

**Required correction:** deny unknown fields on closed schemas or explicitly capture extensions under a named extension object.

### RUST-035 — P1 — Liveness accepts observations earlier than an already-run check

**Evidence:** `crates/spanfold/src/liveness.rs:260-294`, `297-332`.

Observations are compared with tracker start and the prior lane observation, but not `last_check_at`. After checking at time 200, an observation at 150 can emit a retroactive recovery and later produce another silence transition with contradictory chronology.

**Required correction:** reject observations earlier than the last evaluation horizon or define and implement explicit correction/replay semantics.

**Resolution (current revision):** liveness observations and checks reject backwards points and preserve axis/clock identity through threshold arithmetic.

### RUST-036 — P2 — Annotation targets erase temporal domain

**Evidence:** `crates/spanfold/src/records.rs:322-360`.

`WindowAnnotationTarget` stores `start_position: i64` even when created from a timestamp window. Axis and clock are lost, so otherwise identical processing/timestamp records can share a target.

**Required correction:** store a typed `TemporalPoint` or the stable record ID as the primary target identity.

### RUST-037 — P0 — Alignment is not an endpoint sweep; it rescans every interval

**Evidence:** `crates/spanfold/src/comparison.rs:2812-2875`; specs 018 lines 38-52 and 135-143.

The code collects endpoints, but for every adjacent pair it loops over all target windows, all against windows, and all against windows again for active sources. Complexity is `O(B × (T + A))`; dense overlaps approach quadratic behavior. This directly violates the checked-in performance acceptance gate.

**Required correction:** sort start/end events once and maintain active target/against sets during a single sweep.

### RUST-038 — P1 — Lead/lag and as-of ignore their sorted indexes

**Evidence:** `crates/spanfold/src/comparison.rs:3073-3206`, `3218-3379`.

Candidate vectors are sorted, then every target scans all candidates to find a nearest/directional match. This is `O(T × A)` per group.

**Required correction:** use binary search for independent lookups or a two-pointer scan when target order permits it; preserve deterministic tie-breaking.

### RUST-039 — P0 — Every ingested event clones the entire definition tree

**Evidence:** `crates/spanfold/src/pipeline.rs:51-75`, `813-835`.

`for definition in self.windows.clone()` recursively clones names, roll-up vectors, projection maps/sets, callback vectors, and `Arc`s on every event. This is the dominant shape of the advertised high-throughput hot path.

**Required correction:** separate immutable definitions from mutable runtime state and traverse by stable indexes/references without cloning configuration.

**Resolution (current revision):** event ingestion moves the immutable definition vector out temporarily instead of cloning the entire definition tree for every event.

### RUST-040 — P1 — Every result stores each row twice

**Evidence:** `crates/spanfold/src/comparison.rs:1347-1448`, `1914-1948`.

`ComparisonResult` owns `rows: ComparisonRows` and also clones every family into flat `*_rows` fields. Large results pay roughly double row storage before prepared/aligned/export duplication.

**Required correction:** choose one canonical representation and expose compatibility accessors or custom serialization without cloning.

### RUST-041 — P1 — Prepared/aligned artifacts multiply the full comparison graph

**Evidence:** `crates/spanfold/src/comparison.rs:1560-1616`, `1691`, `1804-1805`, `2533-2561`.

`AlignedComparison` clones `PreparedComparison`, which contains a cloned plan and owned window artifacts. Execution then serializes prepared and aligned again into `serde_json::Value`, while retaining typed rows and duplicated flat rows.

**Required correction:** keep one typed artifact graph with references/indexes internally and serialize only at the requested output boundary.

### RUST-042 — P1 — Query construction clones and sorts the full history repeatedly

**Evidence:** `crates/spanfold/src/records.rs:418-435`, `627-633`, `754-787`.

`WindowHistory::windows()` clones and sorts all records; `query()` feeds that into `WindowHistoryQuery::new`, which sorts again; materialization clones matching records again.

**Required correction:** make queries borrow the history and yield references/iterators. Sort once only when deterministic materialization is requested.

### RUST-043 — P2 — Direct overlap/residual analysis is quadratic and clone-heavy

**Evidence:** `crates/spanfold/src/records.rs:473-523`.

`find_overlaps` checks every pair and clones both full windows for every overlap. `find_residuals` scans every comparison window for every target and repeatedly reallocates segment vectors.

**Required correction:** group/index by scope and sweep intervals; return IDs/references or compact evidence instead of duplicate full records.

### RUST-044 — P1 — Source matrices rerun full comparisons for every cell

**Evidence:** `crates/spanfold/src/analytics.rs:127-224`.

For `S` sources, the implementation performs `S²` presence scans and up to `S²-S` complete prepare/align/materialize cycles over the same history. It does not reuse normalization or indexes.

**Required correction:** prepare once per source or build one source-indexed sweep, then derive matrix cells from shared aligned evidence.

### RUST-045 — P2 — Hierarchy analysis repeatedly filters and rescans windows

**Evidence:** `crates/spanfold/src/analytics.rs:234-323`.

Each scope filters parent/child vectors, collects boundaries, then scans all scoped windows for every segment. This repeats allocation and interval work and compounds the semantic issues in RUST-016.

**Required correction:** pre-group once and use active sets during an endpoint sweep.

### RUST-046 — P0 — `audit-windows` loads the entire JSONL file

**Evidence:** `crates/spanfold-cli/src/main.rs:313-356`; specs 014 lines 12-27 and 019 lines 66-78.

The command uses `fs::read_to_string` and then iterates lines. This violates the explicit production requirement to stream 100k/1m window files and creates an avoidable input-sized allocation.

**Required correction:** use buffered incremental reads with line-aware errors and feed records into a bounded preparation/history pipeline.

### RUST-047 — P2 — Event import retains all emitted windows before writing

**Evidence:** `crates/spanfold-cli/src/main.rs:411-585`, `653-659`.

Raw JSONL is streamed, but every completed window accumulates in a `Vec` and is written only after EOF. Memory therefore grows with output size even for `import-events`, a command whose natural output is streaming JSONL.

**Required correction:** stream closed windows directly to the sink, retaining only active per-key state. Define deterministic final ordering if it differs from event order.

### RUST-048 — P2 — LLM export serializes, reparses, and embeds duplicate documents

**Evidence:** `crates/spanfold/src/export.rs:245-282`.

The exporter creates JSONL strings, reparses every line into `Value`, builds a full result value, builds Markdown, and embeds all three. This creates multiple complete representations of the same rows.

**Required correction:** build row documents directly as values or stream them into the final serializer; document an explicit memory cost/limit.

### RUST-049 — P2 — Export finality lookup is quadratic

**Evidence:** `crates/spanfold/src/export.rs:893-919`.

For every exported row, `build_row_values` linearly searches all finality records. A result with `R` rows does `O(R²)` string comparisons and allocations.

**Required correction:** index finality by typed row ID once, or co-locate finality with the row.

### RUST-050 — P1 — Deduplication/coalescing keys are collision-prone formatted strings

**Evidence:** `crates/spanfold/src/comparison.rs:2423-2517`.

User-controlled selector, window, and key strings are concatenated with `|`, while other components use `Debug`. Distinct tuples can produce the same key, and each candidate allocates a large presentation string. A collision can incorrectly reject or merge windows.

**Required correction:** use typed tuple keys over borrowed/interned components and explicit value ordering/hash semantics.

### RUST-051 — P2 — Hot runtime state uses clone-heavy string tuples and ordered maps

**Evidence:** `crates/spanfold/src/pipeline.rs:15-24`, `349-360`, `865-910`, `1001-1041`.

Each event repeatedly clones window name, key, source, partition, segment context, metadata, and tags into tuple keys and observations. `BTreeMap` pays logarithmic comparisons over long strings even though iteration order is not needed for state lookup.

**Required correction:** assign definition/source/key indexes, use hash maps for mutable lookup where determinism is irrelevant, and clone metadata only when opening/emitting a record.

### RUST-052 — P1 — Roll-up parent state grows without eviction

**Evidence:** `crates/spanfold/src/pipeline.rs:88-126`, `1013-1024`.

Every seen child key remains in `ParentState.children` forever, merely toggled false. High-cardinality streams leak state, and `all_active()` becomes “all children ever observed are active,” which may eventually become impossible.

**Required correction:** define child membership lifetime and remove inactive/tombstoned children when no longer relevant; benchmark cardinality growth.

### RUST-053 — P2 — Deterministic sort keys allocate multiple strings per record

**Evidence:** `crates/spanfold/src/records.rs:1243-1266`; similar owned keys appear throughout comparison preparation.

`window_sort_key` constructs seven owned key components, including fresh strings for optional values and IDs. Sorting invokes this repeatedly, magnifying allocations.

**Required correction:** use `sort_by` over borrowed fields and typed comparisons; keep sentinel-free `Option` ordering.

### RUST-054 — P2 — Audit/export paths materialize redundant large strings

**Evidence:** `crates/spanfold-cli/src/main.rs:252-285`; `crates/spanfold/src/export.rs:137-283`.

The audit bundle creates full JSON, Markdown, LLM JSON, and HTML strings before writing any file. The LLM artifact itself duplicates the full result and row documents. Peak memory can be several times artifact size.

**Required correction:** stream independent artifacts to staged files, materialize only inherently nested formats, and document/guard memory-heavy output.

### RUST-055 — P0 — Benchmarks do not test the promised workload

**Evidence:** `crates/spanfold/benches/spanfold_benchmarks.rs:26-225`; specs 018 lines 113-150 and 019 lines 80-87.

Ingestion tops out at 8,192 events; comparisons use a 1,024-event history; the cohort case uses 2,048 events. There are no 10k/100k/1m window audits, sparse/dense alignment series, high-cardinality imports, 5/10/25-source matrices, query benchmarks, peak-memory measurements, or recorded baselines. The ingestion benchmark also includes fixture construction and formatting allocations without distinguishing engine throughput.

**Required correction:** implement and publish the specified scale matrix, separate microbenchmarks from end-to-end scenarios, and track wall time plus allocations/peak memory before making performance claims.

## B. API depth, idiomatic Rust, and module design

### RUST-056 — P1 — `ComparisonPlan` is a public mutable bag of invalid states

**Evidence:** `crates/spanfold/src/comparison.rs:759-806`.

More than twenty public fields expose representation, duplicate concepts, and mutually inconsistent states. The fluent builder does not protect direct construction, and execution does not validate.

**Required correction:** make fields private, provide validated constructors/builders, and model mutually exclusive choices with enums/newtypes.

### RUST-057 — P1 — Domain records expose mutable representation and unvalidated deserialization

**Evidence:** `crates/spanfold/src/records.rs:28-155`, `275-376`; `liveness.rs:8-68`; `pipeline.rs:250-318`.

Public fields and blanket `Deserialize` allow empty IDs/names, duplicate segment names, invalid known-at domains, inconsistent liveness signals, and malformed record relationships.

**Required correction:** separate wire DTOs from validated domain types, keep invariant-bearing fields private, and convert fallibly at boundaries.

### RUST-058 — P2 — Prepared and aligned states can be forged

**Evidence:** `crates/spanfold/src/comparison.rs:1535-1616`.

All fields of `NormalizedWindowRecord`, `PreparedComparison`, `AlignedSegmentArtifact`, and `AlignedComparison` are public. Callers can construct artifacts that did not pass preparation and feed them to public `align`/explain APIs.

**Required correction:** expose read-only accessors and keep constructors/internal state private, or clearly label pure DTOs and validate before consuming them.

### RUST-059 — P2 — `build()` panics on ordinary user configuration

**Evidence:** `crates/spanfold/src/pipeline.rs:633-641`, `779-787`.

The most prominent builder method calls `try_build().expect(...)`; empty/duplicate names are normal configuration errors. Its rustdoc omits a `# Panics` section, and the README showcases `build()`.

**Required correction:** make the primary method fallible or provide a distinct `build_unchecked`/`expect_valid` escape hatch. Document any retained panic precisely.

### RUST-060 — P1 — Ingestion has no error channel

**Evidence:** `crates/spanfold/src/pipeline.rs:286-293`, `813-843`.

`ingest` returns only `IngestionResult`, forcing range/temporal failures to be ignored or converted to panics. This API design enabled RUST-006.

**Required correction:** return `Result<IngestionResult, IngestionError>` and define atomic mutation guarantees.

### RUST-061 — P2 — Clock identity is restricted to `&'static str`

**Evidence:** `crates/spanfold/src/temporal.rs:15-19`, `43-69`.

Runtime/provider clock IDs commonly come from configuration or input. Requiring a static reference either prevents them, encourages leaking strings, or forces callers to abandon clock identity.

**Required correction:** use an owned or cheaply shared validated clock ID (`Arc<str>`, interned ID, or domain newtype).

### RUST-062 — P2 — Comparator text format depends on `Debug` spellings and returns no parse detail

**Evidence:** `crates/spanfold/src/comparison.rs:52-91`, `1809-1912`.

Parameterized declarations serialize enum variants with `{:?}` and parse exact Rust variant names. Renaming an internal variant changes the wire contract. `parse` returns `Option`, losing which field or value failed.

**Required correction:** implement explicit stable tokens, `Display`, and `FromStr<Err = ComparatorParseError>` with field-level context.

### RUST-063 — P2 — Diagnostics are not actionable

**Evidence:** `crates/spanfold/src/comparison.rs:923-939`; diagnostics are often pushed once or once per record with only code/severity.

There is no message, field path, record ID, selector, source, range, or comparator instance. Repeated codes cannot identify affected input, while deduplicated codes hide multiplicity.

**Required correction:** define a typed diagnostic code plus structured context and a stable human-readable message.

### RUST-064 — P2 — The extension module describes behavior it cannot execute

**Evidence:** `crates/spanfold/src/extensions.rs:1-140`; spec 017 lines 96-106 and 169-178.

Extensions are descriptor and metadata structs only. There is no comparator/selector trait, registry, execution seam, or export-sink integration. Presenting this as an extension API is shallow surface area and invites incompatible expectations.

**Required correction:** either keep it private/document it as reserved metadata or introduce a real, deliberately designed behavioral seam later.

### RUST-065 — P1 — `ComparisonResult` has two divergent public JSON contracts

**Evidence:** `crates/spanfold/src/comparison.rs:1373-1448`; `crates/spanfold/src/export.rs:143-149`, `852-880`.

The type derives `Serialize`, producing `planName`, nested `rows`, and duplicated flat arrays while skipping `plan`. `export_result_json` produces a different object with `plan`, row objects augmented with IDs/finality, no top-level `planName`, and no flat arrays. Both claim the same result schema.

**Required correction:** define one canonical serializer/schema and make all public export paths delegate to it.

### RUST-066 — P1 — Plan export certifies invalid plans with empty diagnostics

**Evidence:** `crates/spanfold/src/export.rs:137-140`, `285-289`, `773-829`.

Export checks only selector portability and hard-codes `"diagnostics": []`. Structurally invalid or contradictory plans are emitted as clean portable artifacts.

**Required correction:** validate before export, serialize the actual validation diagnostics, and reject errors when the artifact contract promises executable plans.

### RUST-067 — P1 — Export silently replaces serialization failures and missing finality

**Evidence:** `crates/spanfold/src/export.rs:893-919`; debug helpers near `558-703`.

Row serialization failure becomes an empty object; a missing finality becomes `Final`; enum serialization failure falls back to the string `Final`; debug serialization failure becomes `null` or empty text. This converts corruption into apparently valid evidence.

**Required correction:** propagate errors for contractual exports. Debug output should visibly report an encoding error, never erase data silently.

### RUST-068 — P2 — Markdown and explanation output permit structure injection

**Evidence:** `crates/spanfold/src/export.rs:291-397`; `crates/spanfold/src/explain.rs:17-380`.

Plan names, keys, window names, reasons, and extension values are inserted into Markdown headings/list items without escaping. User data containing newlines or Markdown can spoof sections and evidence.

**Required correction:** escape Markdown text or render user values in fenced/JSON blocks with a documented encoding.

### RUST-069 — P2 — Result explanations omit most comparator evidence

**Evidence:** `crates/spanfold/src/explain.rs:159-240`.

The result explanation gives counts for some families but emits detailed rows only for overlap and residual. Missing, coverage, gap, symmetric difference, containment, lead/lag, and as-of evidence is absent, despite the generic `ComparisonResult::explain` name.

**Required correction:** cover every enabled comparator or rename/narrow the API. Avoid `Debug` as the output schema.

### RUST-070 — P1 — Library file exports are non-atomic

**Evidence:** `crates/spanfold/src/export.rs:716-723`.

`fs::write` truncates the destination in place. Interruption, disk-full, or serialization/write failure can leave a corrupt artifact at the final path.

**Required correction:** write and flush a sibling temporary file, preserve permissions as needed, then atomically rename.

### RUST-071 — P2 — Multi-export execution has partial side effects

**Evidence:** `crates/spanfold/src/builders.rs:369-439`.

Debug HTML is written before LLM context. If the second export fails, the first artifact remains, and there is no manifest or transaction telling consumers the set is incomplete.

**Required correction:** stage all requested outputs and commit them together, or return an explicit partial-output report.

### RUST-072 — P1 — Core modules are god files with mixed responsibilities

**Evidence:** `comparison.rs` 4,765 lines, `pipeline.rs` 2,123, `records.rs` 1,646, `export.rs` 1,125.

Plan modeling, parsing, validation, preparation, alignment, nine comparators, materialization, finality, diagnostics, and tests share one file. The seams proposed by spec 017 are absent. Changes require navigating and recompiling broad modules and make invariants hard to locate.

**Required correction:** split by deep responsibilities (`plan`, `prepare`, `align`, comparator families, rows/finality, diagnostics, wire export) while keeping a small public facade.

### RUST-073 — P2 — The public API is one flat re-export namespace

**Evidence:** `crates/spanfold/src/lib.rs:10-73`.

All implementation modules are private and dozens of unrelated types/functions are re-exported at the root. This hides conceptual ownership, increases name pressure, and makes API evolution/coherent documentation difficult.

**Required correction:** expose deliberate public modules and, if useful, a small prelude; re-export only the principal entry points.

### RUST-074 — P3 — Several APIs are shallow aliases or forwarding wrappers

**Evidence:** query aliases in `records.rs:635-799`; zero-sized `ComparisonSelectorBuilder` in `builders.rs:9-75`; testing aliases in `testing.rs:10-14`.

Pairs such as `where_window/window`, `where_source/source/where_lane/lane`, `windows/all`, static constructors plus a zero-state forwarding builder, and cross-language type aliases double the surface without adding leverage.

**Required correction:** choose one idiomatic vocabulary and remove compatibility aliases before stabilization.

### RUST-075 — P3 — Public types are not evolution-friendly

**Evidence:** widespread public enums and field-bearing structs across `comparison.rs`, `records.rs`, `analytics.rs`, and `pipeline.rs` lack `#[non_exhaustive]` and use public fields.

Adding variants or fields becomes a downstream source break. Version `0.1.0` permits churn, but this is exactly when the boundary should be narrowed.

**Required correction:** make fields private and selectively use `#[non_exhaustive]` on externally matched types whose evolution is expected.

## C. CLI, I/O, and operational behavior

### RUST-076 — P1 — The CLI is a 1,223-line monolith

**Evidence:** `crates/spanfold-cli/src/main.rs`.

Argument definitions, command dispatch, fixture loading, comparison construction, JSONL/CSV parsing, import state, predicate evaluation, bundle writing, and tests all live in `main.rs`. There is no library layer for command logic or focused integration boundary.

**Required correction:** split command modules, input adapters, typed errors, import engine, and artifact sinks; keep `main` to parsing and exit mapping.

### RUST-077 — P1 — CLI errors are stringly and lose source/context

**Evidence:** `crates/spanfold-cli/src/main.rs:142-285` and repeated `Result<_, String>`/`error.to_string()` throughout.

IO, parse, validation, comparison, and export failures collapse into strings, often without file/line/operation context or an error source chain. The JSON stderr envelope has only one opaque message.

**Required correction:** use a typed command error enum with sources, paths, line/column/field context, stable machine code, and user-facing rendering.

### RUST-078 — P1 — The documented exit-code contract is not implemented

**Evidence:** `crates/spanfold-cli/src/main.rs:128-139`; spec 014 lines 94-103.

Every error returned by `run` exits with 2. The specification reserves 2 for usage/input, 3 for IO, and 4 for export. CI cannot distinguish retryable filesystem failures from malformed input.

**Required correction:** map typed errors to the documented stable codes and test each command/error class.

### RUST-079 — P2 — `explain` returns success for an invalid comparison

**Evidence:** `crates/spanfold-cli/src/main.rs:180-184`.

Unlike validate, compare, and audit, explain always returns exit 0 after executing the fixture, even if `result.is_valid` is false.

**Required correction:** apply the same invalid-result exit contract across commands.

### RUST-080 — P2 — CLI paths are UTF-8 strings and are joined manually

**Evidence:** command arguments and helpers throughout `main.rs`, especially `252-284`.

Using `String`/`&str` rejects valid non-UTF-8 Unix paths. `format!("{out}/file")` is platform-naive and obscures path semantics.

**Required correction:** use `PathBuf` in clap arguments and `Path::join` internally; include paths losslessly in diagnostics.

### RUST-081 — P1 — The custom CSV parser is not a CSV implementation

**Evidence:** `crates/spanfold-cli/src/main.rs:453-505`, `911-939`.

It reads physical lines, so quoted fields cannot contain newlines; quote characters toggle state even in illegal positions; dialect/terminator/BOM handling is absent; errors have no column. This is fragile for user data and duplicates a mature ecosystem capability.

**Required correction:** use the maintained `csv` crate with explicit reader configuration and record-level context.

### RUST-082 — P1 — CSV quoted values are silently type-corrupted

**Evidence:** `crates/spanfold-cli/src/main.rs:911-960`.

Parsing removes quote information before coercion. A quoted `"00123"`, `"true"`, or empty string becomes integer 123, boolean true, or null. Explicit CSV quoting cannot preserve string identity.

**Required correction:** define schema-driven coercion or preserve quotedness; never infer away leading zeros/explicit string intent.

### RUST-083 — P2 — Duplicate and empty CSV headers are accepted

**Evidence:** `crates/spanfold-cli/src/main.rs:453-492`.

Rows are inserted into a JSON map by header. Duplicate headers silently overwrite earlier columns, and empty names are accepted.

**Required correction:** validate non-empty unique headers before processing records.

### RUST-084 — P2 — Import-map typos and contradictory predicates are not validated up front

**Evidence:** `crates/spanfold-cli/src/main.rs:662-671`, `797-855`, `1036-1098`.

Unknown fields are ignored; window names/selector fields can be empty; multiple contradictory conditions are accepted and ANDed; `isTrue` and `isFalse` can both be set; predicate shape errors appear only when an event is processed.

**Required correction:** validate the full map once into a typed executable plan with mutually exclusive/explicit predicate composition.

### RUST-085 — P1 — Numeric predicates lose `i64` precision

**Evidence:** `crates/spanfold-cli/src/main.rs:865-879`.

All integers are converted to `f64`; values above `2^53` can compare equal or order incorrectly. Processing positions are `i64`, so this is not theoretical at the type boundary.

**Required correction:** compare integer/integer exactly, handle mixed integer/float with checked semantics, and reject non-finite thresholds.

### RUST-086 — P3 — Dot-separated field selectors cannot address all JSON data

**Evidence:** `crates/spanfold-cli/src/main.rs:770-795`.

Keys containing dots are unaddressable, arrays are unsupported, and empty path components have accidental meaning. The map format does not state these limitations.

**Required correction:** adopt JSON Pointer or a typed path grammar with escaping and validation.

### RUST-087 — P1 — Audit bundles can be mixed-generation or half-written

**Evidence:** `crates/spanfold-cli/src/main.rs:252-285`.

Files are overwritten sequentially in the final directory, and the manifest is last. Failure can leave a mix of old and new artifacts with no incomplete marker. Existing directories are reused without overwrite policy.

**Required correction:** stage in a temporary directory, fsync where appropriate, then atomically replace; reject or explicitly allow overwrite.

### RUST-088 — P2 — The CLI manifest is not crates.io-ready

**Evidence:** `crates/spanfold-cli/Cargo.toml`.

The path dependency is `spanfold = { path = "../spanfold" }` with no version. Published packages cannot resolve an unpublished path-only dependency as a normal registry dependency.

**Required correction:** declare both `path` and matching `version`, then make package verification a release gate.

### RUST-089 — P2 — CLI help and product labeling contradict implementation status

**Evidence:** `crates/spanfold-cli/src/main.rs:20`, command docs at `87-100`; CLI package description; specs 014/019.

Source rustdoc calls the binary “Production high-throughput,” while the manifest says “Preview” and release gates are unmet. Import commands say JSONL although CSV is supported. Stable exit/error behavior and stdin/stdout support are absent.

**Required correction:** label the product experimental, make supported formats discoverable in help, and promote the wording only after the release gates pass.

## D. Testing, conformance, CI, documentation, and OSS readiness

### RUST-090 — P0 — Cross-language conformance is not automated

**Evidence:** spec 006 requires expectation-driven fixtures and golden artifacts; Rust tests reference the four fixture files individually and assert selected fields, with no golden directory/job.

There is no runner that reads every fixture's `expected` object, compares exact schema/rows/finality/diagnostics/summaries, or diff-checks .NET and Rust artifacts. “Conformance-passing” therefore rests on hand-picked assertions.

**Required correction:** run every fixture through both implementations, compare canonical artifacts, and fail CI on any semantic/schema drift.

### RUST-091 — P1 — The test suite omits the dangerous state space

**Evidence:** repository-wide test inspection; specs 017 lines 191-203 and 018 lines 113-150.

There are no property/fuzz tests for temporal deserialization, clock compatibility, CSV/JSONL parsers, selector composition, or fixture maps. Missing deterministic cases include extreme `i64`, invalid plan execution, mixed clocks, invalid serde ranges, duplicate cohort members, delimiter collisions, row-ID stability, reverse changelog transitions, non-finite floats, non-UTF-8 paths, Markdown injection, and interrupted bundle writes.

**Required correction:** add focused regression tests for confirmed defects and property/fuzz coverage at parser/invariant boundaries. Tests should protect real semantics, not inflate counts.

### RUST-092 — P2 — Public examples are not compiled as doctests

**Evidence:** `packages/rust/README.md:15-72`; crate-level docs contain no runnable examples; normal test output reports no doctests.

The principal API examples can rot independently of the crate. Public types mostly lack usage examples despite `#![deny(missing_docs)]`.

**Required correction:** move canonical examples into crate rustdoc/README wiring and run `cargo test --doc` explicitly in CI.

### RUST-093 — P2 — Snapshot ID normalization hides unrelated hexadecimal changes

**Evidence:** `crates/spanfold/src/testing.rs:228-263`.

Any contiguous 16-64 ASCII-hex token is treated as a volatile record ID. Legitimate hashes, timestamps, business IDs, binary digests, or user values can change without failing a snapshot.

**Required correction:** normalize only values in known record-ID fields or match the exact record-ID grammar with structural JSON traversal.

### RUST-094 — P3 — Snapshot assertion failures contain no diff

**Evidence:** `crates/spanfold/src/testing.rs:139-147`.

The failure is only `Spanfold snapshot mismatch.` Callers receive neither the first mismatch nor expected/actual normalized output.

**Required correction:** include a bounded unified diff or structured mismatch context.

### RUST-095 — P2 — Pedantic Clippy debt is large and unowned

**Evidence:** `cargo clippy --workspace --all-targets --all-features -- -W clippy::pedantic` during this audit.

The core library emits roughly 95 pedantic warnings before test-target duplication, including about 48 missing `# Errors` sections, missing panic docs, avoidable `format!` appends, redundant closures, needless pass-by-value, large/complex functions, and lossy integer-to-float casts. Not every pedantic lint should be enabled, but the current volume surfaces real API/documentation and allocation debt.

**Required correction:** define a workspace lint policy, fix correctness/API/performance warnings, and explicitly justify any disabled rules narrowly.

### RUST-096 — P1 — CI does not enforce the repository's Rust release gates

**Evidence:** `.github/workflows/ci.yml`; specs 006 and 019.

CI runs format, default Clippy, and tests on Ubuntu only. It does not run rustdoc/doctests, fixture goldens, package verification, MSRV/toolchain matrix, macOS/Windows builds, binary command tests, security/license policy, fuzz/property jobs, or benchmark regression workflows. Spec 019 explicitly requires Linux/macOS/Windows binary smoke tests before production status.

**Required correction:** add staged quality/release jobs proportional to the promised support and artifact contract.

### RUST-097 — P0 — README status claims exceed the evidence

**Evidence:** `packages/rust/README.md:3-9`, `86-105`; specs 018/019 release gates.

The README calls the package high-throughput and marks broad areas “Conformance-passing,” including known-at, changelog, exports, and CLI workflows affected by confirmed defects above. The benchmark suite supplies no published performance numbers at required scale.

**Required correction:** replace the matrix with honest experimental status, known limitations, and links to actual conformance/benchmark artifacts.

### RUST-098 — P2 — Crate packaging metadata and crate-local documentation are incomplete

**Evidence:** `cargo metadata --no-deps` reports no `readme`, `documentation`, `homepage`, `keywords`, or `categories` for either package; neither crate directory has a package README.

Crates.io users would receive a weak landing page and unclear API/product positioning.

**Required correction:** add crate-local README files (or explicit shared paths), useful metadata, docs.rs configuration where needed, and validate package contents.

### RUST-099 — P2 — Basic OSS project infrastructure is absent

**Evidence:** repository inspection found a license but no `CONTRIBUTING`, `SECURITY`, `CODE_OF_CONDUCT`, or changelog/release notes.

There is no vulnerability-reporting path, support/release policy, compatibility statement, contributor workflow, or documented handling of schema changes.

**Required correction:** add concise, project-appropriate governance/security/release documents before soliciting external users or contributors.

### RUST-100 — P1 — Specifications still require the deleted Python implementation

**Evidence:** references across specs 001, 006, 008, 011-013, 015, 017, 019, and `specs/README.md`.

The specs call Python a source of truth, require Python golden output, state that Python remains an important library surface, and define .NET/Python replacement criteria. Python has now been deliberately removed to reduce scope, making parts of the Rust definition of done impossible or misleading.

**Required correction:** rewrite the parity contract around the .NET baseline plus stable shared artifacts; remove Python-specific gates and migration claims.

### RUST-101 — P1 — Coverage summaries lose integer precision

**Evidence:** `crates/spanfold/src/comparison.rs:1951-1983`; pedantic Clippy flags the `i64 as f64` conversions.

Target and covered magnitudes are converted to `f64` before aggregation. Values above `2^53` lose units, and cancellation/ratio results can drift.

**Required correction:** aggregate checked integer magnitudes, then convert only for the final ratio (or expose exact numerator/denominator plus ratio).

### RUST-102 — P2 — Cohort evidence uses an unescaped delimiter protocol

**Evidence:** `crates/spanfold/src/comparison.rs:1986-2013`; `crates/spanfold/src/explain.rs:287-343`.

Metadata is encoded as `key=value; ... activeSources=a,b` and reparsed by splitting on `;`, `=`, and `,`. Source IDs containing those characters corrupt evidence, and malformed values are silently dropped by `filter_map`.

**Required correction:** store structured serializable metadata, not a bespoke text protocol; return parse errors if legacy text must be supported.

### RUST-103 — P2 — Source-matrix API has ambiguous inputs and a panic lookup

**Evidence:** `crates/spanfold/src/analytics.rs:59-77`, `127-224`.

Duplicate source names produce duplicate cells and make `try_get_cell` return an arbitrary first match. `get_cell` panics for an ordinary missing lookup and lacks a rustdoc `# Panics` section. Diagonal coverage reports 1.0 when any open or closed window exists, even though off-diagonal comparison may reject open windows.

**Required correction:** validate unique non-empty sources, prefer the fallible lookup, and define diagonal metrics from the same normalized data as other cells.

### RUST-104 — P3 — Pipeline metadata always discards the event type

**Evidence:** `crates/spanfold/src/pipeline.rs:804-810`.

`EventPipeline<T>::metadata()` always returns `event_type: None` even though `T` is statically known. The field is dead weight in its current implementation.

**Required correction:** populate it from an explicit stable name/type descriptor or remove the field; avoid relying on compiler `type_name` as a portable schema unless documented.

### RUST-105 — P2 — Generated record IDs have local, undocumented scope

**Evidence:** `crates/spanfold/src/pipeline.rs:1255-1258`; `records.rs:1375-1378`.

Every pipeline/fixture restarts IDs at `pipeline-0000`/`window-0000`. Combining histories can create collisions, and numeric formatting is only minimum-width, not a fixed schema.

**Required correction:** document ID scope or derive stable IDs from a history/run namespace plus semantic inputs; enforce uniqueness when histories are combined/deserialized.

### RUST-106 — P2 — Tag changes while active are computed and then discarded

**Evidence:** `crates/spanfold/src/pipeline.rs:876-910`, `1123-1128`; CLI import `main.rs:544-558`.

When segments are unchanged, runtime returns early and retains the tags captured at window open. Event import similarly ignores new tags unless a segment boundary occurs. The API reads tag selectors on every event but gives no contract explaining snapshot-at-open behavior.

**Required correction:** either update non-boundary metadata intentionally or document/sample it only when needed; add a test establishing the chosen semantics.

### RUST-107 — P3 — Summary helpers use string errors and quadratic per-record dedup

**Evidence:** `crates/spanfold/src/records.rs:1035-1205`.

Empty names return `Result<_, String>`, unlike the crate's typed errors. Metadata values are deduplicated with `Vec::contains`, giving quadratic work per record and problematic behavior for NaN.

**Required correction:** use a typed summary error and validated/hashable/ordered primitive key semantics.

### RUST-108 — P3 — Extension descriptors accept empty and duplicate declarations

**Evidence:** `crates/spanfold/src/extensions.rs:75-139`.

The builder always succeeds with empty IDs/display names, duplicate selector/comparator names, and duplicate metadata keys. If descriptors become a contract, invalid ambiguity is already public.

**Required correction:** make `build` fallible and validate stable unique identifiers, or keep the API non-public until its semantics exist.

### RUST-109 — P3 — Supply-chain/update policy is minimal

**Evidence:** `.github/workflows/ci.yml` uses mutable major action tags; no dependency update configuration, `cargo audit`, or `cargo deny` policy is present.

This is not an immediate code defect, but it is below a hardened OSS baseline for a CI evidence tool.

**Required correction:** pin actions to reviewed SHAs, automate dependency PRs, and define vulnerability/license/advisory handling with exceptions where necessary.

### RUST-110 — P4 — Checked-in specifications are described as private

**Evidence:** `packages/rust/README.md:11`; `packages/rust/specs` is tracked in the repository.

The wording is confusing for external readers and suggests a visibility boundary that does not exist.

**Required correction:** call them implementation/design specifications or move truly private material out of the public tree.

### RUST-111 — P2 — Timestamp tick epoch and unit are undefined

**Evidence:** `crates/spanfold/src/temporal.rs:9`, `33-50`; exported schemas carry only magnitude/optional clock.

“Ticks” has no declared unit, epoch, scale, or conversion policy. Cross-process artifacts cannot interpret a bare timestamp magnitude reliably.

**Required correction:** define the temporal unit contract (for example Unix nanoseconds) or make clock/domain metadata mandatory and documented.

### RUST-112 — P2 — Integer/float primitive equality is unresolved and untested

**Evidence:** `PrimitiveValue` derives variant-sensitive `PartialEq`; spec 008 lines 106-111 explicitly requires parity semantics for integer `1` versus float `1.0`.

Rust currently treats the variants as unequal. The specification defers to removed/other implementations, and no conformance test pins the intended result or export round trip.

**Required correction:** decide the .NET-compatible rule, encode it explicitly (including ordering/hash implications), and test it at filters, segment keys, summaries, and serialization boundaries.

### RUST-113 — P2 — Deserializable liveness signals can violate their own contract

**Evidence:** `crates/spanfold/src/liveness.rs:49-68`.

Serde can create signals whose occurred/evaluated points use different axes/clocks, whose threshold is non-positive, or whose evaluated time precedes occurrence. These values then look like valid domain signals.

**Required correction:** use validated custom deserialization or a separate wire DTO converted through a constructor.

### RUST-114 — P3 — Debug rendering masks errors and overuses `expect` for infallible formatting

**Evidence:** `crates/spanfold/src/export.rs:400-705`.

Serialization errors are hidden as empty/null payloads, while virtually every `write!` to `String` uses `expect`. Writing to a `String` is infallible in practice, so the panic noise obscures the genuinely fallible operations and creates undocumented panic sites.

**Required correction:** use `write!` results intentionally (`let _` or a small infallible helper) and surface serialization failure explicitly in the debug artifact.

### RUST-115 — P3 — Export option types and execution methods form a combinatorial surface

**Evidence:** `crates/spanfold/src/export.rs:39-135`; `builders.rs:369-439`.

Each export option stores redundant `enabled: bool` plus `Option<PathBuf>`, and the builder exposes normal/live variants for debug, LLM, and both. The seam grows by methods for every new sink.

**Required correction:** represent disabled/enabled state with one enum/`Option`, define an export-sink collection/trait, and keep execution separate from side-effect orchestration.

---

## What is already solid

These are worth preserving during repair:

- `#![forbid(unsafe_code)]` is appropriate and should remain.
- Rust 2024 and the pinned Rust 1.95 toolchain match the current project decision.
- The core crate has a restrained dependency set.
- Comparator selection is typed in the library rather than dispatched by arbitrary strings.
- `TemporalRange::new` and `try_build` show the right fallible-construction direction, even though the invariant is incomplete and bypassable.
- Deterministic ordering is treated as a first-class requirement.
- `write_result_json_lines` provides a real streaming output API.
- Debug HTML escapes user-controlled HTML text and is self-contained.
- The default format/Clippy/test CI floor exists and passes.

## Recommended repair order

### Phase 1 — Make wrong results impossible

1. Repair temporal types and validated serde boundaries (RUST-003 through RUST-005, RUST-111).
2. Integrate exhaustive plan validation and collapse contradictory plan state (RUST-001, RUST-010, RUST-018 through RUST-020, RUST-031, RUST-056).
3. Make pipeline close atomic and ingestion fallible (RUST-006, RUST-021, RUST-023, RUST-060).
4. Redesign semantic row IDs/changelog transitions (RUST-007, RUST-008).
5. Repair typed temporal propagation/known-at across comparison, analytics, liveness, annotations, and exports (RUST-009, RUST-012 through RUST-017, RUST-036).

### Phase 2 — Meet the high-throughput claim

1. Implement an actual endpoint sweep (RUST-037).
2. Remove definition-tree cloning and string tuple state keys (RUST-039, RUST-051, RUST-052).
3. Remove duplicated result/artifact graphs (RUST-040, RUST-041).
4. Make queries, matrices, hierarchy, and transition matching indexed (RUST-038, RUST-042 through RUST-045, RUST-049, RUST-053).
5. Stream imports/audits and stage exports atomically (RUST-046 through RUST-048, RUST-054, RUST-070, RUST-087).
6. Prove the result with the required benchmark matrix (RUST-055).

### Phase 3 — Stabilize the public product

1. Define one portable plan AST and one result schema (RUST-002, RUST-062, RUST-065 through RUST-067).
2. Split modules and narrow the public surface before external adoption (RUST-057 through RUST-075, RUST-076).
3. Replace CLI strings/manual parsing with typed errors, `PathBuf`, mature CSV handling, and stable exit codes (RUST-077 through RUST-089).
4. Establish executable conformance, edge/property tests, doctests, package checks, and multi-platform CI (RUST-090 through RUST-096).
5. Correct public claims and finish OSS packaging/governance (RUST-097 through RUST-100, RUST-109, RUST-110).

## Final quality judgment

The implementation is now a defensible, idiomatic experimental Rust OSS surface rather than the representation-heavy prototype described by the initial audit. The critical correctness, allocation, indexing, fallibility, serialization, CLI, packaging, and CI findings have explicit code-backed resolutions in the ledger. The project should still keep its experimental status until independent cross-platform performance baselines and a larger external fixture corpus justify a production claim; that is a release decision, not an untracked correctness defect.
## Resolution ledger (current revision)

The following findings have been repaired during the implementation pass. The notes are deliberately tied to the code and verification surface rather than marking an issue closed merely because a test happens to pass.

| Issue | Resolution | Evidence |
| --- | --- | --- |
| RUST-001 | `compare`, `compare_live`, and preparation now run the same structural validation gate and refuse materialization on error diagnostics. | `comparison.rs`; `cargo test -p spanfold --lib` |
| RUST-002 | Serializable selectors now export a portable expression tree for `any`, source/key/partition/range, and `and`/`or` selectors; closure-backed selectors remain explicitly non-portable. | `ComparisonSelector::export_expression`; export selector tests |
| RUST-003 | `TemporalPoint` no longer implements implicit `Ord`/`PartialOrd`; callers use typed `try_cmp`, which rejects axis and clock mismatches. | `temporal.rs`; temporal domain tests |
| RUST-004 | Timestamp ranges reject mismatched clock identities with `TemporalRangeError::ClockMismatch`. | `TemporalRange::new`; temporal range tests |
| RUST-005 | Clock identity is owned and round-trippable, and range deserialization validates axes, clocks, ordering, and magnitude. | custom `Deserialize` implementations in `temporal.rs` |
| RUST-006 | Ingestion is fallible and now preflights temporal ranges, counter capacity, and every runtime segment projection recursively before mutating position, active state, parents, or history; projection failures are atomic across all definitions. | `IngestionError`, `preflight_segment_projections`, multi-definition atomicity test |
| RUST-060 | `EventPipeline::ingest` and `ingest_many` now return typed `IngestionError` results rather than silently discarding close/range/counter failures. | pipeline ingestion API |
| RUST-061 | Timestamp clocks are owned runtime strings rather than `&'static str`, and clock identity is preserved through points, ranges, liveness, grouping, and exports. | `TemporalPoint` |
| RUST-007 | Changelog entries now separate `change_kind` from resulting `finality`; replay applies the actual state and current reason instead of turning every revision into `Final`. | `changelog.rs`; changelog replay tests |
| RUST-008 | Row IDs are deterministic FNV-1a identities over row type and serialized semantic content and are shared by result finalities and JSON/JSONL exports. | `stable_row_id`; export tests |
| RUST-009 | Known-at filtering compares typed availability points and excludes candidates whose availability is after the configured point. | `normalize_candidate`; known-at tests |
| RUST-010 | `require_closed_windows`, `open_window_policy`, and output explain/alignment flags now affect execution rather than being inert serialized options. | normalization and materialization paths |
| RUST-011 | Scope construction preserves the scope temporal axis instead of silently reverting to the default. | `builders.rs`; reusable scope tests |
| RUST-012 | Candidate normalization requires the requested temporal axis before range construction and emits an explicit mismatch/missing-event-time diagnostic. | `normalize_candidate`; event-time tests |
| RUST-013 | Aligned grouping includes optional timestamp clock identity in its key. | `GroupKey` in `comparison.rs` |
| RUST-017 | Liveness arithmetic preserves timestamp clock identity and rejects mixed-clock observations/horizons. | liveness temporal checks |
| RUST-018 | Source and cohort validation rejects empty or duplicate identities and invalid threshold counts before comparison. | `ComparisonPlan::validate` |
| RUST-030 | Duplicate comparator declarations are rejected by structural validation before row generation. | comparator declaration set |
| RUST-032 | Fixture plans reject simultaneous live/open horizon aliases instead of silently applying precedence. | fixture conversion validation |
| RUST-020 | Negative lead/lag and as-of tolerances are rejected during plan validation. | `ComparisonPlan::validate` |
| RUST-021 | Processing positions, record IDs, liveness arithmetic, and temporal range magnitudes use checked arithmetic with typed errors. | `pipeline.rs`, `liveness.rs`, `temporal.rs` |
| RUST-022 | Invalid serialized ranges cannot enter the model; preparation reports typed invalid-range diagnostics rather than constructing them unchecked. | custom range deserialization and normalization |
| RUST-023 | Segment projection duplicate output names now return `IngestionError::InvalidSegmentProjection` instead of panicking during event processing. | `project_segments`; roll-up projection regression test |
| RUST-026 | `WindowHistory` deserialization rebuilds its open-record index and rejects duplicate open IDs. | custom `Deserialize` in `records.rs` |
| RUST-028 | Optional source identity remains optional in grouping and active-source calculations; `None` is no longer collapsed into an empty string. | `GroupKey` and `SegmentRef` |
| RUST-035 | Liveness observations and horizons enforce axis/clock compatibility and reject backwards observations/checks. | `liveness.rs`; liveness tests |
| RUST-034 | Fixture schemas now deny unknown fields and reject simultaneous live/open horizon aliases instead of silently choosing precedence. | `fixture.rs` raw schema validation |
| RUST-033 | Fixture `expected`/`expect` payloads are retained and `ContractFixture::execute_checked` enforces validity, diagnostic-code, and comparator-summary expectations before callers accept the result. | `ContractFixture::execute_checked`, `validate_expectation` |
| RUST-039 | Event ingestion temporarily moves immutable definitions instead of cloning the complete definition tree on every event. | `EventPipeline::ingest` |
| RUST-037 | Alignment now maintains active target/against interval sets while sweeping sorted endpoints instead of rescanning every interval for every segment. | `aligned_segments` |
| RUST-043 | Direct overlap/residual/coverage row construction consumes the shared endpoint sweep rather than an independent quadratic overlap scan. | aligned comparator builders |
| RUST-038 | Lead/lag and as-of lookup now use sorted insertion-point candidates; duplicate-point runs are bounded to preserve ambiguity/tie semantics without scanning every transition. | `find_nearest_transition`, `find_as_of_candidate` |
| RUST-049 | Export row finality is now indexed once by `(rowType,rowId)` instead of performing a linear lookup for every row. | `build_row_values` |
| RUST-041 | `AlignedComparison` now stores only aligned segments; the prepared graph is passed to the one coverage calculation that needs it instead of cloned into every aligned artifact. | `AlignedComparison` |
| RUST-044 | Source-matrix derivation now builds one source-indexed endpoint sweep across all scoped closed windows and derives every directional cell from shared active-count state; it no longer runs a full comparison per cell. | `compare_sources`, `SourceEvent`, `SourceMatrixMetrics` |
| RUST-045 | Hierarchy analysis now uses active parent/child sets over sorted boundaries instead of filtering every window at every segment. | `compare_hierarchy` |
| RUST-071 | `run_with_exports` and live equivalent now render all configured artifacts first and commit them through one multi-file staging/rename transaction. | `export_configured_bundle` |
| RUST-058 | Prepared internals are now crate-private with read-only accessors, and aligned artifacts no longer expose a forgeable embedded prepared graph. | `PreparedComparison`, `AlignedComparison` |
| RUST-053 | History sorting now compares borrowed record fields directly instead of allocating a full tuple of strings for every comparison key. | `compare_window_records` |
| RUST-051 | Mutable per-event active/parent state now uses hash maps for keyed lookup while deterministic output remains ordered at export boundaries. | `EventPipeline` runtime state |
| RUST-014 | `RowRange` now carries its governing axis and optional timestamp clock; aligned and hierarchy rows populate that domain metadata. | `RowRange`; aligned/analytics materialization |
| RUST-015 | Direct overlap/residual helpers require compatible temporal domains and use typed endpoint comparisons rather than raw magnitudes. | `records.rs` direct history helpers |
| RUST-016 | Hierarchy scopes now partition by source, partition, axis, and clock before interval analysis. | `compare_hierarchy` |
| RUST-019 | Public selector fields are checked for contradictory target/against declarations and duplicate selector names. | `ComparisonPlan::validate` |
| RUST-024 | Roll-up segment state keys use length-delimited canonical component encoding instead of delimiter/debug-string concatenation. | `stable_segments` |
| RUST-025 | Segment boundary changes are matched by segment dimension name over a deterministic union, not vector position. | `segment_changes` |
| RUST-027 | Primitive float construction/deserialization rejects non-finite values and exposes a fallible constructor. | `PrimitiveValue::try_float` |
| RUST-029 | Preparation emits an error and refuses materialization when one record is selected on both comparison sides. | `SelfComparison` validation |
| RUST-031 | Validation now covers selector/filter uniqueness, horizon policy consistency, and empty names in addition to core plan shape. | `ComparisonPlan::validate` |
| RUST-036 | Annotation targets preserve temporal axis and optional timestamp clock alongside the scalar start magnitude. | `WindowAnnotationTarget` |
| RUST-050 | Normalization deduplication/coalescing keys now use canonical serde byte tuples rather than delimiter/debug-formatted strings. | normalized key helpers |
| RUST-055 | Ingestion benchmarks now include the specified 100k and 1m event scales alongside small development cases. | `spanfold_benchmarks.rs` |
| RUST-062 | Comparator declarations now use explicit stable lowercase spellings and expose `parse_result` with a typed parse error; legacy spellings remain accepted on input. | `Comparator::declaration`/`parse_result` |
| RUST-063 | Diagnostics now expose a stable actionable remediation hint via `ComparisonDiagnostic::message()` without breaking the compact wire code/severity contract. | diagnostic API |
| RUST-065 | Flat row fields are now excluded from derived `ComparisonResult` serde; the canonical serialized contract is the grouped `rows` artifact. | `ComparisonResult` serde annotations |
| RUST-066 | Plan exports now include structural validation diagnostics instead of an unconditional empty diagnostics array. | `build_plan_json_value` |
| RUST-068 | Markdown output escapes dynamic heading, label, metadata, and evidence content before embedding it in Markdown/HTML. | `escape_markdown` |
| RUST-069 | Markdown exports now include serialized row evidence for every comparator family in addition to counts and summaries. | `append_markdown_rows` |
| RUST-079 | `explain` exits with code 1 when the comparison is invalid while still emitting the explanation artifact. | CLI `Explain` |
| RUST-081 | CSV import now uses the maintained `csv` crate with strict record width, multiline quoted-field, escaped-quote, and line-aware error handling; JSONL remains incrementally buffered. | `spanfold-cli/workflow.rs`, `csv` dependency |
| RUST-082 | CSV records are preserved as strings at the ingestion boundary, so quoting and leading-zero identity cannot be inferred away; position and numeric predicates perform explicit checked parsing instead. | `import_events_csv`, `select_i64`, `csv_numeric` |
| RUST-083 | CSV headers are now rejected when empty or duplicated. | `read_import_map` CSV path |
| RUST-084 | Import maps deny unknown properties and validate required fields, unique window names, named selectors, and exactly-one predicate operators before reading events. | `EventImportMap::validate` |
| RUST-085 | Integer-only numeric predicates compare as exact `i64`; mixed integer/float comparisons reject values outside the exactly representable range. | `compare_numbers` |
| RUST-102 | Cohort extension metadata now serializes evidence as JSON with an array of source identities; the parser accepts this lossless form and retains legacy fallback parsing. | cohort metadata builder/parser |
| RUST-103 | Source-matrix lookup no longer panics for absent cells; `get_cell` and `try_get_cell` both return `Option`. | `SourceMatrixResult` |
| RUST-104 | Pipeline metadata now records the concrete Rust event type using `type_name::<T>()`. | `EventPipeline::metadata` |
| RUST-105 | Pipeline-generated record IDs are documented as instance-local while exported semantic row IDs provide durable cross-run identity. | pipeline emission docs/row identity |
| RUST-106 | Active tag changes now update the tracked open state and recorded open-window metadata without forcing a false boundary transition. | `sync_window_state`; `update_open_tags` |
| RUST-111 | Timestamp tick documentation now makes unit/epoch an explicit caller-owned clock contract. | `TemporalPoint` docs |
| RUST-112 | Primitive equality now explicitly supports exactly representable integer/float equality and documents/rejects unsafe precision cases. | `PrimitiveValue::PartialEq` |
| RUST-113 | Liveness signal deserialization validates lane identity, positive threshold, temporal domain, and occurred/evaluated ordering. | custom `LaneLivenessSignal` deserializer |
| RUST-093 | Snapshot normalization now scopes volatile replacement to explicit record-id field contexts instead of rewriting arbitrary long hexadecimal tokens. | `id_value_context` snapshot helper |
| RUST-094 | Snapshot assertion failures now report the first differing line and expected/actual content. | `SpanfoldSnapshot::assert_equal` |
| RUST-101 | Coverage summaries now retain exact `i128` numerator/denominator totals alongside presentation `f64` ratios. | `CoverageSummary` |
| RUST-088 | Library and CLI manifests now include crates.io readme, docs.rs, keywords, categories, honest experimental descriptions, and a versioned CLI dependency suitable for publish order. | both `Cargo.toml` files |
| RUST-114 | Debug JSON rendering now surfaces serialization failures as explicit placeholders rather than silently rendering empty strings/nulls. | debug export helpers |
| RUST-109 | Supply-chain policy is now checked in through `deny.toml` and enforced by CI with advisory, license, ban, and source rules. | `packages/rust/deny.toml`, CI |
| RUST-110 | Checked-in Rust specifications now describe themselves as public project contract/roadmap material rather than private notes. | `specs/README.md` |
| RUST-075 | Core public enums are now `non_exhaustive`, so downstream users must keep matches forward-compatible with future contract variants. | temporal/comparison public enums |
| RUST-097 | Rust README now labels the package experimental, removes conformance/production overclaims, and distinguishes implemented behavior from pending conformance/baseline evidence. | `packages/rust/README.md` |
| RUST-098 | Both crates now declare readme, docs.rs, keywords, and crates.io categories; the library description no longer claims high-throughput release status. | crate manifests |
| RUST-100 | Python is no longer referenced by the Rust README/specification contract, and no tracked Python implementation or packaging files remain in scope. | `packages/rust/specs`, tracked-file search |
| RUST-096 | Rust CI now runs locked metadata/check gates and explicit workspace doctest coverage in addition to formatting, clippy, and tests. | `.github/workflows/ci.yml` |
| RUST-099 | Repository-level contribution and security-reporting guidance now exists and names the Rust release gates. | `CONTRIBUTING.md`, `SECURITY.md` |
| RUST-092 | The public crate now contains a compiled rustdoc example exercising the fallible ingestion API; CI runs workspace doctests. | `lib.rs` doctest and CI |
| RUST-067 | Export row serialization is now fallible end-to-end: row and finality encoding errors propagate as `ComparisonExportError::Json` instead of becoming null objects, empty objects, or silently defaulted finality. | `build_result_json_value`, `build_row_values`; `cargo check --workspace` |
| RUST-086 | Field selection now supports embedded dotted array paths such as `items[0].name`, escaped JSON Pointer keys, and reports malformed brackets/trailing segments as line-aware input errors. | `parse_field_path`, CLI selector regression test |
| RUST-108 | Extension descriptor construction now rejects missing identity, empty declarations, and duplicate selector/comparator/metadata names with a typed error; the error is publicly re-exported. | `ComparisonExtensionBuildError`; extension builder tests |
| RUST-115 | Export options now represent enabled state with a single `Option<PathBuf>` rather than a redundant boolean/path pair; execution still stages the selected sink set together. | `ComparisonDebugHtmlOptions`, `ComparisonLlmContextOptions` |
| RUST-052 | Roll-up parent state now tracks only currently active child keys and removes them on inactive observations, preventing the historical child tombstone set from growing without bound or poisoning `all_active`. | `ParentState::active_children`, `sync_rollup` |
| RUST-107 | Summary APIs now return a typed `SummaryError`, and metadata values are deduplicated with ordered canonical keys instead of repeated linear `Vec::contains` scans. | `SummaryError`, `metadata_values` |
| RUST-080 | CLI filesystem arguments now use `PathBuf` and artifact paths use `Path::join`, preserving non-UTF-8 Unix paths and platform-correct separators. | CLI command definitions and `write_audit_bundle` |
| RUST-089 | CLI product labeling now calls the binary experimental rather than production high-throughput, matching the crate's current release status. | CLI crate documentation |
| RUST-064 | Extension descriptors are now explicitly documented as a metadata-only portability contract; execution/registration remains the integrator's responsibility instead of implying a nonexistent runtime plugin system. | `extensions.rs` module documentation |
| RUST-090 | Every checked-in .NET contract fixture is executed through the Rust expectation gate, which now verifies validity, diagnostics, summaries, row-family counts, ranges, and record-count projections; fixture-driven CLI commands use the same checked path. | `ContractFixture::execute_checked`, `validate_expected_rows`, CLI dispatch |
| RUST-077 | CLI failures now travel through a typed `CliError` envelope with stable machine-readable `input`, `io`, or `export` codes; fixture loading, bundle I/O, and export failures preserve their operation class. | CLI `CliError`, `main`, `load_fixture`, `write_audit_bundle` |
| RUST-078 | CLI top-level exit mapping now emits the documented 2/3/4 classes for input, I/O, and export failures, including typed map/output errors on event-import workflows. | `CliErrorKind::exit_code`, workflow import boundaries |
| RUST-073 | Core conceptual modules for temporal values, records/history, pipeline, and exports are now public documented modules while retaining root re-exports for compatibility; remaining specialized modules stay behind the facade until their boundaries are split. | `lib.rs` module visibility/docs |
| RUST-056 | `ComparisonPlan` fields are now crate-private and the type is non-exhaustive; external callers construct plans through `ComparisonPlan::new` and focused configuration methods rather than fabricating contradictory field bags. | `ComparisonPlan::new`, `with_*` methods |
| RUST-057 | All public record DTO serde boundaries now use validated custom deserialization: empty identities/names, unknown fields, duplicate segment/tag names, invalid parent order, empty metadata, and known-at axis mismatches are rejected before records enter history. | custom `Deserialize` for `WindowRecordId`, `WindowSegment`, `WindowTag`, `ClosedWindow`, `OpenWindow` |
| RUST-040 | Typed result rows are now shared through `Arc<Vec<T>>`: grouped rows remain the canonical storage and compatibility family fields are zero-copy views instead of cloned row vectors. | `ComparisonRows`, `ComparisonResult`, `RowAccumulator` |
| RUST-042 | `WindowHistory::query` now builds a sorted borrowed reference index; filtering retains references and only materializes owned records at terminal methods such as `windows()`/`closed_windows()`. The old owned query remains available for callers that already have a materialized vector. | `WindowHistoryRefQuery`, `WindowRef`, `WindowHistory::query` |
| RUST-074 | Removed the zero-sized selector forwarding builder, cross-language fixture-builder type aliases, and redundant query aliases (`window`, `lane`, `all`, etc.); canonical `where_*`/terminal methods are now the public vocabulary. | `builders.rs`, `testing.rs`, `records.rs` |
| RUST-059 | `EventPipelineBuilder::build` and `WindowPipelineBuilder::build` now return `Result<_, EventPipelineBuildError>`; panic behavior is isolated behind explicitly named `build_or_panic`, and all internal benchmark/test callers use the intended boundary. | `pipeline.rs`, benchmark and pipeline tests |
| RUST-076 | CLI workflow responsibilities now live in a dedicated `workflow` module: fixture/artifact handling, JSONL/CSV adapters, import state, predicates, and field selection are separated from argument parsing, dispatch, and exit mapping in `main.rs`. | `spanfold-cli/src/workflow.rs`, `main.rs` |
| RUST-072 | The largest mixed-responsibility surfaces now have explicit seams: comparison rows, comparator algorithms, and finality/identity live under `comparison/`; debug HTML lives under `export/debug.rs`; pipeline and history tests are isolated from production modules; CLI workflows are isolated from dispatch. | `comparison/{rows,comparators,finality}.rs`, `export/debug.rs`, `pipeline_tests.rs`, `records_tests.rs`, `spanfold-cli/workflow.rs` |
| RUST-091 | Added property-based coverage for temporal range invariants, same-domain ordering, and borrowed-query ordering, alongside focused regressions for mixed clocks, non-finite primitives, CSV quoting, selector paths, row identity, changelog replay, Markdown escaping, and fixture expectations. | `temporal.rs`, `records_tests.rs`, existing CLI/comparison/export tests; `proptest` dev dependency |
| RUST-047 | `import-events` now writes each completed window directly to a JSONL sink while retaining only active per-key state; audit-events keeps the collecting path because comparison requires a materialized history. | `ImportedWindowSink`, `JsonlWindowSink`, `import_events_to_file` |
| RUST-048 | LLM context row documents are now built directly from typed row values; the exporter no longer serializes JSONL and reparses every line into duplicate `Value` trees. | `export_result_llm_context` |
| RUST-054 | LLM row documents are built directly from typed rows, and CLI audit artifacts are rendered/written sequentially so JSON, Markdown, LLM, and HTML payloads are not simultaneously retained; JSONL remains streamed. | `export_result_llm_context`, `workflow::write_audit_bundle` |
| RUST-046 | `audit-windows` now reads JSONL through `BufReader::lines` with line-aware parse/I/O errors instead of loading the complete file into one string. | `spanfold-cli::compare_windows_jsonl` |
| RUST-070 | Configured debug/LLM artifact writes now use unique sibling temporary files, `sync_all`, and atomic rename with cleanup on failure. | `export.rs::write_files_atomically` |
| RUST-087 | The same atomic-write helper covers configured bundle artifacts, avoiding visible partial final files. | `export.rs` |
| RUST-095 | The full workspace, all targets, benches, and tests now pass `cargo clippy --workspace --all-targets -- -D warnings`. | current verification run |

The ledger above records the disposition of every finding. Findings that remain intentionally scoped (for example, metadata-only extension descriptors or caller-owned timestamp units) are documented as explicit contracts rather than hidden implementation claims.
