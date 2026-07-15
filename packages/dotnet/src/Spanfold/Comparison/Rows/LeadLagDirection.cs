namespace Spanfold.Comparison;

/// <summary>
/// Describes the ordering relationship between target and comparison transitions.
/// </summary>
public enum LeadLagDirection
{
    /// <summary>
    /// The target and comparison transitions occurred at the same point.
    /// </summary>
    Equal = 0,

    /// <summary>
    /// The target transition occurred before the comparison transition.
    /// </summary>
    TargetLeads = 1,

    /// <summary>
    /// The target transition occurred after the comparison transition.
    /// </summary>
    TargetLags = 2,

    /// <summary>
    /// No corresponding comparison transition was found for the target transition.
    /// </summary>
    MissingComparison = 3
}
