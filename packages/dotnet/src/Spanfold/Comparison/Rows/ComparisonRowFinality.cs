namespace Spanfold;

/// <summary>
/// Describes finality metadata for a materialized comparison row.
/// </summary>
/// <param name="RowType">The canonical exported row family, such as overlap or residual.</param>
/// <param name="RowId">The opaque deterministic row identifier assigned by the producing result.</param>
/// <param name="Finality">Whether the row is final or provisional.</param>
/// <param name="Reason">A short human-readable finality reason.</param>
/// <param name="Version">The deterministic row metadata version.</param>
/// <param name="SupersedesRowId">The prior row identifier superseded by this metadata, when any.</param>
///
/// Row IDs belong to the current C# artifact/schema identity contract. Consumers
/// should preserve them rather than recomputing the private identity algorithm
/// or assuming parity with Rust identifiers.
public sealed record ComparisonRowFinality(
    string RowType,
    string RowId,
    ComparisonFinality Finality,
    string Reason,
    int Version = 1,
    string? SupersedesRowId = null);
