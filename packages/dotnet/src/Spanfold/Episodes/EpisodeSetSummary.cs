namespace Spanfold.Episodes;

/// <summary>
/// Summarizes occurrence counts, fragments, and magnitudes for one episode set.
/// </summary>
/// <param name="TimeAxis">The temporal axis and magnitude unit.</param>
/// <param name="EpisodeCount">The total number of episodes.</param>
/// <param name="FinalEpisodeCount">The number of final episodes.</param>
/// <param name="ProvisionalEpisodeCount">The number of provisional episodes.</param>
/// <param name="FragmentCount">The total number of source fragments.</param>
/// <param name="MultiFragmentEpisodeCount">The number of episodes containing multiple fragments.</param>
/// <param name="MultiFragmentEpisodeRate">The multi-fragment count divided by episode count.</param>
/// <param name="MeanFragmentsPerEpisode">The mean fragment count per episode.</param>
/// <param name="MaximumFragmentsPerEpisode">The largest fragment count in one episode.</param>
/// <param name="TotalActiveMagnitude">The sum of per-episode active magnitudes.</param>
/// <param name="TotalElapsedMagnitude">The sum of per-episode elapsed magnitudes.</param>
/// <param name="TotalInternalGapMagnitude">The sum of per-episode internal gaps.</param>
/// <param name="ActiveMagnitudeDistribution">The per-episode active-magnitude distribution.</param>
/// <param name="ElapsedMagnitudeDistribution">The per-episode elapsed-magnitude distribution.</param>
/// <param name="InternalGapMagnitudeDistribution">The per-episode internal-gap distribution.</param>
public sealed record EpisodeSetSummary(
    TemporalAxis TimeAxis,
    int EpisodeCount,
    int FinalEpisodeCount,
    int ProvisionalEpisodeCount,
    int FragmentCount,
    int MultiFragmentEpisodeCount,
    double? MultiFragmentEpisodeRate,
    double? MeanFragmentsPerEpisode,
    int MaximumFragmentsPerEpisode,
    long TotalActiveMagnitude,
    long TotalElapsedMagnitude,
    long TotalInternalGapMagnitude,
    EpisodeDistributionSummary ActiveMagnitudeDistribution,
    EpisodeDistributionSummary ElapsedMagnitudeDistribution,
    EpisodeDistributionSummary InternalGapMagnitudeDistribution);
