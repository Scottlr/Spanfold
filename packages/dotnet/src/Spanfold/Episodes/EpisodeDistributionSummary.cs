namespace Spanfold.Episodes;

/// <summary>
/// Describes a deterministic distribution over temporal magnitudes.
/// </summary>
/// <param name="Count">The number of observed values.</param>
/// <param name="Minimum">The smallest value, or null for an empty distribution.</param>
/// <param name="Mean">The arithmetic mean, or null for an empty distribution.</param>
/// <param name="Median">The median value, or null for an empty distribution.</param>
/// <param name="Percentile95">The nearest-rank 95th percentile, or null for an empty distribution.</param>
/// <param name="Maximum">The largest value, or null for an empty distribution.</param>
public sealed record EpisodeDistributionSummary(
    int Count,
    long? Minimum,
    double? Mean,
    double? Median,
    long? Percentile95,
    long? Maximum);
