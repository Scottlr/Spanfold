namespace Spanfold.Comparison;

/// <summary>
/// Describes the authoritative lineage used to produce one comparison row.
/// </summary>
public abstract class ComparisonRowTrace
{
    private protected ComparisonRowTrace(
        ComparisonRowFinality metadata,
        IEnumerable<WindowRecordEvidence> contributingRecords,
        IEnumerable<NormalizedWindowRecord> normalizedWindows,
        IEnumerable<AlignedSegment> alignedSegments,
        IEnumerable<ExcludedWindowRecord> relevantExclusions)
    {
        Metadata = metadata;
        ContributingRecords = Materialize(contributingRecords);
        NormalizedWindows = Materialize(normalizedWindows);
        AlignedSegments = Materialize(alignedSegments);
        RelevantExclusions = Materialize(relevantExclusions);
    }

    /// <summary>Gets the canonical row reference.</summary>
    public ComparisonRowReference Reference => Metadata.Reference;

    /// <summary>Gets authoritative finality metadata for the row.</summary>
    public ComparisonRowFinality Metadata { get; }

    /// <summary>Gets source evidence directly referenced by the row.</summary>
    public IReadOnlyList<WindowRecordEvidence> ContributingRecords { get; }

    /// <summary>Gets normalized windows directly referenced by the row.</summary>
    public IReadOnlyList<NormalizedWindowRecord> NormalizedWindows { get; }

    /// <summary>Gets aligned segments supported by the row's source records.</summary>
    public IReadOnlyList<AlignedSegment> AlignedSegments { get; }

    /// <summary>Gets preparation exclusions in the same logical row scope.</summary>
    public IReadOnlyList<ExcludedWindowRecord> RelevantExclusions { get; }

    private static IReadOnlyList<T> Materialize<T>(IEnumerable<T> values) =>
        Array.AsReadOnly(values.ToArray());
}

/// <summary>
/// Describes the authoritative lineage for one strongly typed comparison row.
/// </summary>
/// <typeparam name="TRow">The comparison row type.</typeparam>
public sealed class ComparisonRowTrace<TRow> : ComparisonRowTrace
{
    internal ComparisonRowTrace(
        TRow row,
        ComparisonRowFinality metadata,
        IEnumerable<WindowRecordEvidence> contributingRecords,
        IEnumerable<NormalizedWindowRecord> normalizedWindows,
        IEnumerable<AlignedSegment> alignedSegments,
        IEnumerable<ExcludedWindowRecord> relevantExclusions)
        : base(metadata, contributingRecords, normalizedWindows, alignedSegments, relevantExclusions)
    {
        Row = row;
    }

    /// <summary>Gets the typed comparison row.</summary>
    public TRow Row { get; }
}
