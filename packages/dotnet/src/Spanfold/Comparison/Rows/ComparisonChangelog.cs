namespace Spanfold;

/// <summary>
/// Creates and replays deterministic row-finality changelogs between snapshots.
/// </summary>
/// <remarks>
/// Changelogs are optional metadata. They do not store duplicate row payloads;
/// they describe which row-finality records were added, revised, or retracted
/// so live consumers can audit changing current-state output.
/// </remarks>
public static class ComparisonChangelog
{
    /// <summary>
    /// Creates changelog entries from one snapshot's row-finality metadata to another.
    /// </summary>
    /// <param name="previous">The previous snapshot row-finality metadata.</param>
    /// <param name="current">The current snapshot row-finality metadata.</param>
    /// <returns>Deterministic changelog entries sorted by row type and row identifier.</returns>
    public static IReadOnlyList<ComparisonChangelogEntry> Create(
        IEnumerable<ComparisonRowFinality> previous,
        IEnumerable<ComparisonRowFinality> current)
    {
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(current);

        var previousByKey = previous.ToDictionary(static row => CreateKey(row.RowType, row.RowId), StringComparer.Ordinal);
        var currentByKey = current.ToDictionary(static row => CreateKey(row.RowType, row.RowId), StringComparer.Ordinal);
        var entries = new List<ComparisonChangelogEntry>();

        foreach (var currentRow in currentByKey.Values.OrderBy(static row => row.RowType, StringComparer.Ordinal).ThenBy(static row => row.RowId, StringComparer.Ordinal))
        {
            var key = CreateKey(currentRow.RowType, currentRow.RowId);
            if (!previousByKey.TryGetValue(key, out var previousRow))
            {
                entries.Add(new ComparisonChangelogEntry(
                    currentRow.RowType,
                    currentRow.RowId,
                    currentRow.Version,
                    currentRow.Finality,
                    currentRow.SupersedesRowId,
                    currentRow.Reason));
                continue;
            }

            if (previousRow.Finality == currentRow.Finality
                && string.Equals(previousRow.Reason, currentRow.Reason, StringComparison.Ordinal))
            {
                continue;
            }

            entries.Add(new ComparisonChangelogEntry(
                currentRow.RowType,
                currentRow.RowId,
                previousRow.Version + 1,
                ComparisonFinality.Revised,
                previousRow.RowId,
                currentRow.Reason,
                currentRow.Finality));
        }

        foreach (var previousRow in previousByKey.Values.OrderBy(static row => row.RowType, StringComparer.Ordinal).ThenBy(static row => row.RowId, StringComparer.Ordinal))
        {
            if (currentByKey.ContainsKey(CreateKey(previousRow.RowType, previousRow.RowId)))
            {
                continue;
            }

            entries.Add(new ComparisonChangelogEntry(
                previousRow.RowType,
                previousRow.RowId,
                previousRow.Version + 1,
                ComparisonFinality.Retracted,
                previousRow.RowId,
                "Row was not emitted by the current snapshot."));
        }

        return entries.ToArray();
    }

    /// <summary>
    /// Replays changelog entries over a previous row-finality snapshot.
    /// </summary>
    /// <param name="previous">The previous active row-finality metadata.</param>
    /// <param name="entries">The changelog entries to replay.</param>
    /// <returns>The active row-finality metadata after applying all non-retracted changes.</returns>
    public static IReadOnlyList<ComparisonRowFinality> Replay(
        IEnumerable<ComparisonRowFinality> previous,
        IEnumerable<ComparisonChangelogEntry> entries)
    {
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(entries);

        var active = previous.ToDictionary(static row => CreateKey(row.RowType, row.RowId), StringComparer.Ordinal);
        foreach (var entry in entries.OrderBy(static entry => entry.RowType, StringComparer.Ordinal).ThenBy(static entry => entry.RowId, StringComparer.Ordinal).ThenBy(static entry => entry.Version))
        {
            var key = CreateKey(entry.RowType, entry.RowId);
            if (entry.Finality == ComparisonFinality.Retracted)
            {
                active.Remove(key);
                continue;
            }

            var resultingFinality = entry.ResultFinality
                ?? (entry.Finality == ComparisonFinality.Revised
                    ? ComparisonFinality.Final
                    : entry.Finality);
            active[key] = new ComparisonRowFinality(
                entry.RowType,
                entry.RowId,
                resultingFinality,
                entry.Reason,
                entry.Version,
                entry.SupersedesRowId);
        }

        return active.Values
            .OrderBy(static row => row.RowType, StringComparer.Ordinal)
            .ThenBy(static row => row.RowId, StringComparer.Ordinal)
            .ToArray();
    }

    private static string CreateKey(string rowType, string rowId)
    {
        return rowType + "\n" + rowId;
    }
}
