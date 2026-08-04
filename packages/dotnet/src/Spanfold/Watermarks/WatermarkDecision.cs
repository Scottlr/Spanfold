namespace Spanfold.Watermarks;

/// <summary>Contains the deterministic decision for one event revision.</summary>
/// <param name="Revision">The event revision evaluated by the tracker.</param>
/// <param name="EventTime">The event-time instant in UTC.</param>
/// <param name="Kind">The resulting bounded-watermark decision.</param>
/// <param name="Watermark">The lane watermark used for the decision, when progress is known.</param>
/// <param name="Correction">The replacement and retraction pair for a corrected revision.</param>
public sealed record WatermarkDecision(
    WatermarkRevisionReference Revision,
    DateTimeOffset EventTime,
    WatermarkDecisionKind Kind,
    DateTimeOffset? Watermark,
    WatermarkCorrection? Correction = null);
