# Rust Row Identity And Finality Consumer API

**Status:** Implemented for the planned Rust 0.1.1 release  
**Immediate target:** `spanfold` 0.1.1, if that version remains unpublished  
**Scope:** Rust consumer API and Rust comparison exports; a shared .NET/Rust
identity format is a separate pre-1.0 contract task

## Decision summary

Spanfold should close the downstream integration gap, but it should not expose
the proposed public generic hashing trait.

The immediate API should let a consumer iterate each typed row family together
with the authoritative `ComparisonRowFinality` produced for that row. This keeps
identity and finality result-owned, avoids downstream hashing and positional
IDs, avoids a linear metadata search for every row, and does not turn the
current hash implementation into a public extension point.

The same result-owned pairing must also drive every export. Exporters must use
the stored authoritative row ID and finality instead of recomputing identity
from format-specific labels. Missing metadata must be an error; it must never
silently become `Final`.

This work should also add a typed `ComparisonRowKind`, clarify coverage
semantics and canonical row storage, and correct the three advanced-family
labels in Rust JSON Lines. It should not yet claim that a Rust row ID is a
permanent or cross-language identifier: Rust currently hashes serialized JSON
with 64-bit FNV-1a, while .NET hashes a separate canonical description with
SHA-256.

## Current evidence

The integration concern is real:

- [`comparison/finality.rs`](crates/spanfold/src/comparison/finality.rs) creates
  content-derived IDs, but only private and crate-private generic helpers can
  calculate them.
- [`comparison/rows.rs`](crates/spanfold/src/comparison/rows.rs) exposes typed
  rows and a separate `Vec<ComparisonRowFinality>` without a supported typed
  association between them.
- [`export.rs`](crates/spanfold/src/export.rs) independently recalculates IDs.
  JSON Lines uses `symmetric-difference`, `lead-lag`, and `asof`, while result
  finality and canonical JSON use `symmetricDifference`, `leadLag`, and `asOf`.
- Rust JSON Lines currently omits row finality, reason, version, and supersession
  metadata. A streaming consumer can therefore lose finality even when the
  in-memory result was correct.
- `build_row_values` currently defaults missing metadata to `Final`. A label or
  identity mismatch can therefore turn provisional evidence into final
  evidence in an export.
- [`ComparisonRowIdentity.cs`](../dotnet/src/Spanfold/Internal/Comparison/ComparisonRowIdentity.cs)
  uses SHA-256 over explicitly selected canonical fields. Rust uses FNV-1a over
  serde JSON. Equal logical rows are not currently guaranteed to have equal IDs
  across implementations.

The current result construction order is useful: `build_row_finalities` walks
the same nine canonical row collections in the same order in which they are
stored. That permits allocation-free typed row/metadata views, provided the
association is validated and owned by Spanfold rather than reconstructed by a
consumer.

## Proposal review

| Proposal | Decision | Reason |
| --- | --- | --- |
| Expose authoritative identity and finality to typed consumers | **Concede** | Positional `family:index` IDs are unstable and dropping finality is unsafe. This is the release-worthy consumer gap. |
| Add `ComparisonRowKind` | **Concede with narrower ownership** | A closed enum should own result-row kinds and canonical artifact names. It must not replace comparator declarations, which are a separate configuration grammar and can contain parameters. |
| Add public `ComparisonRowIdentity: Serialize` | **Rebut** | The proposed trait is implementable downstream unless sealed, exposes serialization as identity policy, invites recomputation, and makes the current hash behavior part of the public API. The row families are closed, so an open trait is the wrong abstraction. |
| Add `ComparisonResult::row_finality_for(row)` | **Reframe** | A simple implementation performs a hash plus a linear scan per row, making normal iteration quadratic. Result-owned typed row/finality views solve the actual iteration use case in linear time without public hashing. Add arbitrary lookup only if a real second use case needs it. |
| Centralize row labels | **Concede for row artifacts** | Canonical row-kind labels should drive metadata, identity, JSON, JSON Lines, and LLM row documents. `Comparator::declaration()` should retain its existing hyphenated and parameterized syntax. |
| Preserve finality through all paths | **Concede and strengthen** | Typed views are necessary, and JSON Lines must also carry finality metadata. Export must fail on missing metadata instead of defaulting to `Final`. |
| Clarify `CoverageRow` versus `CoverageSummary` | **Concede** | A coverage row is one aligned target segment and is normally wholly covered or uncovered. The summary is the grouped aggregate and the right source for an overall ratio. |
| Declare `ComparisonResult::rows()` canonical | **Concede** | `rows` is canonical storage and serialization. The public family accessors borrow typed slices from it without duplicate flat fields or wire keys. |
| Freeze fixed FNV vectors for every family now | **Rebut for 0.1.1** | Golden digests would prematurely freeze a Rust-only, serde-coupled 64-bit scheme that already differs from .NET. Protect cross-export equality now; add fixed cross-language vectors after a shared identity specification is approved. |
| Change the hash algorithm during this integration fix | **Defer** | An uncoordinated change would rewrite every Rust ID and still would not prove .NET parity. Specify and version the shared format first. |
| Add string helpers for `TemporalAxis`, `ComparisonSide`, and `ContainmentStatus` | **Defer** | These are already typed enums, and presentation strings legitimately belong to adapters. Add canonical text only when it is part of a Spanfold wire contract or more consumers demonstrate the need. |
| Add `RowRange::duration()` | **Defer** | Checked `end - start` is small and consumer-neutral. A method is worthwhile only when Spanfold defines the invalid-range behavior and multiple callers benefit. |
| Add a heterogeneous public row enum | **Defer** | It can be added later without breaking the typed API. One external adapter does not yet justify forcing every consumer through non-exhaustive heterogeneous matching. |

## Contract model

The design keeps four concepts distinct:

1. **Row kind** identifies the closed result-row family.
2. **Row value** is the typed comparison evidence such as `OverlapRow`.
3. **Row ID** is an opaque identifier assigned by the producing Spanfold result.
4. **Row finality metadata** describes the ID's final/provisional state, reason,
   version, and supersession relationship in that result.

A row ID is not a method by which arbitrary serializable values become
Spanfold evidence. Finality is not intrinsic to a cloned row value; it belongs
to the result snapshot that produced the row. The primary consumer seam should
therefore borrow both from the result.

## Proposed Rust API

### Typed row kind

```rust
#[non_exhaustive]
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ComparisonRowKind {
    Overlap,
    Residual,
    Missing,
    Coverage,
    Gap,
    SymmetricDifference,
    Containment,
    LeadLag,
    AsOf,
}

impl ComparisonRowKind {
    /// Returns the canonical comparison-artifact spelling.
    pub const fn as_str(self) -> &'static str;
}

impl FromStr for ComparisonRowKind {
    type Err = ComparisonRowKindParseError;

    /// Accepts canonical spellings plus the three Rust 0.1.0 JSONL aliases.
    fn from_str(value: &str) -> Result<Self, Self::Err>;
}

impl ComparisonRowFinality {
    pub fn row_kind(&self) -> Result<ComparisonRowKind, ComparisonRowKindParseError>;
}
```

Canonical artifact spellings are:

| Kind | Canonical artifact spelling | Accepted Rust 0.1.0 alias |
| --- | --- | --- |
| `Overlap` | `overlap` | none |
| `Residual` | `residual` | none |
| `Missing` | `missing` | none |
| `Coverage` | `coverage` | none |
| `Gap` | `gap` | none |
| `SymmetricDifference` | `symmetricDifference` | `symmetric-difference` |
| `Containment` | `containment` | none |
| `LeadLag` | `leadLag` | `lead-lag` |
| `AsOf` | `asOf` | `asof` |

`Comparator::declaration()` remains unchanged. For example,
`lead-lag:start:timestamp:5` is a comparator declaration, not a row kind.

### Result-owned typed views

```rust
#[derive(Clone, Copy, Debug)]
pub struct ComparisonRowWithFinality<'a, R> {
    pub row: &'a R,
    pub metadata: &'a ComparisonRowFinality,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "inconsistent {family:?} row metadata at index {metadata_index}: \
     expected {expected_count} {expected_kind:?} records, found {actual_count}; \
     actual kind: {actual_kind:?}"
)]
pub struct ComparisonRowMetadataError {
    pub family: ComparisonRowKind,
    pub metadata_index: usize,
    pub expected_count: usize,
    pub actual_count: usize,
    pub expected_kind: ComparisonRowKind,
    pub actual_kind: Option<String>,
}

impl ComparisonResult {
    pub fn overlap_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, OverlapRow>>,
        ComparisonRowMetadataError,
    >;

    // Equivalent methods for residual, missing, coverage, gap,
    // symmetric difference, containment, lead/lag, and as-of.
}
```

The methods should:

- borrow canonical rows from `ComparisonResult::rows()`;
- borrow the authoritative existing `ComparisonRowFinality` records;
- validate the expected count and kind before yielding entries;
- preserve family and row order;
- allocate no row or metadata objects;
- never hash or scan the complete finality vector once per yielded row.

`ComparisonRowMetadataError` should report the row family, expected and actual
family counts, expected and actual kinds, and the absolute metadata index at
which validation first failed. `actual_kind` is `None` when metadata is absent
at that index. This gives exporters and adapters useful context without
pretending that validation can reconstruct identity.

### Ordering and corruption boundary

Spanfold explicitly guarantees that, for a genuine Spanfold-produced result:

- `row_finalities` is partitioned in canonical family order;
- metadata within each family is parallel to row order in
  `ComparisonResult::rows()`;
- each typed view yields the row and metadata at the same family-relative
  index;
- every Spanfold exporter consumes that same association.

Count and kind validation catches missing families, extra metadata, and kind
layout corruption. It deliberately does not recompute identity, so it cannot
detect two metadata records reordered within the same family. Detecting that
case would require hashing again or changing result storage, both of which this
0.1.1 design intentionally avoids.

Reordering or independently replacing the public row and metadata fields after
production is unsupported. Export consistency guarantees apply to unmodified,
genuine Spanfold-produced results. The API and errors must not claim to detect
every manually corrupted `ComparisonResult`.

A downstream adapter then uses the supported association directly:

```rust
for entry in result.overlap_rows_with_finality()? {
    let row = entry.row;
    let row_id = entry.metadata.row_id.as_str();
    let finality = &entry.metadata.finality;

    // Consumer-owned projection follows.
}
```

This is intentionally more explicit than `row.row_id()`: the ID and finality
come from the result that produced the row.

## Internal ownership and export changes

### One row-kind owner

`ComparisonRowKind` should be the only owner of canonical artifact labels. A
private row-family helper or macro may associate each concrete row type with its
kind, canonical collection, and finality slice. The generic hash helper must
accept `ComparisonRowKind`, not an arbitrary `&str`.

Comparator declaration text remains owned by `Comparator`, because it is a
different protocol with values such as transition, axis, and tolerance.

### Materialize metadata once

`build_row_finalities` remains the authority that creates IDs and finality. In
0.1.1 it should preserve the existing FNV-1a output for families whose labels
were already canonical. The three advanced-family JSONL IDs should be corrected
to use the canonical kind because their current cross-export disagreement is a
defect.

The private serialization fallback must not hash an empty payload. Because only
known Spanfold row DTOs reach this helper, serialization failure is an
impossible internal invariant and should be reported as such with a precise
`expect` message until the later canonical encoder removes serde from identity
generation.

### Export authoritative metadata; do not recompute it

JSON, JSON Lines, streaming JSON Lines, LLM context, and audit artifacts should
all consume the same validated row/finality pairing. In particular:

- remove `stable_row_id_for_export`;
- remove format-specific hashing calls from `append_json_lines`,
  `write_json_lines`, and `build_row_values`;
- write `metadata.row_id` and `metadata.finality` directly;
- return `ComparisonExportError::InconsistentRowMetadata` when the validated
  count/kind layout cannot provide authoritative metadata for a row;
- remove the fallback that converts missing metadata to `Final`.

Rust JSON Lines should carry the complete finality envelope rather than only
the ID:

```json
{
  "schema": "spanfold.comparison.result-row",
  "schemaVersion": 0,
  "artifact": "result-row",
  "rowType": "leadLag",
  "rowId": "leadLag:...",
  "finality": "Provisional",
  "reason": "depends on an open window clipped to the evaluation horizon",
  "version": 1,
  "supersedesRowId": null,
  "row": {}
}
```

Normalizing the three JSONL `rowType` values is a wire correction from Rust
0.1.0 and must be named in the 0.1.1 changelog. Parsers can use
`ComparisonRowKind::from_str` when reading stored 0.1.0 labels.

## Coverage and canonical collection documentation

Rustdoc should state these invariants directly:

- `CoverageRow` represents one aligned target-active segment.
- `target_magnitude` is that segment's magnitude.
- `covered_magnitude` is normally either zero or the complete segment
  magnitude, so a per-row ratio is normally `0.0` or `1.0`.
- `CoverageSummary` groups all coverage segments for the same window name, key,
  and partition. Its exact `i128` numerator and denominator are the authority
  for aggregate coverage.
- Consumers needing overall coverage must use `coverage_summaries`, not average
  or reinterpret individual segment ratios.

`ComparisonResult::rows()` should be documented as canonical grouped storage and
the canonical serialized collection. The `overlap_rows()`, `coverage_rows()`,
and other family accessors borrow typed slices from `rows()`; they do not create
duplicate storage or flat wire keys. New internal code and the typed
row/finality views should use `rows()`.

## Identity compatibility policy

### Policy for 0.1.1

For the immediate release:

- keep the hash implementation private;
- expose authoritative IDs only through result metadata and result-owned views;
- preserve existing Rust IDs for the six unaffected row families;
- correct the three advanced-family cross-export IDs and document the change;
- describe IDs as opaque deterministic identifiers for the current Rust
  artifact/schema contract;
- do not describe them as collision-proof, permanent across schema changes, or
  identical to .NET IDs;
- do not add fixed FNV digest vectors that imply the algorithm is the final
  public contract.

This gives RawScope and other Rust consumers a reliable authority now without
pretending that the underlying format is already settled.

### Shared identity task before 1.0

Before promising durable multi-version or cross-language identity, define a
repository-wide identity specification and implement it in Rust and .NET
together. The specification should include:

- an explicit identity scheme version;
- canonical `ComparisonRowKind` discriminants;
- length-framed UTF-8 strings rather than delimiter-dependent concatenation;
- fixed-width integer encoding and explicit option markers;
- canonical temporal axis, clock, range, and point encoding;
- an explicit list of identity-bearing fields for every row family;
- canonical ordering for contributing record IDs;
- SHA-256 or another agreed collision-resistant digest;
- an ID shape that includes the kind and scheme, for example
  `overlap:v1:<digest>`;
- shared fixture vectors proving byte-for-byte equality between Rust and .NET;
- an artifact schema-version change and migration note when the new IDs land.

The .NET implementation is a useful starting direction, but it is not itself a
cross-language specification. Its canonical formatter and selected fields must
be written down and reconciled with the Rust row model before Rust copies it.

## Meaningful regression coverage

Implementation should add only tests that protect the behavior being changed:

1. A cross-export association test covering all nine row families and proving
   that the ID and finality in result metadata, JSON, JSON Lines, streaming JSON
   Lines, and LLM row documents agree.
2. A live-comparison test proving a provisional typed row view remains
   provisional in JSON Lines; this protects against the current unsafe
   default-to-`Final` behavior.
3. A coverage semantics test with segment rows `0 / 2` and `2 / 2` and one
   aggregate summary `2 / 4 = 0.5`.
4. A compact parsing table for canonical row-kind labels and the three actual
   Rust 0.1.0 aliases.

Do not add generic clone/repeat tests, temporary validators, or one test per
method when the cross-family invariant test already protects the behavior. Add
fixed digest vectors only as part of the later shared identity specification.

## Consumer and package ownership

Spanfold owns:

- typed comparison rows and row kinds;
- authoritative row IDs and finality metadata;
- the association between a row and its metadata;
- aggregate coverage summaries;
- comparison artifact and streaming export contracts.

Consumers such as RawScope continue to own:

- rectangular or dataframe projections;
- optional columns used to flatten heterogeneous rows;
- coordinate normalization and visual x/y choices;
- consumer manifests, transactions, discovery, and launching;
- adapter feature flags;
- presentation labels that are not Spanfold artifact contracts;
- encoding source-record ID arrays into consumer-specific cells.

### RawScope follow-on after 0.1.1

RawScope integration is not part of the Spanfold 0.1.1 implementation. Once the
new API is published, the RawScope adapter should be updated immediately as a
separate consumer change:

1. Pin `spanfold = "=0.1.1"` while adopting the new contract.
2. Iterate through the nine `*_rows_with_finality()` methods, which are backed
   by canonical `result.rows()`, rather than reaching into row-family storage.
3. Remove every `family:index` evidence identifier.
4. Rename the projected evidence key from `rawscope_row_id` to
   `spanfold_row_id`.
5. Preserve `spanfold_row_id`, `spanfold_finality`,
   `spanfold_finality_reason`, `spanfold_row_version`, and
   `spanfold_supersedes_row_id` in the rectangular dataset.
6. Add `ComparisonRowMetadataError` transparently to `SpanfoldAdapterError`.
7. Rename the current per-row `coverage_ratio` to
   `segment_coverage_ratio`, or remove it. Any aggregate coverage value must
   come from `ComparisonResult::coverage_summaries`.
8. Retain RawScope-owned `start_offset`, duration, flattened record-ID arrays,
   session metadata, and visual projection choices.
9. Add focused integration coverage proving that authoritative IDs and
   provisional finality survive CSV materialization and RawScope loading.

The current ordinal IDs must not be presented as authoritative while RawScope
is waiting for the published Spanfold API.

## Implementation sequence

1. Add `ComparisonRowKind`, legacy parsing, typed metadata errors, and the nine
   result-owned typed row/finality views.
2. Refactor finality construction and exporters to use the typed kind and the
   stored metadata association; remove export-time identity recomputation and
   fail closed on inconsistent metadata.
3. Normalize the three advanced JSONL row kinds and include the full finality
   envelope in JSON Lines.
4. Clarify coverage and canonical row-collection rustdoc and add the focused
   behavior tests above.
5. Update the Rust 0.1.1 changelog and crate README with the consumer API and
   advanced-family ID correction. Do not publish as part of the implementation
   unless publication is separately authorized.
6. Track the shared versioned Rust/.NET identity specification as a distinct
   pre-1.0 task rather than hiding it inside this integration change.

## Delivery sequence

1. Land this design document with the Spanfold implementation rather than as a
   documentation-only release.
2. Implement and validate the Rust API and export behavior.
3. Update the existing 0.1.1 changelog entry with the typed views, JSONL
   finality envelope, and corrected advanced-family IDs.
4. If publication is explicitly authorized and 0.1.1 remains available,
   publish `spanfold 0.1.1`, wait for registry resolution, and then publish
   `spanfold-cli 0.1.1`.
5. Update RawScope against the published exact version and replace ordinal
   evidence IDs before shipping that integration.
6. Specify the shared versioned Rust/.NET identity format as a separate pre-1.0
   task.

## Acceptance criteria

- A Rust consumer can iterate any selected typed row family and obtain its
  authoritative ID and finality without ordinals, strings, hashing, JSON
  parsing, or a per-row scan of all metadata.
- For unmodified Spanfold-produced results, result JSON, JSON Lines, streaming
  JSON Lines, LLM context, and audit bundles use the exact ID stored in
  `ComparisonResult::row_finalities` for all nine row families.
- Typed views and exports reject detectable metadata count/kind layout errors
  with family and metadata-index context, without claiming to detect
  same-family manual reordering.
- JSON Lines retains finality, reason, version, and supersession metadata.
- No export path silently substitutes `Final` when authoritative metadata is
  absent.
- Comparator declaration syntax remains compatible.
- Coverage documentation directs aggregate consumers to
  `coverage_summaries`.
- Family accessors borrow from canonical `rows()` while `rows` remains the sole
  serialized row collection.
- Release notes explicitly identify corrected Rust 0.1.0 advanced-family IDs
  and accurately limit the stability claim for the current identity scheme.
