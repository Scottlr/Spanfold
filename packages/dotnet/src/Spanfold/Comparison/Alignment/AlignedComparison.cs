namespace Spanfold;

/// <summary>
/// Represents a prepared comparison after temporal segment alignment.
/// </summary>
/// <remarks>
/// Aligned comparisons split normalized windows at every relevant boundary so
/// comparators can reason over one deterministic segment at a time.
/// </remarks>
/// <param name="Prepared">The prepared comparison input.</param>
/// <param name="Segments">The deterministic aligned segments.</param>
public sealed record AlignedComparison
{
    internal AlignedComparison(PreparedComparison prepared, IReadOnlyList<AlignedSegment> segments)
    {
        Prepared = prepared;
        Segments = segments;
    }

    /// <summary>Gets the prepared comparison input.</summary>
    public PreparedComparison Prepared { get; }
    /// <summary>Gets the deterministic aligned segments.</summary>
    public IReadOnlyList<AlignedSegment> Segments { get; }
}
