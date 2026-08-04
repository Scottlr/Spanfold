namespace Spanfold.Watermarks;

/// <summary>Identifies one caller-defined event revision within a watermark lane.</summary>
/// <param name="LaneId">The stable lane identifier.</param>
/// <param name="EventId">The stable logical event identifier.</param>
/// <param name="RevisionId">The stable revision identifier supplied by the caller.</param>
public sealed record WatermarkRevisionReference(string LaneId, string EventId, string RevisionId);
