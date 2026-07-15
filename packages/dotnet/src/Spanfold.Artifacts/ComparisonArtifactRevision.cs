namespace Spanfold.Artifacts;

/// <summary>Describes row identity and finality changes between comparison artifacts.</summary>
public sealed class ComparisonArtifactRevision
{
    private ComparisonArtifactRevision(IEnumerable<ComparisonChangelogEntry> rows)
    {
        Rows = Array.AsReadOnly(rows.ToArray());
    }

    /// <summary>Gets deterministic row changes.</summary>
    public IReadOnlyList<ComparisonChangelogEntry> Rows { get; }

    /// <summary>Gets whether both artifacts expose equivalent row metadata.</summary>
    public bool IsEmpty => Rows.Count == 0;

    /// <summary>Compares canonical row metadata without reconstructing runtime results.</summary>
    public static ComparisonArtifactRevision Between(ComparisonArtifact previous, ComparisonArtifact current)
    {
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(current);
        var before = previous.RowMetadata.Select(static row => new ComparisonRowFinality(
            row.Reference, row.Finality, "Parsed artifact metadata.", row.Version));
        var after = current.RowMetadata.Select(static row => new ComparisonRowFinality(
            row.Reference, row.Finality, "Parsed artifact metadata.", row.Version));
        return new ComparisonArtifactRevision(ComparisonChangelog.Create(before, after));
    }
}
