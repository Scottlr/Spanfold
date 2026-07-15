namespace Spanfold.Comparison;

/// <summary>
/// Identifies how a normalized window participates in a comparison.
/// </summary>
public enum ComparisonSide
{
    /// <summary>
    /// The window belongs to the target side of the comparison.
    /// </summary>
    Target = 0,

    /// <summary>
    /// The window belongs to one of the comparison sides.
    /// </summary>
    Against = 1
}
