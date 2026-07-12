namespace Spanfold;

/// <summary>
/// Represents a temporal explanation of parent and child recorded windows.
/// </summary>
/// <remarks>
/// Hierarchy comparison currently infers lineage from the supplied parent and
/// child window names plus matching source and partition. It does not require
/// domain-specific parent/child key metadata.
/// </remarks>
/// <param name="Name">The comparison name.</param>
/// <param name="ParentWindowName">The parent recorded window name.</param>
/// <param name="ChildWindowName">The child contribution window name.</param>
/// <param name="Rows">The deterministic hierarchy rows.</param>
/// <param name="Diagnostics">Diagnostics produced while building the comparison.</param>
public sealed record HierarchyComparisonResult(
    string Name,
    string ParentWindowName,
    string ChildWindowName,
    IReadOnlyList<HierarchyComparisonRow> Rows,
    IReadOnlyList<ComparisonPlanDiagnostic> Diagnostics)
{
    /// <summary>Gets deterministic hierarchy rows.</summary>
    public IReadOnlyList<HierarchyComparisonRow> Rows { get; } = Array.AsReadOnly(Rows.ToArray());

    /// <summary>Gets diagnostics produced while building the comparison.</summary>
    public IReadOnlyList<ComparisonPlanDiagnostic> Diagnostics { get; } = Array.AsReadOnly(Diagnostics.ToArray());
}
