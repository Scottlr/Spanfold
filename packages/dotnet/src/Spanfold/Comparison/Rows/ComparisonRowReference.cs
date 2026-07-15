namespace Spanfold.Comparison;

/// <summary>
/// Identifies one materialized comparison row within its row family.
/// </summary>
public readonly record struct ComparisonRowReference
{
    /// <summary>
    /// Creates a comparison row reference.
    /// </summary>
    /// <param name="kind">The closed comparison row family.</param>
    /// <param name="rowId">The opaque deterministic row identifier.</param>
    public ComparisonRowReference(ComparisonRowKind kind, string rowId)
    {
        if (!Enum.IsDefined(kind))
        {
            throw new ArgumentOutOfRangeException(nameof(kind), kind, "Unknown comparison row kind.");
        }

        ArgumentException.ThrowIfNullOrWhiteSpace(rowId);
        Kind = kind;
        RowId = rowId;
    }

    /// <summary>Gets the closed comparison row family.</summary>
    public ComparisonRowKind Kind { get; }

    /// <summary>Gets the opaque deterministic row identifier.</summary>
    public string RowId { get; }

    /// <inheritdoc />
    public override string ToString()
    {
        return Kind.ToArtifactLabel() + ":" + RowId;
    }
}
