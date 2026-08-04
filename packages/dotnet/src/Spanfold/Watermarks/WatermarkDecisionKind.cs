namespace Spanfold.Watermarks;

/// <summary>Describes the bounded event-time decision made for one event revision.</summary>
public enum WatermarkDecisionKind
{
    /// <summary>The event is ahead of reported lane progress and remains buffered.</summary>
    Buffered = 0,
    /// <summary>The event revision is accepted without retracting an earlier revision.</summary>
    Accepted = 1,
    /// <summary>The event is outside the bounded acceptance and correction horizons.</summary>
    Rejected = 2,
    /// <summary>The event revision replaces an accepted revision of the same event.</summary>
    Corrected = 3
}
