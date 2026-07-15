namespace Spanfold.Episodes;

/// <summary>
/// Describes how close fragments must be to relate episodes across two sides.
/// </summary>
public sealed record EpisodeRelationPolicy
{
    /// <summary>
    /// Creates an episode-relation policy.
    /// </summary>
    /// <param name="timeAxis">The temporal axis used by both episode sets.</param>
    /// <param name="toleranceMagnitude">The maximum cross-side fragment gap.</param>
    public EpisodeRelationPolicy(TemporalAxis timeAxis, long toleranceMagnitude)
    {
        if (timeAxis == TemporalAxis.Unknown)
        {
            throw new ArgumentException("Episode relation requires a known temporal axis.", nameof(timeAxis));
        }

        ArgumentOutOfRangeException.ThrowIfNegative(toleranceMagnitude);
        TimeAxis = timeAxis;
        ToleranceMagnitude = toleranceMagnitude;
    }

    /// <summary>Gets the temporal axis used by both episode sets.</summary>
    public TemporalAxis TimeAxis { get; }

    /// <summary>Gets the maximum cross-side fragment gap.</summary>
    public long ToleranceMagnitude { get; }
}
