using Spanfold.Comparison;

namespace Spanfold.Revisions;

/// <summary>
/// Creates and replays deterministic row changelogs between comparison snapshots.
/// </summary>
public static class ComparisonChangelog
{
    /// <summary>
    /// Creates changelog entries from one snapshot's row metadata to another.
    /// </summary>
    public static IReadOnlyList<ComparisonChangelogEntry> Create(
        IEnumerable<ComparisonRowFinality> previous,
        IEnumerable<ComparisonRowFinality> current)
    {
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(current);

        var previousByRow = previous.ToDictionary(static metadata => metadata.Reference);
        var currentByRow = current.ToDictionary(static metadata => metadata.Reference);
        var entries = new List<ComparisonChangelogEntry>();

        foreach (var currentMetadata in Order(currentByRow.Values))
        {
            if (!previousByRow.TryGetValue(currentMetadata.Reference, out var previousMetadata))
            {
                entries.Add(new ComparisonChangelogEntry(
                    currentMetadata.Reference,
                    currentMetadata.Version,
                    ComparisonRevisionKind.Added,
                    PreviousFinality: null,
                    currentMetadata.Finality,
                    currentMetadata.SupersedesRowId,
                    currentMetadata.Reason));
                continue;
            }

            if (previousMetadata.Finality == currentMetadata.Finality
                && string.Equals(previousMetadata.Reason, currentMetadata.Reason, StringComparison.Ordinal))
            {
                continue;
            }

            entries.Add(new ComparisonChangelogEntry(
                currentMetadata.Reference,
                previousMetadata.Version + 1,
                ComparisonRevisionKind.Revised,
                previousMetadata.Finality,
                currentMetadata.Finality,
                previousMetadata.RowId,
                currentMetadata.Reason));
        }

        foreach (var previousMetadata in Order(previousByRow.Values))
        {
            if (currentByRow.ContainsKey(previousMetadata.Reference))
            {
                continue;
            }

            entries.Add(new ComparisonChangelogEntry(
                previousMetadata.Reference,
                previousMetadata.Version + 1,
                ComparisonRevisionKind.Retracted,
                previousMetadata.Finality,
                CurrentFinality: null,
                previousMetadata.RowId,
                "Row was not emitted by the current snapshot."));
        }

        return entries.ToArray();
    }

    /// <summary>
    /// Replays changelog entries over a previous row-metadata snapshot.
    /// </summary>
    public static IReadOnlyList<ComparisonRowFinality> Replay(
        IEnumerable<ComparisonRowFinality> previous,
        IEnumerable<ComparisonChangelogEntry> entries)
    {
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(entries);

        var active = previous.ToDictionary(static metadata => metadata.Reference);
        foreach (var entry in entries
                     .OrderBy(static entry => entry.Row.Kind)
                     .ThenBy(static entry => entry.Row.RowId, StringComparer.Ordinal)
                     .ThenBy(static entry => entry.Version))
        {
            if (entry.Kind == ComparisonRevisionKind.Retracted)
            {
                active.Remove(entry.Row);
                continue;
            }

            var currentFinality = entry.CurrentFinality
                ?? throw new ArgumentException(
                    "Added and revised changelog entries require a current finality.",
                    nameof(entries));
            active[entry.Row] = new ComparisonRowFinality(
                entry.Row,
                currentFinality,
                entry.Reason,
                entry.Version,
                entry.SupersedesRowId);
        }

        return Order(active.Values).ToArray();
    }

    private static IOrderedEnumerable<ComparisonRowFinality> Order(
        IEnumerable<ComparisonRowFinality> metadata)
    {
        return metadata
            .OrderBy(static row => row.Reference.Kind)
            .ThenBy(static row => row.Reference.RowId, StringComparer.Ordinal);
    }
}
