# Spanfold Design Notes

Spanfold models windows as state transitions rather than as time buckets.

A source window opens when the configured `isActive` selector changes from false
to true for a key. It closes when that selector changes back to false.

Roll-ups observe child activity and apply a parent predicate to a compact child
activity view. This keeps the public API small while allowing the runtime to own
the necessary state tracking.

The builder surface is intentionally narrow:

- `EventPipeline.For<TEvent>()`
- `Window(...)`
- `RollUp(...)`
- `Build()`
- optional ingestion and analysis settings

Window history, overlap queries, and residual analysis live behind runtime history
objects. They do not add extra concepts to the main window-definition API.
