namespace Spanfold.Comparison;

/// <summary>
/// Builds normalization policy for a comparison plan.
/// </summary>
public sealed class ComparisonNormalizationBuilder
{
    private TemporalAxis timeAxis = TemporalAxis.ProcessingPosition;
    private ComparisonOpenWindowPolicy openWindowPolicy = ComparisonOpenWindowPolicy.RequireClosed;
    private TemporalPoint? openWindowHorizon;
    private ComparisonNullTimestampPolicy nullTimestampPolicy = ComparisonNullTimestampPolicy.Reject;
    private TemporalPoint? knownAt;

    /// <summary>
    /// Requires recorded windows to be closed before historical comparison.
    /// </summary>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder RequireClosedWindows()
    {
        this.openWindowPolicy = ComparisonOpenWindowPolicy.RequireClosed;
        this.openWindowHorizon = null;
        return this;
    }

    /// <summary>
    /// Clips open windows to an explicit horizon.
    /// </summary>
    /// <remarks>
    /// Open windows are not treated as infinite. The supplied horizon becomes
    /// the exclusive effective end for open windows, and the normalized range is
    /// marked with <see cref="TemporalRangeEndStatus.OpenAtHorizon" />.
    /// </remarks>
    /// <param name="horizon">The effective end for open windows.</param>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder ClipOpenWindowsTo(TemporalPoint horizon)
    {
        this.openWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon;
        this.openWindowHorizon = horizon;
        return this;
    }

    /// <summary>
    /// Normalizes windows on the processing-position axis.
    /// </summary>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder OnPosition()
    {
        this.timeAxis = TemporalAxis.ProcessingPosition;
        return this;
    }

    /// <summary>
    /// Normalizes windows on the event-time axis.
    /// </summary>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder OnEventTime()
    {
        this.timeAxis = TemporalAxis.Timestamp;
        return this;
    }

    /// <summary>
    /// Rejects records with missing event timestamps in event-time mode.
    /// </summary>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder RejectMissingEventTime()
    {
        this.nullTimestampPolicy = ComparisonNullTimestampPolicy.Reject;
        return this;
    }

    /// <summary>
    /// Excludes records with missing event timestamps in event-time mode.
    /// </summary>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder ExcludeMissingEventTime()
    {
        this.nullTimestampPolicy = ComparisonNullTimestampPolicy.Exclude;
        return this;
    }

    /// <summary>
    /// Applies a processing-position known-at point to prevent future leakage.
    /// </summary>
    /// <remarks>
    /// Known-at is availability time, not event time. Closed windows become
    /// available at their close position; open windows are available at their
    /// start position.
    /// </remarks>
    /// <param name="position">The processing position known at decision time.</param>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder KnownAtPosition(long position)
    {
        this.knownAt = TemporalPoint.ForPosition(position);
        return this;
    }

    /// <summary>
    /// Applies a known-at point to prevent future leakage.
    /// </summary>
    /// <remarks>
    /// Known-at is availability time, not event time. Processing-position
    /// known-at points are enforced during preparation; timestamp known-at
    /// points are diagnosed until explicit availability clocks are available.
    /// </remarks>
    /// <param name="point">The availability point known at decision time.</param>
    /// <returns>This builder.</returns>
    public ComparisonNormalizationBuilder KnownAt(TemporalPoint point)
    {
        this.knownAt = point;
        return this;
    }

    internal ComparisonNormalizationPolicy Build()
    {
        return new ComparisonNormalizationPolicy(
            this.timeAxis,
            this.openWindowPolicy,
            this.openWindowHorizon,
            this.nullTimestampPolicy,
            this.knownAt);
    }
}
