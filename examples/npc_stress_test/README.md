# NPC Temporal-Window Stress Test

This showcase models a simulated city day for thousands of NPCs using
Spanfold-style temporal windows. Each generated person receives a deterministic
routine made of half-open time windows `[start_tick, end_tick)`.

The example is intentionally local to `examples/` because it is a stress-test
and teaching artifact, not a reusable production crate.

## What It Demonstrates

- People as compact `PersonId` values.
- Daily routines as temporal windows.
- First-class query dimensions for person, chunk, canonical district, and
  activity.
- Segments/tags/metadata-style fields for reporting.
- Point-in-time queries such as "who is in chunk2 at 10:35?".
- Range-overlap queries for co-location and repeated contact.
- Relationship graph queries for connected people.
- JSONL and Markdown report exports.

## Rust Demo

```bash
cd examples/npc_stress_test/rust
cargo run -- generate --people 10000 --seed 42
cargo run -- query chunk-at --chunk chunk2 --tick 37800
cargo run -- query district-at --district district_23 --tick 37800
cargo run -- query person-at --person 12345 --tick 37800
cargo run -- query connected-at --person 12345 --tick 37800 --json
cargo run -- report repeated-contact --person 12345 --min-count 3
```

Generated files are written under:

```text
examples/npc_stress_test/rust/artifacts/
```

## Mapping To Spanfold Concepts

- Window: one NPC activity over `[start_tick, end_tick)`.
- Segment: hot query dimensions like chunk, canonical district, and activity.
- Tags: archetype and descriptive labels.
- Metadata: deterministic routine context useful for debug reports.
- Point-in-time query: indexed `active_at(tick)` lookup.
- Overlap query: co-location and repeated-contact analysis.

## Current Limitations

- The demo uses an in-memory index only.
- It deliberately avoids async, networking, databases, and game-engine APIs.
- It follows Spanfold concepts but does not depend on the production Rust
  Spanfold crate yet.

Future work can map the domain records onto the real Rust Spanfold API once the
example graduates from showcase to reusable integration.
