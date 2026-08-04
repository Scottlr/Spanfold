namespace Spanfold.Sequences;

/// <summary>
/// Represents one deterministic ordered match and its source window evidence.
/// </summary>
public sealed class WindowSequenceMatch
{
    internal WindowSequenceMatch(
        object key,
        object? source,
        object? partition,
        IReadOnlyList<WindowSnapshotRecord> evidence,
        long startPosition,
        long endPosition,
        long totalGap,
        ComparisonFinality finality)
    {
        Key = key;
        Source = source;
        Partition = partition;
        Evidence = Array.AsReadOnly(evidence.ToArray());
        StartPosition = startPosition;
        EndPosition = endPosition;
        EndToEndPositionMagnitude = endPosition - startPosition;
        TotalGapPositionMagnitude = totalGap;
        Finality = finality;
    }

    /// <summary>Gets the first-step key that anchors correlation.</summary>
    public object Key { get; }

    /// <summary>Gets the exact source identity shared by every step.</summary>
    public object? Source { get; }

    /// <summary>Gets the exact partition identity shared by every step.</summary>
    public object? Partition { get; }

    /// <summary>Gets the ordered snapshot evidence, one record per configured step.</summary>
    public IReadOnlyList<WindowSnapshotRecord> Evidence { get; }

    /// <summary>Gets the first step onset processing position.</summary>
    public long StartPosition { get; }

    /// <summary>Gets the latest effective end across all selected steps.</summary>
    public long EndPosition { get; }

    /// <summary>Gets the processing-position length of the sequence envelope.</summary>
    public long EndToEndPositionMagnitude { get; }

    /// <summary>Gets the sum of positive inactive gaps between consecutive steps.</summary>
    public long TotalGapPositionMagnitude { get; }

    /// <summary>Gets whether every selected step is final at the evaluation horizon.</summary>
    public ComparisonFinality Finality { get; }
}
