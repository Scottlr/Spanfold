namespace Spanfold.Liveness;

/// <summary>
/// Describes a lane liveness state change emitted by <see cref="LaneLivenessTracker" />.
/// </summary>
/// <param name="Lane">The lane whose liveness changed.</param>
/// <param name="IsSilent">Whether the lane is considered silent.</param>
/// <param name="OccurredAt">When the liveness state changed.</param>
/// <param name="EvaluatedAt">When the state change was detected or observed.</param>
/// <param name="SilenceThreshold">The silence threshold used by the tracker.</param>
public sealed record LaneLivenessSignal(
    object Lane,
    bool IsSilent,
    DateTimeOffset OccurredAt,
    DateTimeOffset EvaluatedAt,
    TimeSpan SilenceThreshold);
