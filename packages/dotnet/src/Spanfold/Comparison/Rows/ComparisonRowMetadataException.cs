namespace Spanfold;

/// <summary>
/// Indicates that result rows and row-finality metadata do not share the
/// expected canonical count or family layout.
/// </summary>
public sealed class ComparisonRowMetadataException : InvalidOperationException
{
    /// <summary>
    /// Creates an exception containing the first detectable layout mismatch.
    /// </summary>
    public ComparisonRowMetadataException(
        ComparisonRowKind family,
        int metadataIndex,
        int expectedCount,
        int actualCount,
        ComparisonRowKind expectedKind,
        string? actualKind)
        : base(CreateMessage(family, metadataIndex, expectedCount, actualCount, expectedKind, actualKind))
    {
        Family = family;
        MetadataIndex = metadataIndex;
        ExpectedCount = expectedCount;
        ActualCount = actualCount;
        ExpectedKind = expectedKind;
        ActualKind = actualKind;
    }

    /// <summary>Gets the family being validated.</summary>
    public ComparisonRowKind Family { get; }

    /// <summary>Gets the absolute metadata index where validation failed.</summary>
    public int MetadataIndex { get; }

    /// <summary>Gets the expected count for the failing family.</summary>
    public int ExpectedCount { get; }

    /// <summary>Gets the observed count for the failing family layout span.</summary>
    public int ActualCount { get; }

    /// <summary>Gets the expected family at the failing metadata index.</summary>
    public ComparisonRowKind ExpectedKind { get; }

    /// <summary>Gets the raw observed family label, or null when metadata is absent.</summary>
    public string? ActualKind { get; }

    private static string CreateMessage(
        ComparisonRowKind family,
        int metadataIndex,
        int expectedCount,
        int actualCount,
        ComparisonRowKind expectedKind,
        string? actualKind)
    {
        return $"Inconsistent {family} row metadata at index {metadataIndex}: "
            + $"expected {expectedCount} {expectedKind} records, found {actualCount}; "
            + $"actual kind: {actualKind ?? "<missing>"}.";
    }
}
