namespace Spanfold.Comparison;

/// <summary>
/// Describes how event-time normalization handles records without timestamps.
/// </summary>
public enum ComparisonNullTimestampPolicy
{
    /// <summary>
    /// Treats missing event timestamps as diagnostics that block safe event-time comparison.
    /// </summary>
    Reject = 0,

    /// <summary>
    /// Excludes records with missing event timestamps from event-time comparison.
    /// </summary>
    Exclude = 1
}
