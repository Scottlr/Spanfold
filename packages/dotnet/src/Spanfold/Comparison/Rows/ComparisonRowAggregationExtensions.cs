namespace Spanfold.Comparison;

/// <summary>
/// Provides small aggregation helpers for comparison result rows.
/// </summary>
public static class ComparisonRowAggregationExtensions
{
    /// <summary>
    /// Sums event-time duration across overlap rows.
    /// </summary>
    /// <param name="rows">The overlap rows to aggregate.</param>
    /// <returns>The total event-time duration.</returns>
    public static TimeSpan TotalTimeDuration(this IEnumerable<OverlapRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = TimeSpan.Zero;
        foreach (var row in rows)
        {
            total += row.Range.GetTimeDuration();
        }

        return total;
    }

    /// <summary>
    /// Sums processing-position length across overlap rows.
    /// </summary>
    /// <param name="rows">The overlap rows to aggregate.</param>
    /// <returns>The total processing-position length.</returns>
    public static long TotalPositionLength(this IEnumerable<OverlapRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0L;
        foreach (var row in rows)
        {
            total += row.Range.GetPositionLength();
        }

        return total;
    }

    /// <summary>
    /// Sums event-time duration across residual rows.
    /// </summary>
    /// <param name="rows">The residual rows to aggregate.</param>
    /// <returns>The total event-time duration.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown when any row is not on the timestamp axis.
    /// </exception>
    public static TimeSpan TotalTimeDuration(this IEnumerable<ResidualRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = TimeSpan.Zero;
        foreach (var row in rows)
        {
            total += row.Range.GetTimeDuration();
        }

        return total;
    }

    /// <summary>
    /// Sums processing-position length across residual rows.
    /// </summary>
    /// <param name="rows">The residual rows to aggregate.</param>
    /// <returns>The total processing-position length.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown when any row is not on the processing-position axis.
    /// </exception>
    public static long TotalPositionLength(this IEnumerable<ResidualRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0L;
        foreach (var row in rows)
        {
            total += row.Range.GetPositionLength();
        }

        return total;
    }

    /// <summary>
    /// Sums event-time duration across missing rows.
    /// </summary>
    /// <param name="rows">The missing rows to aggregate.</param>
    /// <returns>The total event-time duration.</returns>
    public static TimeSpan TotalTimeDuration(this IEnumerable<MissingRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = TimeSpan.Zero;
        foreach (var row in rows)
        {
            total += row.Range.GetTimeDuration();
        }

        return total;
    }

    /// <summary>
    /// Sums processing-position length across missing rows.
    /// </summary>
    /// <param name="rows">The missing rows to aggregate.</param>
    /// <returns>The total processing-position length.</returns>
    public static long TotalPositionLength(this IEnumerable<MissingRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0L;
        foreach (var row in rows)
        {
            total += row.Range.GetPositionLength();
        }

        return total;
    }

    /// <summary>
    /// Sums event-time duration across coverage rows.
    /// </summary>
    /// <param name="rows">The coverage rows to aggregate.</param>
    /// <returns>The total event-time duration.</returns>
    public static TimeSpan TotalTimeDuration(this IEnumerable<CoverageRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = TimeSpan.Zero;
        foreach (var row in rows)
        {
            total += row.Range.GetTimeDuration();
        }

        return total;
    }

    /// <summary>
    /// Sums processing-position length across coverage rows.
    /// </summary>
    /// <param name="rows">The coverage rows to aggregate.</param>
    /// <returns>The total processing-position length.</returns>
    public static long TotalPositionLength(this IEnumerable<CoverageRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0L;
        foreach (var row in rows)
        {
            total += row.Range.GetPositionLength();
        }

        return total;
    }

    /// <summary>
    /// Sums target denominator magnitude across coverage rows.
    /// </summary>
    /// <param name="rows">The coverage rows to aggregate.</param>
    /// <returns>The total target magnitude.</returns>
    public static double TotalTargetMagnitude(this IEnumerable<CoverageRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0d;
        foreach (var row in rows)
        {
            total += row.TargetMagnitude;
        }

        return total;
    }

    /// <summary>
    /// Sums comparison-covered numerator magnitude across coverage rows.
    /// </summary>
    /// <param name="rows">The coverage rows to aggregate.</param>
    /// <returns>The total covered magnitude.</returns>
    public static double TotalCoveredMagnitude(this IEnumerable<CoverageRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0d;
        foreach (var row in rows)
        {
            total += row.CoveredMagnitude;
        }

        return total;
    }

    /// <summary>
    /// Sums event-time duration across gap rows.
    /// </summary>
    /// <param name="rows">The gap rows to aggregate.</param>
    /// <returns>The total event-time duration.</returns>
    public static TimeSpan TotalTimeDuration(this IEnumerable<GapRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = TimeSpan.Zero;
        foreach (var row in rows)
        {
            total += row.Range.GetTimeDuration();
        }

        return total;
    }

    /// <summary>
    /// Sums processing-position length across gap rows.
    /// </summary>
    /// <param name="rows">The gap rows to aggregate.</param>
    /// <returns>The total processing-position length.</returns>
    public static long TotalPositionLength(this IEnumerable<GapRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0L;
        foreach (var row in rows)
        {
            total += row.Range.GetPositionLength();
        }

        return total;
    }

    /// <summary>
    /// Sums event-time duration across symmetric-difference rows.
    /// </summary>
    /// <param name="rows">The symmetric-difference rows to aggregate.</param>
    /// <returns>The total event-time duration.</returns>
    public static TimeSpan TotalTimeDuration(this IEnumerable<SymmetricDifferenceRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = TimeSpan.Zero;
        foreach (var row in rows)
        {
            total += row.Range.GetTimeDuration();
        }

        return total;
    }

    /// <summary>
    /// Sums processing-position length across symmetric-difference rows.
    /// </summary>
    /// <param name="rows">The symmetric-difference rows to aggregate.</param>
    /// <returns>The total processing-position length.</returns>
    public static long TotalPositionLength(this IEnumerable<SymmetricDifferenceRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);

        var total = 0L;
        foreach (var row in rows)
        {
            total += row.Range.GetPositionLength();
        }

        return total;
    }
}
