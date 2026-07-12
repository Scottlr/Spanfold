# Fixture Schema

Spanfold CLI fixtures are compact JSON files for portable comparison examples and
agent-readable regression cases.

## Shape

JSON examples are left uncommented because JSON fixtures must remain valid JSON.

```json
{
  "schema": "spanfold.contract-fixture",
  "schemaVersion": 1,
  "name": "basic-overlap",
  "windows": [
    {
      "windowName": "DeviceOffline",
      "key": "device-1",
      "source": "provider-a",
      "partition": "fleet-a",
      "startPosition": 1,
      "endPosition": 5,
      "segments": [
        { "name": "lifecycle", "value": "Incident" },
        { "name": "stage", "value": "Escalated", "parentName": "lifecycle" }
      ],
      "tags": [
        { "name": "fleet", "value": "critical" }
      ]
    }
  ],
  "plan": {
    "name": "Provider QA",
    "targetSource": "provider-a",
    "againstSources": [ "provider-b" ],
    "scopeWindow": "DeviceOffline",
    "scopeSegments": [
      { "name": "lifecycle", "value": "Incident" }
    ],
    "scopeTags": [
      { "name": "fleet", "value": "critical" }
    ],
    "comparators": [ "overlap", "residual", "coverage" ],
    "strict": false
  }
}
```

Use `againstCohort` instead of `againstSources` when the comparison side is a
group:

```json
{
  "againstCohort": {
    "name": "cohort",
    "sources": [ "provider-b", "provider-c" ],
    "activity": "any"
  }
}
```

Supported cohort activities are `any`, `all`, `none`, `at-least`, `at-most`,
and `exactly`. Threshold activities use `count`.

## Open Windows

Use `null` for `endPosition` when a window is still open:

```json
{
  "windowName": "DeviceOffline",
  "key": "device-1",
  "source": "provider-a",
  "startPosition": 10,
  "endPosition": null
}
```

Historical runs keep Spanfold's normal safety behavior. Live fixture runs should add
`liveHorizonPosition` to the plan.

## Commands

```bash
spanfold validate-plan fixture.json # Validate the fixture plan.
spanfold compare fixture.json --format json # Execute and export JSON.
spanfold compare fixture.json --format llm-context # Execute and export agent-readable context.
spanfold explain fixture.json # Execute and export Markdown.
spanfold audit fixture.json --out artifacts/spanfold-audit # Write JSON, Markdown, debug HTML, LLM context, and a manifest.
```

For lower-setup audits, use flat JSON Lines where each line is one recorded
window. This avoids writing the full fixture envelope and plan by hand.

```json
{"key":"device-1","source":"provider-a","startPosition":1,"endPosition":5}
{"key":"device-1","source":"provider-b","startPosition":3,"endPosition":7}
```

```bash
spanfold audit-windows windows.jsonl --window DeviceOffline --target provider-a --against provider-b --out artifacts/spanfold-audit
```

`audit-windows` accepts optional `windowName`, `partition`, `segments`, and
`tags` fields on each JSONL row using the same names as fixture windows. Use
`--comparators overlap,residual,coverage`, repeated `--against` values, and
`--live-horizon-position 100` when needed.

The CLI validates required fixture properties before execution and returns JSON
diagnostics on standard error for malformed fixtures.
