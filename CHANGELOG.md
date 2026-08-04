# Changelog

## Unreleased

- Add a caller-driven .NET bounded-watermark tracker with per-lane event-time
  progress, deterministic buffering, bounded late correction, and stable
  revision/retraction references for downstream changelog and finality flows.
- Add .NET and Rust episode formation with position or timestamp normalization,
  side-local gap stitching, retained fragments, envelopes, and active magnitude.
- Add complete component-graph episode relations for one-to-one, split, merge,
  complex, and unmatched occurrences.
- Preserve known-at filtering and horizon-relative provisional finality for live
  episode analysis without claiming watermark or late-record completeness.
- Add neutral episode summaries and explicit reference-oriented recall and
  precision through `AsReference()`.
- Expose Episode formation, exhaustive relation graphs, summaries, and explicit
  reference scorecards through both the .NET and Rust library APIs.

## 0.2.0-preview.1

- Organize comparison, liveness, assessment, revisions, and artifacts into
  explicit C# namespaces and packages.
- Add canonical typed row references, row lineage traces, semantic snapshot
  revisions, portable assessment rules and suites, and assertion helpers.
- Move export and explanation APIs into the optional `Spanfold.Artifacts`
  package with atomic audit bundles, disclosure profiles, parsed artifact
  models, and SHA-256 integrity verification.
- Add CLI `check`, `suite`, `verify-bundle`, and `diff` workflows.
- Replace the pipeline entry point with `EventPipeline.For<TEvent>()`; the old
  entry point remains obsolete for source migration.
- Remove comparison-run output options so execution remains free of file I/O.

## 0.1.0-preview.2

- Render the package README logo using CommonMark image syntax supported by
  NuGet.org.

## 0.1.0-preview.1

- Keep the .NET library, testing package, and CLI on one centrally managed
  preview version.
- Document compatibility rules for comparison exports and CLI fixtures.
- Initial preview release.
