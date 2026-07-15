namespace Spanfold.Comparison;

/// <summary>
/// Summarizes lead/lag measurements for one comparator run.
/// </summary>
/// <param name="Transition">The transition point that was measured.</param>
/// <param name="Axis">The temporal axis used for measurement.</param>
/// <param name="ToleranceMagnitude">The configured tolerance in positions or timestamp ticks.</param>
/// <param name="RowCount">The number of emitted rows.</param>
/// <param name="TargetLeadCount">The number of rows where the target led.</param>
/// <param name="TargetLagCount">The number of rows where the target lagged.</param>
/// <param name="EqualCount">The number of rows where both transitions were equal.</param>
/// <param name="MissingComparisonCount">The number of rows with no comparison transition.</param>
/// <param name="OutsideToleranceCount">The number of rows outside tolerance.</param>
/// <param name="MinimumDeltaMagnitude">The minimum signed delta, when any paired transitions exist.</param>
/// <param name="MaximumDeltaMagnitude">The maximum signed delta, when any paired transitions exist.</param>
public sealed record LeadLagSummary(
    LeadLagTransition Transition,
    TemporalAxis Axis,
    long ToleranceMagnitude,
    int RowCount,
    int TargetLeadCount,
    int TargetLagCount,
    int EqualCount,
    int MissingComparisonCount,
    int OutsideToleranceCount,
    long? MinimumDeltaMagnitude,
    long? MaximumDeltaMagnitude);
