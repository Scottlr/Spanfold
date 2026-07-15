namespace Spanfold.Revisions;

/// <summary>
/// Describes how a comparison row changed between snapshots.
/// </summary>
public enum ComparisonRevisionKind
{
    /// <summary>The row was not present in the previous snapshot.</summary>
    Added = 0,

    /// <summary>The row metadata changed while retaining the same identity.</summary>
    Revised = 1,

    /// <summary>The row is no longer present in the current snapshot.</summary>
    Retracted = 2
}
