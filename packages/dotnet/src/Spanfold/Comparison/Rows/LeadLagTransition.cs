namespace Spanfold.Comparison;

/// <summary>
/// Describes which recorded window transition should be used for lead/lag measurement.
/// </summary>
public enum LeadLagTransition
{
    /// <summary>
    /// Compare recorded window start transitions.
    /// </summary>
    Start = 0,

    /// <summary>
    /// Compare recorded window end transitions.
    /// </summary>
    End = 1
}
