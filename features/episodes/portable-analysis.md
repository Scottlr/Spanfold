# Portable Episode analysis

`spanfold.episode.analysis` schema version 1 describes one comparison between a
target source and an against source for one named window family. It compiles
into the existing Episode builders in both runtimes; the document is not a
second analytical implementation.

```json
{
  "schema": "spanfold.episode.analysis",
  "schemaVersion": 1,
  "name": "Provider and detector offline episodes",
  "target": { "name": "provider", "source": "provider-a" },
  "against": { "name": "detector", "source": "detector-b" },
  "windowName": "DeviceOffline",
  "normalizationAxis": "processingPosition",
  "stitchTolerance": 1,
  "relationTolerance": 0,
  "liveHorizon": 14
}
```

Version 1 intentionally supports only processing positions. A timestamp
version needs an explicit clock identity and canonical timestamp representation
before it can be portable. `liveHorizon` is optional; when present it clips open
windows and preserves provisional Episode and relation finality. Tolerances and
the live horizon must be non-negative.

Portable identities are deliberately narrow: keys are strings and partitions
are strings or null. Episode arrays order keys and non-null partitions by their
lexicographic UTF-8 bytes, with null partitions before strings, then use the
episode's temporal and materialized fields for a total order. Relation indexes
refer to that portable order rather than either runtime's native Episode order.

Both runtimes emit `spanfold.episode.analysis.result` JSON or a Markdown view of
the same counts, occurrences, and exhaustive relation components. Results use
zero-based per-side Episode indexes. They deliberately do not expose or compare
runtime Episode IDs because those IDs have runtime-specific identity contracts.
Result JSON uses one canonical string representation: quotation marks and
reverse solidus are escaped, backspace, form feed, line feed, carriage return,
and tab use their short escapes, and remaining U+0000 through U+001F controls
use lowercase `\u00xx`. Every other valid Unicode scalar is emitted directly as
UTF-8, including private-use and supplementary characters.
Markdown renders keys and partitions as JSON literals, so a null partition is
`null` while the string value is `"null"`.

Run the shared provider/detector example with either CLI:

```bash
spanfold episodes \
  features/episodes/fixtures/portable-provider-detector-plan.json \
  features/episodes/fixtures/portable-provider-detector-windows.jsonl \
  --format json
```

The shared expected result is
[`portable-provider-detector-result.json`](fixtures/portable-provider-detector-result.json).
