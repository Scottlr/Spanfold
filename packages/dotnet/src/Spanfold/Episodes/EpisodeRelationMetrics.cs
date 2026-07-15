namespace Spanfold.Episodes;

/// <summary>
/// Describes active coverage, proximity, and directional deltas for one relation component.
/// </summary>
/// <param name="TimeAxis">The temporal axis and magnitude unit.</param>
/// <param name="TargetActiveMagnitude">The union magnitude of target fragments.</param>
/// <param name="AgainstActiveMagnitude">The union magnitude of against fragments.</param>
/// <param name="OverlapMagnitude">The intersection magnitude of both active unions.</param>
/// <param name="TargetCoverageRatio">The fraction of target activity covered by against activity.</param>
/// <param name="AgainstCoverageRatio">The fraction of against activity covered by target activity.</param>
/// <param name="IntersectionOverUnion">The active intersection divided by the active union.</param>
/// <param name="MinimumGapMagnitude">The minimum cross-side fragment gap.</param>
/// <param name="OnsetDeltaMagnitude">The earliest against start minus the earliest target start.</param>
/// <param name="RecoveryDeltaMagnitude">The latest against end minus the latest target end.</param>
/// <param name="ActiveMagnitudeDelta">Against active magnitude minus target active magnitude.</param>
/// <param name="ElapsedMagnitudeDelta">Against envelope magnitude minus target envelope magnitude.</param>
public sealed record EpisodeRelationMetrics(
    TemporalAxis TimeAxis,
    long TargetActiveMagnitude,
    long AgainstActiveMagnitude,
    long OverlapMagnitude,
    double? TargetCoverageRatio,
    double? AgainstCoverageRatio,
    double? IntersectionOverUnion,
    long? MinimumGapMagnitude,
    long? OnsetDeltaMagnitude,
    long? RecoveryDeltaMagnitude,
    long? ActiveMagnitudeDelta,
    long? ElapsedMagnitudeDelta);
