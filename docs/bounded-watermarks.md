# Bounded watermarks and late correction

`BoundedWatermarkTracker` is a caller-driven .NET primitive for deciding when
event-time revisions can enter downstream temporal analysis. It is deliberately
separate from `EventPipeline<TEvent>`: applications opt in, report progress for
each lane, and route accepted or corrected revisions into their own pipeline.

For a configured nonnegative allowed lateness `L`, lane progress `P` produces
the monotonic watermark `W = P - L`.

- Events ahead of `P` are buffered until progress reaches them.
- Events from `W` through `P`, including the boundary at `W`, are accepted.
- Events before `W` are rejected unless they replace a retained accepted
  revision of the same stable event ID.
- A correction names both the stable replacement revision and the exact prior
  revision to retract. These references can be mapped into comparison
  changelogs or used to trigger a new live/finality snapshot.
- Revision identities remain eligible for correction for one additional `L`
  interval behind `W`. After that bounded horizon, they are evicted and a late
  replacement is rejected.

Progress and state are isolated by the caller-supplied lane ID. Buffered events
released by one advancement are ordered by event time, event ID, then revision
ID, all using ordinal identity ordering.

## Objective limitations

This API is an in-memory, single-writer coordination primitive. It does not
claim general or distributed watermark completeness. In particular, it does
not discover source partitions, infer that a source is complete, advance idle
lanes, persist or replicate progress, schedule timers, or rewire ingestion.
Callers must provide trustworthy lane progress, serialize calls, remove lanes
whose lifecycle has ended, and bound input volume when progress stalls. The
current surface is .NET-only; no cross-runtime wire contract is claimed.
