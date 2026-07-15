namespace Spanfold.Comparison;

/// <summary>
/// Describes the outcome of an as-of lookup.
/// </summary>
public enum AsOfMatchStatus
{
    /// <summary>
    /// A comparison transition matched exactly.
    /// </summary>
    Exact = 0,

    /// <summary>
    /// A comparison transition matched within tolerance.
    /// </summary>
    Matched = 1,

    /// <summary>
    /// No eligible comparison transition was inside tolerance.
    /// </summary>
    NoMatch = 2,

    /// <summary>
    /// A future transition was available but rejected by previous-only lookup.
    /// </summary>
    FutureRejected = 3,

    /// <summary>
    /// Multiple comparison transitions were equally eligible; the emitted match is deterministic.
    /// </summary>
    Ambiguous = 4
}
