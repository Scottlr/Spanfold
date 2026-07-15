namespace Spanfold.Comparison;

/// <summary>
/// Describes how a parent/child hierarchy segment should be interpreted.
/// </summary>
public enum HierarchyComparisonRowKind
{
    /// <summary>
    /// Parent activity is explained by at least one active child contribution.
    /// </summary>
    ParentExplained = 0,

    /// <summary>
    /// Parent activity has no active child contribution in the same source and partition.
    /// </summary>
    UnexplainedParent = 1,

    /// <summary>
    /// Child activity is present outside an active parent window.
    /// </summary>
    OrphanChild = 2
}
