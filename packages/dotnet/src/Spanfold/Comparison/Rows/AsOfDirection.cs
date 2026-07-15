namespace Spanfold.Comparison;

/// <summary>
/// Describes which comparison transition is eligible for an as-of lookup.
/// </summary>
public enum AsOfDirection
{
    /// <summary>
    /// Match the latest comparison transition at or before the target point.
    /// </summary>
    Previous = 0,

    /// <summary>
    /// Match the earliest comparison transition at or after the target point.
    /// </summary>
    Next = 1,

    /// <summary>
    /// Match the closest comparison transition on either side of the target point.
    /// </summary>
    Nearest = 2
}
