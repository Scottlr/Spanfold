namespace Spanfold.Comparison;

/// <summary>
/// Describes finality metadata for a materialized comparison row.
/// </summary>
/// <param name="Reference">The canonical row family and opaque row identifier.</param>
/// <param name="Finality">Whether the row is final or provisional.</param>
/// <param name="Reason">A short human-readable finality reason.</param>
/// <param name="Version">The deterministic row metadata version.</param>
/// <param name="SupersedesRowId">The prior row identifier superseded by this metadata, when any.</param>
///
/// Row IDs belong to the current C# artifact/schema identity contract. Consumers
/// should preserve them rather than recomputing the private identity algorithm
/// or assuming parity with Rust identifiers.
public sealed record ComparisonRowFinality(
    ComparisonRowReference Reference,
    ComparisonFinality Finality,
    string Reason,
    int Version = 1,
    string? SupersedesRowId = null)
{
    /// <summary>Gets the canonical row family.</summary>
    public ComparisonRowKind RowKind => Reference.Kind;

    /// <summary>Gets the canonical exported row-family label.</summary>
    public string RowType => Reference.Kind.ToArtifactLabel();

    /// <summary>Gets the opaque deterministic row identifier.</summary>
    public string RowId => Reference.RowId;
}
