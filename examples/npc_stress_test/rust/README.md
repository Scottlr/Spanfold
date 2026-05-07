# Rust NPC Stress Test

Standalone Rust example for deterministic NPC schedule generation and indexed
temporal-window queries.

## Commands

```bash
cargo test
cargo run -- generate --people 10000 --seed 42
cargo run -- generate --people 100000 --seed 42
cargo run -- query chunk-at --chunk chunk2 --tick 37800
cargo run -- query district-at --district district_23 --tick 38400
cargo run -- query person-at --person 12345 --tick 37800
cargo run -- query connected-at --person 12345 --tick 37800 --json
cargo run -- report repeated-contact --person 12345 --min-count 3
```

The CLI regenerates deterministic data for each run. `generate` defaults to
10,000 people. Person-specific query/report commands automatically generate
enough people to include the requested ID unless `--people` is supplied.

## Data Model

- `TimeWindow`: activity interval for a person using `[start_tick, end_tick)`.
- `Location`: `chunk_id`, canonical district, precise location, building, room.
- `PersonConnection`: directed relationship edge with kind, strength, metadata.
- `WorldIndex`: maps person/chunk/district/activity/graph edges to compact IDs.

Common point-in-time location queries use sorted window ID lists, then stop once
the indexed windows have starts beyond the requested tick.

## Example Output

```text
$ cargo run -- query person-at --person 12345 --tick 37800
person 12345 at tick 37800: Working in district_23 / chunk2 / district_23_work_...
```

Reports are Markdown and exports are JSON Lines:

```text
artifacts/people.jsonl
artifacts/windows.jsonl
artifacts/connections.jsonl
artifacts/reports/person_12345.md
artifacts/reports/district_23.md
artifacts/reports/repeated_contact_12345.md
```

## Benchmarks

This standalone example avoids introducing benchmark dependencies. Use CLI
timing output for basic smoke timings:

```bash
cargo run --release -- generate --people 100000 --seed 42
```
