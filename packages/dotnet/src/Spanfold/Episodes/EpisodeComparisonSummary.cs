namespace Spanfold.Episodes;

/// <summary>
/// Summarizes neutral target-against episode relationships and timing.
/// </summary>
/// <param name="TimeAxis">The temporal axis and magnitude unit.</param>
/// <param name="TargetEpisodeCount">The target episode count.</param>
/// <param name="AgainstEpisodeCount">The against episode count.</param>
/// <param name="MatchedTargetEpisodeCount">Target episodes in components containing both sides.</param>
/// <param name="MatchedAgainstEpisodeCount">Against episodes in components containing both sides.</param>
/// <param name="UnmatchedTargetEpisodeCount">Target episodes with no against relationship.</param>
/// <param name="UnmatchedAgainstEpisodeCount">Against episodes with no target relationship.</param>
/// <param name="OneToOneRelationCount">The one-to-one component count.</param>
/// <param name="SplitRelationCount">The split component count.</param>
/// <param name="MergeRelationCount">The merge component count.</param>
/// <param name="ComplexRelationCount">The many-to-many component count.</param>
/// <param name="SplitTargetEpisodeCount">Target episodes participating in split components.</param>
/// <param name="MergedAgainstEpisodeCount">Against episodes participating in merge components.</param>
/// <param name="EpisodeCountBias">Against episode count minus target episode count.</param>
/// <param name="ActiveMagnitudeBias">Against set active magnitude minus target set active magnitude.</param>
/// <param name="TargetMatchRate">Matched target episodes divided by target episodes.</param>
/// <param name="AgainstMatchRate">Matched against episodes divided by against episodes.</param>
/// <param name="SplitTargetRate">Split target episodes divided by target episodes.</param>
/// <param name="MergeAgainstRate">Merged against episodes divided by against episodes.</param>
/// <param name="TotalOverlapMagnitude">The sum of component active-overlap magnitudes.</param>
/// <param name="TargetCoverageRatio">Total overlap divided by component target active magnitude.</param>
/// <param name="AgainstCoverageRatio">Total overlap divided by component against active magnitude.</param>
/// <param name="IntersectionOverUnion">Total overlap divided by the component active union.</param>
/// <param name="OnsetDeltaDistribution">One-to-one onset deltas.</param>
/// <param name="RecoveryDeltaDistribution">One-to-one recovery deltas.</param>
/// <param name="ActiveMagnitudeDeltaDistribution">One-to-one active-magnitude deltas.</param>
/// <param name="ElapsedMagnitudeDeltaDistribution">One-to-one elapsed-magnitude deltas.</param>
public sealed record EpisodeComparisonSummary(
    TemporalAxis TimeAxis,
    int TargetEpisodeCount,
    int AgainstEpisodeCount,
    int MatchedTargetEpisodeCount,
    int MatchedAgainstEpisodeCount,
    int UnmatchedTargetEpisodeCount,
    int UnmatchedAgainstEpisodeCount,
    int OneToOneRelationCount,
    int SplitRelationCount,
    int MergeRelationCount,
    int ComplexRelationCount,
    int SplitTargetEpisodeCount,
    int MergedAgainstEpisodeCount,
    int EpisodeCountBias,
    long ActiveMagnitudeBias,
    double? TargetMatchRate,
    double? AgainstMatchRate,
    double? SplitTargetRate,
    double? MergeAgainstRate,
    long TotalOverlapMagnitude,
    double? TargetCoverageRatio,
    double? AgainstCoverageRatio,
    double? IntersectionOverUnion,
    EpisodeDistributionSummary OnsetDeltaDistribution,
    EpisodeDistributionSummary RecoveryDeltaDistribution,
    EpisodeDistributionSummary ActiveMagnitudeDeltaDistribution,
    EpisodeDistributionSummary ElapsedMagnitudeDeltaDistribution);
