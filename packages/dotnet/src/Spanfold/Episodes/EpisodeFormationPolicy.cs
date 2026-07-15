namespace Spanfold.Episodes;

/// <summary>
/// Describes how normalized window fragments are stitched into episodes.
/// </summary>
public sealed record EpisodeFormationPolicy
{
    /// <summary>
    /// Creates an episode-formation policy.
    /// </summary>
    /// <param name="timeAxis">The temporal axis used by fragment ranges.</param>
    /// <param name="stitchToleranceMagnitude">The maximum gap included in one episode.</param>
    public EpisodeFormationPolicy(TemporalAxis timeAxis, long stitchToleranceMagnitude)
    {
        if (timeAxis == TemporalAxis.Unknown)
        {
            throw new ArgumentException("Episode formation requires a known temporal axis.", nameof(timeAxis));
        }

        ArgumentOutOfRangeException.ThrowIfNegative(stitchToleranceMagnitude);
        TimeAxis = timeAxis;
        StitchToleranceMagnitude = stitchToleranceMagnitude;
    }

    /// <summary>Gets the temporal axis used by fragment ranges.</summary>
    public TemporalAxis TimeAxis { get; }

    /// <summary>Gets the maximum gap included in one episode.</summary>
    public long StitchToleranceMagnitude { get; }
}
