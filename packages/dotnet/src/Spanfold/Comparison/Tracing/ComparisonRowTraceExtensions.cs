namespace Spanfold.Comparison;

/// <summary>Provides typed lineage queries over comparison results.</summary>
public static class ComparisonRowTraceExtensions
{
    /// <summary>Traces a canonical row reference back to preparation and alignment evidence.</summary>
    /// <exception cref="KeyNotFoundException">The result does not contain the reference.</exception>
    public static ComparisonRowTrace TraceRow(this ComparisonResult result, ComparisonRowReference reference)
    {
        ArgumentNullException.ThrowIfNull(result);

        return reference.Kind switch
        {
            ComparisonRowKind.Overlap => FindAndTrace(result, result.OverlapRowsWithFinality(), reference),
            ComparisonRowKind.Residual => FindAndTrace(result, result.ResidualRowsWithFinality(), reference),
            ComparisonRowKind.Missing => FindAndTrace(result, result.MissingRowsWithFinality(), reference),
            ComparisonRowKind.Coverage => FindAndTrace(result, result.CoverageRowsWithFinality(), reference),
            ComparisonRowKind.Gap => FindAndTrace(result, result.GapRowsWithFinality(), reference),
            ComparisonRowKind.SymmetricDifference => FindAndTrace(result, result.SymmetricDifferenceRowsWithFinality(), reference),
            ComparisonRowKind.Containment => FindAndTrace(result, result.ContainmentRowsWithFinality(), reference),
            ComparisonRowKind.LeadLag => FindAndTrace(result, result.LeadLagRowsWithFinality(), reference),
            ComparisonRowKind.AsOf => FindAndTrace(result, result.AsOfRowsWithFinality(), reference),
            _ => throw new ArgumentOutOfRangeException(nameof(reference), reference, "Unknown comparison row kind.")
        };
    }

    /// <summary>Traces a typed row and its authoritative metadata.</summary>
    public static ComparisonRowTrace<TRow> TraceRow<TRow>(
        this ComparisonResult result,
        ComparisonRowWithFinality<TRow> row)
    {
        ArgumentNullException.ThrowIfNull(result);
        var trace = result.TraceRow(row.Metadata.Reference);
        if (trace is not ComparisonRowTrace<TRow> typed)
        {
            throw new ArgumentException("The row type does not match its canonical row reference.", nameof(row));
        }

        return typed;
    }

    private static ComparisonRowTrace<TRow> FindAndTrace<TRow>(
        ComparisonResult result,
        IEnumerable<ComparisonRowWithFinality<TRow>> rows,
        ComparisonRowReference reference)
    {
        foreach (var row in rows)
        {
            if (row.Metadata.Reference == reference)
            {
                return CreateTrace(result, row);
            }
        }

        throw new KeyNotFoundException($"Comparison row '{reference}' was not found in the result.");
    }

    private static ComparisonRowTrace<TRow> CreateTrace<TRow>(
        ComparisonResult result,
        ComparisonRowWithFinality<TRow> row)
    {
        var ids = GetRecordIds(row.Row).Distinct().ToHashSet();
        var scope = GetScope(row.Row);
        var normalized = result.Prepared?.NormalizedWindows
            .Where(window => ids.Contains(window.RecordId)) ?? [];
        var aligned = result.Aligned?.Segments
            .Where(segment => segment.TargetRecordIds.Any(ids.Contains)
                || segment.AgainstRecordIds.Any(ids.Contains)) ?? [];
        var exclusions = result.Prepared?.ExcludedWindows
            .Where(exclusion => scope.Matches(exclusion.Window)) ?? [];

        return new ComparisonRowTrace<TRow>(
            row.Row,
            row.Metadata,
            result.RecordEvidence.Where(evidence => ids.Contains(evidence.Id)),
            normalized,
            aligned,
            exclusions);
    }

    private static IEnumerable<WindowRecordId> GetRecordIds<TRow>(TRow row) => row switch
    {
        OverlapRow value => value.TargetRecordIds.Concat(value.AgainstRecordIds),
        ResidualRow value => value.TargetRecordIds,
        MissingRow value => value.AgainstRecordIds,
        CoverageRow value => value.TargetRecordIds.Concat(value.AgainstRecordIds),
        GapRow value => value.BoundaryRecordIds,
        SymmetricDifferenceRow value => value.TargetRecordIds.Concat(value.AgainstRecordIds),
        ContainmentRow value => value.TargetRecordIds.Concat(value.ContainerRecordIds),
        LeadLagRow value => Optional(value.TargetRecordId, value.ComparisonRecordId),
        AsOfRow value => Optional(value.TargetRecordId, value.MatchedRecordId),
        _ => throw new ArgumentException($"Unsupported comparison row type '{typeof(TRow).FullName}'.", nameof(row))
    };

    private static RowScope GetScope<TRow>(TRow row) => row switch
    {
        OverlapRow value => new(value.WindowName, value.Key, value.Partition),
        ResidualRow value => new(value.WindowName, value.Key, value.Partition),
        MissingRow value => new(value.WindowName, value.Key, value.Partition),
        CoverageRow value => new(value.WindowName, value.Key, value.Partition),
        GapRow value => new(value.WindowName, value.Key, value.Partition),
        SymmetricDifferenceRow value => new(value.WindowName, value.Key, value.Partition),
        ContainmentRow value => new(value.WindowName, value.Key, value.Partition),
        LeadLagRow value => new(value.WindowName, value.Key, value.Partition),
        AsOfRow value => new(value.WindowName, value.Key, value.Partition),
        _ => throw new ArgumentException($"Unsupported comparison row type '{typeof(TRow).FullName}'.", nameof(row))
    };

    private static IEnumerable<WindowRecordId> Optional(WindowRecordId required, WindowRecordId? optional)
    {
        yield return required;
        if (optional is { } value)
        {
            yield return value;
        }
    }

    private readonly record struct RowScope(string WindowName, object Key, object? Partition)
    {
        public bool Matches(WindowRecord window) =>
            StringComparer.Ordinal.Equals(WindowName, window.WindowName)
            && Equals(Key, window.Key)
            && Equals(Partition, window.Partition);
    }
}
