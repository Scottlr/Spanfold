namespace Spanfold.Comparison;

/// <summary>
/// Describes the lead or lag between a target transition and its nearest comparison transition.
/// </summary>
/// <remarks>
/// Lead/lag rows are transition measurements, not proof of causality. Negative
/// deltas mean the target led the comparison; positive deltas mean the target
/// lagged the comparison.
/// </remarks>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Transition">The transition point being compared.</param>
/// <param name="Axis">The temporal axis used for the measurement.</param>
/// <param name="TargetPoint">The target transition point.</param>
/// <param name="ComparisonPoint">The nearest comparison transition point, if one was found.</param>
/// <param name="DeltaMagnitude">The signed target-minus-comparison delta in positions or timestamp ticks.</param>
/// <param name="ToleranceMagnitude">The allowed absolute delta in positions or timestamp ticks.</param>
/// <param name="IsWithinTolerance">Whether the absolute delta is inside the configured tolerance.</param>
/// <param name="Direction">The lead/lag direction.</param>
/// <param name="TargetRecordId">The target record that produced the transition.</param>
/// <param name="ComparisonRecordId">The comparison record that produced the nearest transition, if any.</param>
public sealed record LeadLagRow(
    string WindowName,
    object Key,
    object? Partition,
    LeadLagTransition Transition,
    TemporalAxis Axis,
    TemporalPoint TargetPoint,
    TemporalPoint? ComparisonPoint,
    long? DeltaMagnitude,
    long ToleranceMagnitude,
    bool IsWithinTolerance,
    LeadLagDirection Direction,
    WindowRecordId TargetRecordId,
    WindowRecordId? ComparisonRecordId);
