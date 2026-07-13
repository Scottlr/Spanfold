using Spanfold.Internal.Comparison;

namespace Spanfold;

/// <summary>
/// Provides small query helpers over materialized comparison results.
/// </summary>
public static class ComparisonResultQueryExtensions
{
    /// <summary>Gets overlap rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical overlap order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<OverlapRow>> OverlapRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.Overlap);
        return Pair(result.OverlapRows, result.RowFinalities, offset);
    }

    /// <summary>Gets residual rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical residual order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<ResidualRow>> ResidualRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.Residual);
        return Pair(result.ResidualRows, result.RowFinalities, offset);
    }

    /// <summary>Gets missing rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical missing order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<MissingRow>> MissingRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.Missing);
        return Pair(result.MissingRows, result.RowFinalities, offset);
    }

    /// <summary>Gets coverage rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical coverage order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<CoverageRow>> CoverageRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.Coverage);
        return Pair(result.CoverageRows, result.RowFinalities, offset);
    }

    /// <summary>Gets gap rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical gap order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<GapRow>> GapRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.Gap);
        return Pair(result.GapRows, result.RowFinalities, offset);
    }

    /// <summary>Gets symmetric-difference rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical symmetric-difference order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<SymmetricDifferenceRow>> SymmetricDifferenceRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.SymmetricDifference);
        return Pair(result.SymmetricDifferenceRows, result.RowFinalities, offset);
    }

    /// <summary>Gets containment rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical containment order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<ContainmentRow>> ContainmentRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.Containment);
        return Pair(result.ContainmentRows, result.RowFinalities, offset);
    }

    /// <summary>Gets lead/lag rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical lead/lag order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<LeadLagRow>> LeadLagRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.LeadLag);
        return Pair(result.LeadLagRows, result.RowFinalities, offset);
    }

    /// <summary>Gets as-of rows paired with authoritative result metadata.</summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Rows and metadata in canonical as-of order.</returns>
    /// <exception cref="ComparisonRowMetadataException">The result metadata layout is inconsistent.</exception>
    public static IEnumerable<ComparisonRowWithFinality<AsOfRow>> AsOfRowsWithFinality(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        var offset = ComparisonRowMetadataValidator.ValidateAndGetOffset(result, ComparisonRowKind.AsOf);
        return Pair(result.AsOfRows, result.RowFinalities, offset);
    }

    /// <summary>
    /// Gets error diagnostics from a comparison result.
    /// </summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Error diagnostics in result order.</returns>
    public static IReadOnlyList<ComparisonPlanDiagnostic> ErrorDiagnostics(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return DiagnosticsBySeverity(result, ComparisonPlanDiagnosticSeverity.Error);
    }

    /// <summary>
    /// Gets warning diagnostics from a comparison result.
    /// </summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Warning diagnostics in result order.</returns>
    public static IReadOnlyList<ComparisonPlanDiagnostic> WarningDiagnostics(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return DiagnosticsBySeverity(result, ComparisonPlanDiagnosticSeverity.Warning);
    }

    /// <summary>
    /// Gets provisional row finality metadata from a comparison result.
    /// </summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Provisional row finality metadata in result order.</returns>
    public static IReadOnlyList<ComparisonRowFinality> ProvisionalRowFinalities(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return FinalitiesByState(result, ComparisonFinality.Provisional);
    }

    /// <summary>
    /// Gets final row finality metadata from a comparison result.
    /// </summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Final row finality metadata in result order.</returns>
    public static IReadOnlyList<ComparisonRowFinality> FinalRowFinalities(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return FinalitiesByState(result, ComparisonFinality.Final);
    }

    /// <summary>
    /// Gets whether the result contains any provisional rows.
    /// </summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>True when at least one row is provisional.</returns>
    public static bool HasProvisionalRows(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        for (var i = 0; i < result.RowFinalities.Count; i++)
        {
            if (result.RowFinalities[i].Finality == ComparisonFinality.Provisional)
            {
                return true;
            }
        }

        return false;
    }

    private static IReadOnlyList<ComparisonPlanDiagnostic> DiagnosticsBySeverity(
        ComparisonResult result,
        ComparisonPlanDiagnosticSeverity severity)
    {
        var diagnostics = new List<ComparisonPlanDiagnostic>();
        for (var i = 0; i < result.Diagnostics.Count; i++)
        {
            var diagnostic = result.Diagnostics[i];
            if (diagnostic.Severity == severity)
            {
                diagnostics.Add(diagnostic);
            }
        }

        return diagnostics.ToArray();
    }

    private static IReadOnlyList<ComparisonRowFinality> FinalitiesByState(
        ComparisonResult result,
        ComparisonFinality finality)
    {
        var finalities = new List<ComparisonRowFinality>();
        for (var i = 0; i < result.RowFinalities.Count; i++)
        {
            var rowFinality = result.RowFinalities[i];
            if (rowFinality.Finality == finality)
            {
                finalities.Add(rowFinality);
            }
        }

        return finalities.ToArray();
    }

    private static IEnumerable<ComparisonRowWithFinality<TRow>> Pair<TRow>(
        IReadOnlyList<TRow> rows,
        IReadOnlyList<ComparisonRowFinality> metadata,
        int offset)
    {
        for (var index = 0; index < rows.Count; index++)
        {
            yield return new ComparisonRowWithFinality<TRow>(rows[index], metadata[offset + index]);
        }
    }
}
