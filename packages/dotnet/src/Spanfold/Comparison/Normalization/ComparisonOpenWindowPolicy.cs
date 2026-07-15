namespace Spanfold.Comparison;

/// <summary>
/// Describes how open recorded windows participate in comparison normalization.
/// </summary>
public enum ComparisonOpenWindowPolicy
{
    /// <summary>
    /// Rejects open windows unless a later policy explicitly clips them.
    /// </summary>
    RequireClosed = 0,

    /// <summary>
    /// Clips open windows to an explicit exclusive evaluation horizon.
    /// </summary>
    ClipToHorizon = 1
}
