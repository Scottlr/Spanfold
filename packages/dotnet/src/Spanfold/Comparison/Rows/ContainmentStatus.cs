namespace Spanfold.Comparison;

/// <summary>
/// Describes the containment relationship for a target-active segment.
/// </summary>
public enum ContainmentStatus
{
    /// <summary>
    /// The target segment is covered by at least one comparison container window.
    /// </summary>
    Contained = 0,

    /// <summary>
    /// The target segment is not covered by a comparison container window.
    /// </summary>
    NotContained = 1,

    /// <summary>
    /// The target segment starts before comparison container coverage.
    /// </summary>
    LeftOverhang = 2,

    /// <summary>
    /// The target segment continues after comparison container coverage.
    /// </summary>
    RightOverhang = 3
}
