namespace Spanfold;

/// <summary>
/// Describes how recorded windows are normalized before comparison.
/// </summary>
/// <remarks>
/// Normalization is where Spanfold chooses the temporal axis, open-window finality
/// policy, missing timestamp behavior, and duplicate handling before windows are
/// aligned into deterministic comparison segments.
/// </remarks>
/// <param name="TimeAxis">The temporal axis used for normalized ranges.</param>
/// <param name="OpenWindowPolicy">How open windows are handled.</param>
/// <param name="OpenWindowHorizon">The optional exclusive horizon used when open windows are clipped. Open windows are not infinite.</param>
/// <param name="NullTimestampPolicy">How missing event timestamps are handled in event-time mode.</param>
/// <param name="KnownAt">The availability point used to prevent future leakage. This is availability time, not event time.</param>
public sealed record ComparisonNormalizationPolicy(
    TemporalAxis TimeAxis = TemporalAxis.ProcessingPosition,
    ComparisonOpenWindowPolicy OpenWindowPolicy = ComparisonOpenWindowPolicy.RequireClosed,
    TemporalPoint? OpenWindowHorizon = null,
    ComparisonNullTimestampPolicy NullTimestampPolicy = ComparisonNullTimestampPolicy.Reject,
    TemporalPoint? KnownAt = null)
{
    /// <summary>
    /// Gets the default historical comparison normalization policy.
    /// </summary>
    public static ComparisonNormalizationPolicy Default { get; } = new();
}
