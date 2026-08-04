namespace Spanfold.Watermarks;

/// <summary>Describes a stable replacement and retraction pair for downstream changelogs.</summary>
/// <param name="Replacement">The newly accepted event revision.</param>
/// <param name="Retraction">The previously accepted event revision being retracted.</param>
public sealed record WatermarkCorrection(
    WatermarkRevisionReference Replacement,
    WatermarkRevisionReference Retraction);
