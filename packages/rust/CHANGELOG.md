# Rust changelog

## Unreleased

- Add the public `WindowRecorder` and `WindowObservation` API for recording
  lifecycle transitions at explicit temporal points, including deterministic
  IDs, history, segment boundaries, and tag updates.
- Make snapshot finality records-owned with `WindowSnapshotFinality`, containing
  only `Final` and `Provisional`. Migrate callers from
  `WindowSnapshotRecord.finality: ComparisonFinality` to the new type; the
  serialized values remain exactly `"Final"` and `"Provisional"`, and higher
  comparison layers translate snapshot finality at their boundary.
- Replace `WindowArtifact`'s scalar `startPosition`, `endPosition`, and
  `knownAtPosition` fields with typed `start`, `end`, and `knownAt` temporal
  points. Consumers must migrate to the new source fields and wire keys, which
  now preserve axis and timestamp-clock identity.
- Make `LaneKey` construction fallible and its fields private so empty lane or
  partition identities cannot enter liveness state. Callers must migrate direct
  construction and `From` conversions to `LaneKey::new` or
  `LaneKey::with_partition` and handle `LaneKeyError`.
- Replace the raw `PrimitiveValue::Float(f64)` payload with validated
  `FiniteFloat`. Callers must migrate direct construction and pattern matches
  to `PrimitiveValue::try_float`, `FiniteFloat::try_new`, and `as_f64`.
- Replace the nine public `ComparisonResult` row fields with borrowed accessors:
  migrate `overlap_rows`, `residual_rows`, `missing_rows`, `coverage_rows`,
  `gap_rows`, `symmetric_difference_rows`, `containment_rows`, `lead_lag_rows`,
  and `as_of_rows` to `*_rows()` (or use `rows().<family>`). The grouped
  serialized `rows` collection is unchanged.
- Add Episode formation over normalized window evidence, same-side gap
  stitching, exhaustive cross-side relation graphs, neutral summaries, and
  explicit reference scorecards.

## 0.1.1

- Give the library and CLI distinct crates.io documentation focused on their
  respective Rust API and command-line workflows.
- Add `ComparisonRowKind` and nine result-owned typed row/finality views so
  consumers can borrow authoritative row IDs and finality without positional
  IDs, hashing, JSON parsing, or per-row metadata scans.
- Make JSON, JSON Lines, streaming JSON Lines, and LLM-context row documents
  consume the stored row/finality association and fail closed on detectable
  metadata count or kind corruption instead of substituting `Final`.
- Include reason, version, and supersession metadata in JSON Lines, and correct
  the Rust 0.1.0 advanced-family labels and IDs from `symmetric-difference`,
  `lead-lag`, and `asof` to `symmetricDifference`, `leadLag`, and `asOf`.
- Clarify that coverage rows describe individual aligned segments while
  `coverage_summaries` owns grouped aggregate coverage.
- Treat current Rust row IDs as opaque deterministic identifiers for the
  current artifact/schema contract; cross-version and Rust/.NET identity
  compatibility remain a separate pre-1.0 specification task.

## 0.1.0

- Add event-driven state-window recording with hierarchical roll-ups, segment
  projections, callbacks, batch ingestion, and processing/event-time axes.
- Add staged temporal comparison plans, selectors, normalization, alignment,
  core and advanced comparators, cohorts, source matrices, and hierarchy
  analytics.
- Add deterministic JSON, JSON Lines, Markdown, debug HTML, LLM-context, and
  audit-bundle exports.
- Add fixture, event import, comparison, and audit CLI workflows.
- Add liveness, known-at, live finality, changelog, testing, snapshot, and
  virtual-clock helpers.
