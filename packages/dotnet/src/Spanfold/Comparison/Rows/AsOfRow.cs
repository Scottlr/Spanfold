namespace Spanfold.Comparison;

/// <summary>
/// Describes one point-in-time as-of lookup result.
/// </summary>
/// <remarks>
/// As-of lookup is designed for point-in-time enrichment. Use
/// <see cref="AsOfDirection.Previous" /> to avoid future leakage.
/// </remarks>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Axis">The temporal axis used for lookup.</param>
/// <param name="Direction">The configured lookup direction.</param>
/// <param name="TargetPoint">The target lookup point.</param>
/// <param name="MatchedPoint">The matched comparison point, when one was selected.</param>
/// <param name="DistanceMagnitude">The absolute distance in positions or timestamp ticks, when a candidate was evaluated.</param>
/// <param name="ToleranceMagnitude">The configured tolerance in positions or timestamp ticks.</param>
/// <param name="Status">The lookup status.</param>
/// <param name="TargetRecordId">The target record that produced the lookup point.</param>
/// <param name="MatchedRecordId">The comparison record selected for enrichment, when any.</param>
public sealed record AsOfRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalAxis Axis,
    AsOfDirection Direction,
    TemporalPoint TargetPoint,
    TemporalPoint? MatchedPoint,
    long? DistanceMagnitude,
    long ToleranceMagnitude,
    AsOfMatchStatus Status,
    WindowRecordId TargetRecordId,
    WindowRecordId? MatchedRecordId);
