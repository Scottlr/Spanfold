# Spanfold CLI

`spanfold-cli` provides the `spanfold` command for validating comparison plans,
comparing temporal fixtures, importing event streams, and producing reproducible
audit artifacts.

## Install

```bash
cargo install spanfold-cli
```

Confirm the installation:

```bash
spanfold --version
spanfold --help
```

## Commands

| Command | Purpose |
| --- | --- |
| `validate-plan` | Validate a Spanfold fixture plan |
| `compare` | Run a fixture comparison and select an output format |
| `explain` | Render a fixture explanation as Markdown |
| `audit` | Produce a complete audit artifact bundle from a fixture |
| `audit-windows` | Audit flat window records supplied as JSON Lines |
| `import-events` | Convert mapped JSON Lines or CSV events into window records |
| `audit-events` | Import mapped events and produce an audit bundle in one step |

Run `spanfold <command> --help` for command-specific arguments.

## Examples

Compare a fixture and emit JSON:

```bash
spanfold compare comparison.json --format json
```

Import event records using a declarative mapping:

```bash
spanfold import-events events.jsonl --map import-map.json --out windows.jsonl
```

Create an audit bundle directly from events:

```bash
spanfold audit-events events.csv \
  --map csv-import-map.json \
  --target provider-a \
  --against provider-b \
  --out artifacts/audit
```

Import mappings use field paths or CSV column names and fixed comparison
predicates; they do not execute user-supplied code.

The CLI uses the [`spanfold`](https://crates.io/crates/spanfold) library. See the
[repository](https://github.com/Scottlr/Spanfold) and
[Rust changelog](https://github.com/Scottlr/Spanfold/blob/main/packages/rust/CHANGELOG.md)
for fixture schemas, source, and release notes.
